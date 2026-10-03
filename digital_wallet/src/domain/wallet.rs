//! Composition root: the digital wallet "singleton". See DESIGN.md.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::domain::accounts::{Account, User};
use crate::domain::payments::PaymentMethod;
use crate::domain::transactions::{Transaction, TransactionManager};
use crate::error::{WalletError, WalletResult};
use crate::locks::{rd, wr};
use crate::money::{Currency, Money};

struct StoredMethod {
    owner: u64,
    method: PaymentMethod,
}

/// Owns the user directory; balances live in `tx`. Lock order where two are
/// held at once: `users` then `payment_methods`, never the ledger.
pub struct DigitalWallet {
    pub tx: Arc<TransactionManager>,
    users: RwLock<HashMap<u64, User>>,
    payment_methods: RwLock<HashMap<u64, StoredMethod>>,
    next_user: AtomicU64,
    next_account: AtomicU64,
    next_method: AtomicU64,
}

impl DigitalWallet {
    pub fn new() -> Self {
        Self {
            tx: Arc::new(TransactionManager::new()),
            users: RwLock::new(HashMap::new()),
            payment_methods: RwLock::new(HashMap::new()),
            next_user: AtomicU64::new(0),
            next_account: AtomicU64::new(0),
            next_method: AtomicU64::new(0),
        }
    }

    pub fn create_user(&self, name: &str, email: &str, password: &str) -> WalletResult<User> {
        if name.is_empty() || email.is_empty() || password.is_empty() {
            return Err(WalletError::InvalidInput(
                "name, email, and password are required".into(),
            ));
        }
        let id = self.next_user.fetch_add(1, Ordering::Relaxed) + 1;
        let user = User::new(id, name, email, &format!("hash({password})"));
        wr(&self.users).insert(id, user.clone());
        Ok(user)
    }

    pub fn user(&self, id: u64) -> Option<User> {
        rd(&self.users).get(&id).cloned()
    }

    /// Unknown email and wrong password fail alike, so the caller cannot
    /// enumerate registered addresses.
    pub fn authenticate(&self, email: &str, password: &str) -> WalletResult<User> {
        let expected = format!("hash({password})");
        rd(&self.users)
            .values()
            .find(|u| u.email == email && u.password_hash == expected)
            .cloned()
            .ok_or(WalletError::AuthFailed)
    }

    pub fn open_account(&self, user_id: u64, currency: Currency) -> WalletResult<Account> {
        let id = {
            let mut users = wr(&self.users);
            let user = users
                .get_mut(&user_id)
                .ok_or(WalletError::UserNotFound(user_id))?;
            if user.accounts.contains_key(&currency) {
                return Err(WalletError::DuplicateCurrency(currency.to_string()));
            }
            let id = self.next_account.fetch_add(1, Ordering::Relaxed) + 1;
            user.accounts.insert(currency, id);
            id
        };
        let account = Account::new(id, user_id, currency);
        self.tx.insert_account(account.clone());
        Ok(account)
    }

    pub fn add_payment_method(
        &self,
        user_id: u64,
        method: PaymentMethod,
    ) -> WalletResult<PaymentMethod> {
        if !rd(&self.users).contains_key(&user_id) {
            return Err(WalletError::UserNotFound(user_id));
        }
        let id = self.next_method.fetch_add(1, Ordering::Relaxed) + 1;
        let method = method.with_id(id);
        wr(&self.payment_methods).insert(
            id,
            StoredMethod {
                owner: user_id,
                method: method.clone(),
            },
        );
        Ok(method)
    }

    pub fn payment_method(&self, id: u64) -> Option<PaymentMethod> {
        rd(&self.payment_methods)
            .get(&id)
            .map(|stored| stored.method.clone())
    }

    pub fn remove_payment_method(&self, id: u64) -> WalletResult<()> {
        wr(&self.payment_methods)
            .remove(&id)
            .map(|_| ())
            .ok_or(WalletError::PaymentMethodNotFound(id))
    }

    /// Fund an account from a method its own holder attached; otherwise a bare
    /// method id would be enough to charge someone else's card.
    pub fn fund_account(
        &self,
        account_id: u64,
        method_id: u64,
        amount: Money,
    ) -> WalletResult<Transaction> {
        let account = self
            .tx
            .account(account_id)
            .ok_or(WalletError::AccountNotFound(account_id))?;
        let label = {
            let methods = rd(&self.payment_methods);
            let stored = methods
                .get(&method_id)
                .ok_or(WalletError::PaymentMethodNotFound(method_id))?;
            if stored.owner != account.user_id {
                return Err(WalletError::MethodNotOwned {
                    method: method_id,
                    account: account_id,
                });
            }
            stored.method.label()
        };
        self.tx.deposit(account_id, amount, label)
    }
}

