//! Demo driver for the digital wallet service.

use digital_wallet::domain::payments::{BankDetails, CardDetails, PaymentMethod};
use digital_wallet::money::Money;
use digital_wallet::{Currency, DigitalWallet, TransactionKind, WalletError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wallet = DigitalWallet::new();

    let alice = wallet.create_user("Alice", "alice@example.com", "s3cret")?;
    let bob = wallet.create_user("Bob", "bob@example.com", "hunter2")?;

    let alice_card = wallet.add_payment_method(
        alice.id,
        PaymentMethod::CreditCard(CardDetails::new(0, "4242", "tok_alice", 27, 11)),
    )?;
    wallet.add_payment_method(
        bob.id,
        PaymentMethod::BankAccount(BankDetails::new(0, "9988", "rt_bob")),
    )?;

    let alice_eur = wallet.open_account(alice.id, Currency::Eur)?;
    let bob_usd = wallet.open_account(bob.id, Currency::Usd)?;

    wallet.fund_account(
        alice_eur.id,
        alice_card.id(),
        Money::from_parts(1_000, 0, Currency::Eur),
    )?;
    wallet.tx.transfer(
        alice_eur.id,
        bob_usd.id,
        Money::from_parts(100, 0, Currency::Eur),
    )?;

    for tx in wallet.tx.history_for(alice_eur.id)? {
        match tx.kind {
            TransactionKind::Deposit { method_label } => {
                println!("tx#{} in  {} from {method_label}", tx.id, tx.amount)
            }
            TransactionKind::Transfer { credited } => {
                println!("tx#{} out {} (recipient got {credited})", tx.id, tx.amount)
            }
        }
    }

    match wallet.tx.transfer(
        bob_usd.id,
        alice_eur.id,
        Money::from_parts(999, 0, Currency::Usd),
    ) {
        Err(WalletError::InsufficientFunds) => println!("demo: bob cannot overdraw (rejected)"),
        other => println!("unexpected: {other:?}"),
    }
    match wallet.tx.transfer(
        alice_eur.id,
        bob_usd.id,
        Money::from_parts(10, 0, Currency::Inr),
    ) {
        Err(e @ WalletError::CurrencyMismatch { .. }) => println!("demo: {e} (rejected)"),
        other => println!("unexpected: {other:?}"),
    }

    Ok(())
}
