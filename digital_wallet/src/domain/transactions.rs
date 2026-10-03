//! Transactions and the atomic transfer engine. See DESIGN.md.

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::accounts::Account;
use crate::error::{WalletError, WalletResult};
use crate::locks::{rd, wr};
use crate::money::{CurrencyConverter, Money};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionKind {
    Deposit { method_label: String },
    Transfer { credited: Money },
}

/// `amount` is always what left `source_account`, in that account's currency.
#[derive(Clone, Debug)]
pub struct Transaction {
    pub id: u64,
    pub source_account: u64,
    pub dest_account: Option<u64>,
    pub amount: Money,
    pub timestamp: u64,
    pub kind: TransactionKind,
}

impl Transaction {
    pub fn has_account(&self, account_id: u64) -> bool {
        self.source_account == account_id || self.dest_account == Some(account_id)
    }
}

/// Owns the account ledger and the transaction history. See DESIGN.md.
pub struct TransactionManager {
    accounts: RwLock<HashMap<u64, Account>>,
    history: RwLock<Vec<Transaction>>,
    next_transaction: AtomicU64,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            history: RwLock::new(Vec::new()),
            next_transaction: AtomicU64::new(0),
        }
    }

    pub fn insert_account(&self, account: Account) {
        wr(&self.accounts).insert(account.id, account);
    }

    pub fn account(&self, id: u64) -> Option<Account> {
        rd(&self.accounts).get(&id).cloned()
    }

    /// Called under the account write lock, so log order is commit order.
    fn record(
        &self,
        source: u64,
        dest: Option<u64>,
        amount: Money,
        kind: TransactionKind,
    ) -> Transaction {
        let tx = Transaction {
            id: self.next_transaction.fetch_add(1, Ordering::Relaxed) + 1,
            source_account: source,
            dest_account: dest,
            amount,
            timestamp: now_millis(),
            kind,
        };
        wr(&self.history).push(tx.clone());
        tx
    }

    pub fn deposit(
        &self,
        account_id: u64,
        amount: Money,
        method_label: String,
    ) -> WalletResult<Transaction> {
        let mut accounts = wr(&self.accounts);
        let account = accounts
            .get_mut(&account_id)
            .ok_or(WalletError::AccountNotFound(account_id))?;
        account.balance = account.balance_after_credit(amount)?;
        Ok(self.record(
            account_id,
            None,
            amount,
            TransactionKind::Deposit { method_label },
        ))
    }

    /// `amount` is in the source account's currency; the destination is
    /// credited the converted equivalent.
    pub fn transfer(
        &self,
        source_account: u64,
        dest_account: u64,
        amount: Money,
    ) -> WalletResult<Transaction> {
        if source_account == dest_account {
            return Err(WalletError::SelfTransfer);
        }
        let mut accounts = wr(&self.accounts);
        let [Some(source), Some(dest)] =
            accounts.get_disjoint_mut([&source_account, &dest_account])
        else {
            let missing = match accounts.contains_key(&source_account) {
                true => dest_account,
                false => source_account,
            };
            return Err(WalletError::AccountNotFound(missing));
        };

        // Both balances are decided before either is written, so no failure
        // can leave one leg applied.
        let source_balance = source.balance_after_debit(amount)?;
        let credited = CurrencyConverter::convert(&amount, dest.currency)?;
        let dest_balance = dest.balance_after_credit(credited)?;
        source.balance = source_balance;
        dest.balance = dest_balance;

        Ok(self.record(
            source_account,
            Some(dest_account),
            amount,
            TransactionKind::Transfer { credited },
        ))
    }

    /// Statement of one account's movements, oldest first.
    pub fn history_for(&self, account_id: u64) -> WalletResult<Vec<Transaction>> {
        if !rd(&self.accounts).contains_key(&account_id) {
            return Err(WalletError::AccountNotFound(account_id));
        }
        Ok(rd(&self.history)
            .iter()
            .filter(|tx| tx.has_account(account_id))
            .cloned()
            .collect())
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Currency;
    use std::sync::Arc;

    fn ledger() -> TransactionManager {
        let tx = TransactionManager::new();
        tx.insert_account(Account::new(1, 1, Currency::Usd));
        tx.insert_account(Account::new(2, 2, Currency::Usd));
        tx
    }

    fn balance(tx: &TransactionManager, id: u64) -> i64 {
        tx.account(id).unwrap().balance
    }

    #[test]
    fn test_transfer_overdraft() {
        let tx = ledger();
        assert_eq!(
            tx.transfer(1, 2, Money::minor_units(100, Currency::Usd))
                .unwrap_err(),
            WalletError::InsufficientFunds
        );
    }

    #[test]
    fn test_transfer_balances() {
        let tx = ledger();
        tx.deposit(1, Money::minor_units(5_000, Currency::Usd), "card".into())
            .unwrap();
        tx.transfer(1, 2, Money::minor_units(2_000, Currency::Usd))
            .unwrap();
        assert_eq!(balance(&tx, 1), 3_000);
        assert_eq!(balance(&tx, 2), 2_000);
    }

    #[test]
    fn test_cross_currency() {
        let tx = TransactionManager::new();
        tx.insert_account(Account::new(1, 1, Currency::Usd));
        tx.insert_account(Account::new(2, 2, Currency::Inr));
        tx.deposit(1, Money::minor_units(10_000, Currency::Usd), "card".into())
            .unwrap();
        let moved = tx
            .transfer(1, 2, Money::minor_units(1_000, Currency::Usd))
            .unwrap();
        assert_eq!(balance(&tx, 1), 9_000);
        assert_eq!(balance(&tx, 2), 83_000);
        assert_eq!(moved.amount, Money::minor_units(1_000, Currency::Usd));
        assert_eq!(
            moved.kind,
            TransactionKind::Transfer {
                credited: Money::minor_units(83_000, Currency::Inr)
            }
        );
    }

    #[test]
    fn test_unknown_account() {
        let tx = ledger();
        assert_eq!(
            tx.transfer(1, 99, Money::minor_units(100, Currency::Usd))
                .unwrap_err(),
            WalletError::AccountNotFound(99)
        );
    }

    #[test]
    fn test_deposit_mismatch() {
        let tx = ledger();
        assert!(matches!(
            tx.deposit(1, Money::minor_units(10_000, Currency::Inr), "card".into()),
            Err(WalletError::CurrencyMismatch { .. })
        ));
        assert_eq!(balance(&tx, 1), 0);
    }

    #[test]
    fn test_transfer_mismatch() {
        let tx = ledger();
        tx.deposit(1, Money::minor_units(10_000, Currency::Usd), "card".into())
            .unwrap();
        assert!(matches!(
            tx.transfer(1, 2, Money::minor_units(8_300, Currency::Inr)),
            Err(WalletError::CurrencyMismatch { .. })
        ));
        assert_eq!(balance(&tx, 1) + balance(&tx, 2), 10_000);
    }

    #[test]
    fn test_statement_legs() {
        let tx = ledger();
        tx.deposit(1, Money::minor_units(5_000, Currency::Usd), "card".into())
            .unwrap();
        tx.transfer(1, 2, Money::minor_units(2_000, Currency::Usd))
            .unwrap();
        assert_eq!(tx.history_for(1).unwrap().len(), 2);
        assert_eq!(tx.history_for(2).unwrap().len(), 1);
    }

    #[test]
    fn test_concurrent_overdraft() {
        let tx = ledger();
        tx.deposit(1, Money::minor_units(10_000, Currency::Usd), "card".into())
            .unwrap();
        let tx = Arc::new(tx);
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    tx.transfer(1, 2, Money::minor_units(1_000, Currency::Usd))
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
        assert_eq!(balance(&tx, 1), 0);
        assert_eq!(balance(&tx, 2), 10_000);
        let ids: Vec<u64> = tx.history_for(2).unwrap().iter().map(|t| t.id).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
    }
}
