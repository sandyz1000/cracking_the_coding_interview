//! Digital wallet service.
//!
//! Spec: `readme.md`. Design and concurrency decisions: `DESIGN.md`.

pub mod domain;
pub mod error;
pub(crate) mod locks;
pub mod money;

pub use domain::accounts::{Account, User};
pub use domain::payments::{BankDetails, CardDetails, PaymentMethod};
pub use domain::transactions::{Transaction, TransactionKind, TransactionManager};
pub use domain::wallet::DigitalWallet;
pub use error::{WalletError, WalletResult};
pub use money::{Currency, CurrencyConverter, Money};
