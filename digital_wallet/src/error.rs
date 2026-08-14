use thiserror::Error;

use crate::money::Currency;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WalletError {
    #[error("user {0} not found")]
    UserNotFound(u64),
    #[error("account {0} not found")]
    AccountNotFound(u64),
    #[error("payment method {0} not found")]
    PaymentMethodNotFound(u64),
    #[error("payment method {method} is not owned by the holder of account {account}")]
    MethodNotOwned { method: u64, account: u64 },
    #[error("invalid email or password")]
    AuthFailed,
    #[error("user already has account in currency {0}")]
    DuplicateCurrency(String),
    #[error("cannot transfer to the same account")]
    SelfTransfer,
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("amount is in {got} but the account is denominated in {expected}")]
    CurrencyMismatch { expected: Currency, got: Currency },
    #[error("invalid amount")]
    InvalidAmount,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("could not convert amount: {0}")]
    ConversionOverflow(String),
    #[error("balance overflow on account {0}")]
    BalanceOverflow(u64),
}

pub type WalletResult<T> = Result<T, WalletError>;