impl Default for DigitalWallet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CardDetails;
    use crate::Currency;

    fn card(wallet: &DigitalWallet, user_id: u64) -> PaymentMethod {
        wallet
            .add_payment_method(
                user_id,
                PaymentMethod::CreditCard(CardDetails::new(0, "4242", "tok", 27, 11)),
            )
            .unwrap()
    }

    #[test]
    fn test_duplicate_currency() {
        let wallet = DigitalWallet::new();
        let user = wallet.create_user("A", "a@x", "pw").unwrap();
        wallet.open_account(user.id, Currency::Usd).unwrap();
        assert!(wallet.open_account(user.id, Currency::Usd).is_err());
        assert!(wallet.open_account(user.id, Currency::Eur).is_ok());
    }

    #[test]
    fn test_empty_user() {
        let wallet = DigitalWallet::new();
        assert!(matches!(
            wallet.create_user("", "a@x", "pw"),
            Err(WalletError::InvalidInput(_))
        ));
    }

    #[test]
    fn test_authenticate() {
        let wallet = DigitalWallet::new();
        let user = wallet.create_user("Alice", "a@x", "pw").unwrap();
        assert_eq!(wallet.authenticate("a@x", "pw").unwrap().id, user.id);
        assert_eq!(
            wallet.authenticate("a@x", "nope").unwrap_err(),
            WalletError::AuthFailed
        );
        assert_eq!(
            wallet.authenticate("ghost@x", "pw").unwrap_err(),
            WalletError::AuthFailed
        );
    }

    #[test]
    fn test_unknown_method() {
        let wallet = DigitalWallet::new();
        let user = wallet.create_user("Alice", "a@x", "pw").unwrap();
        let account = wallet.open_account(user.id, Currency::Usd).unwrap();
        assert!(matches!(
            wallet.fund_account(account.id, 999, Money::minor_units(10, Currency::Usd)),
            Err(WalletError::PaymentMethodNotFound(999))
        ));
    }

    #[test]
    fn test_method_unowned() {
        let wallet = DigitalWallet::new();
        let alice = wallet.create_user("Alice", "a@x", "pw").unwrap();
        let bob = wallet.create_user("Bob", "b@x", "pw").unwrap();
        let alice_card = card(&wallet, alice.id);
        let bob_usd = wallet.open_account(bob.id, Currency::Usd).unwrap();
        assert_eq!(
            wallet
                .fund_account(
                    bob_usd.id,
                    alice_card.id(),
                    Money::minor_units(5_000, Currency::Usd)
                )
                .unwrap_err(),
            WalletError::MethodNotOwned {
                method: alice_card.id(),
                account: bob_usd.id,
            }
        );
        assert_eq!(wallet.tx.account(bob_usd.id).unwrap().balance, 0);
    }

    #[test]
    fn test_unknown_owner() {
        let wallet = DigitalWallet::new();
        assert!(matches!(
            wallet.add_payment_method(
                42,
                PaymentMethod::CreditCard(CardDetails::new(0, "4242", "tok", 27, 11))
            ),
            Err(WalletError::UserNotFound(42))
        ));
    }

    #[test]
    fn test_transfer_balances() {
        let wallet = DigitalWallet::new();
        let alice = wallet.create_user("Alice", "a@x", "pw").unwrap();
        let bob = wallet.create_user("Bob", "b@x", "pw").unwrap();
        let source = wallet.open_account(alice.id, Currency::Usd).unwrap();
        let dest = wallet.open_account(bob.id, Currency::Usd).unwrap();
        let card = card(&wallet, alice.id);
        wallet
            .fund_account(
                source.id,
                card.id(),
                Money::minor_units(5_000, Currency::Usd),
            )
            .unwrap();
        wallet
            .tx
            .transfer(source.id, dest.id, Money::minor_units(2_000, Currency::Usd))
            .unwrap();
        assert_eq!(wallet.tx.account(source.id).unwrap().balance, 3_000);
        assert_eq!(wallet.tx.account(dest.id).unwrap().balance, 2_000);
        assert_eq!(wallet.tx.history_for(source.id).unwrap().len(), 2);
    }

    #[test]
    fn test_conservation_contention() {
        let wallet = DigitalWallet::new();
        let alice = wallet.create_user("Alice", "a@x", "pw").unwrap();
        let bob = wallet.create_user("Bob", "b@x", "pw").unwrap();
        let source = wallet.open_account(alice.id, Currency::Usd).unwrap();
        let dest = wallet.open_account(bob.id, Currency::Usd).unwrap();
        let card = card(&wallet, alice.id);
        wallet
            .fund_account(
                source.id,
                card.id(),
                Money::minor_units(10_000, Currency::Usd),
            )
            .unwrap();
        let wallet = Arc::new(wallet);
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let wallet = wallet.clone();
                std::thread::spawn(move || {
                    wallet
                        .tx
                        .transfer(source.id, dest.id, Money::minor_units(1_000, Currency::Usd))
                        .is_ok()
                })
            })
            .collect();
        let landed = handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|ok| *ok)
            .count();
        assert_eq!(landed, 10);
        let source_balance = wallet.tx.account(source.id).unwrap().balance;
        let dest_balance = wallet.tx.account(dest.id).unwrap().balance;
        assert_eq!(source_balance + dest_balance, 10_000);
        assert_eq!(source_balance, 0);
    }

    #[test]
    fn test_concurrent_open() {
        let wallet = Arc::new(DigitalWallet::new());
        let user = wallet.create_user("Alice", "a@x", "pw").unwrap();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let wallet = wallet.clone();
                std::thread::spawn(move || wallet.open_account(user.id, Currency::Usd).is_ok())
            })
            .collect();
        let opened = handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|ok| *ok)
            .count();
        assert_eq!(opened, 1);
        assert_eq!(wallet.user(user.id).unwrap().accounts.len(), 1);
    }
}
