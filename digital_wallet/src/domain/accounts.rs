//! User and account entities. See DESIGN.md.

use std::collections::HashMap;

use crate::error::{WalletError, WalletResult};
use crate::money::{Currency, Money};

#[derive(Clone, Debug)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    /// Keyed by currency, so one-account-per-currency is the data structure.
    pub accounts: HashMap<Currency, u64>,
}

impl User {
    pub fn new(id: u64, name: &str, email: &str, password_hash: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            accounts: HashMap::new(),
        }
    }
}

/// A single-denomination wallet: one account holds one currency.
#[derive(Clone, Debug)]
pub struct Account {
    pub id: u64,
    pub user_id: u64,
    pub currency: Currency,
    pub balance: i64,
}

impl Account {
    pub fn new(id: u64, user_id: u64, currency: Currency) -> Self {
        Self {
            id,
            user_id,
            currency,
            balance: 0,
        }
    }

    pub fn is_denominated(&self, amount: Money) -> bool {
        amount.currency == self.currency
    }

    /// Balance after crediting `amount`, without applying it.
    pub fn balance_after_credit(&self, amount: Money) -> WalletResult<i64> {
        self.check(amount)?;
        self.balance
            .checked_add(amount.amount)
            .ok_or(WalletError::BalanceOverflow(self.id))
    }

    /// Balance after debiting `amount`, without applying it.
    pub fn balance_after_debit(&self, amount: Money) -> WalletResult<i64> {
        self.check(amount)?;
        if self.balance < amount.amount {
            return Err(WalletError::InsufficientFunds);
        }
        Ok(self.balance - amount.amount)
    }

    fn check(&self, amount: Money) -> WalletResult<()> {
        if !self.is_denominated(amount) {
            return Err(WalletError::CurrencyMismatch {
                expected: self.currency,
                got: amount.currency,
            });
        }
        if amount.amount <= 0 {
            return Err(WalletError::InvalidAmount);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usd(balance: i64) -> Account {
        let mut account = Account::new(1, 1, Currency::Usd);
        account.balance = balance;
        account
    }

    #[test]
    fn test_credit_mismatch() {
        assert_eq!(
            usd(0).balance_after_credit(Money::minor_units(100, Currency::Inr)),
            Err(WalletError::CurrencyMismatch {
                expected: Currency::Usd,
                got: Currency::Inr,
            })
        );
    }

    #[test]
    fn test_debit_mismatch() {
        assert!(matches!(
            usd(10_000).balance_after_debit(Money::minor_units(100, Currency::Eur)),
            Err(WalletError::CurrencyMismatch { .. })
        ));
    }

    #[test]
    fn test_debit_overdraft() {
        assert_eq!(
            usd(50).balance_after_debit(Money::minor_units(51, Currency::Usd)),
            Err(WalletError::InsufficientFunds)
        );
    }

    #[test]
    fn test_credit_overflow() {
        assert_eq!(
            usd(i64::MAX).balance_after_credit(Money::minor_units(1, Currency::Usd)),
            Err(WalletError::BalanceOverflow(1))
        );
    }

    #[test]
    fn test_nonpositive_rejected() {
        assert_eq!(
            usd(100).balance_after_credit(Money::minor_units(0, Currency::Usd)),
            Err(WalletError::InvalidAmount)
        );
        assert_eq!(
            usd(100).balance_after_debit(Money::minor_units(-5, Currency::Usd)),
            Err(WalletError::InvalidAmount)
        );
    }
}
