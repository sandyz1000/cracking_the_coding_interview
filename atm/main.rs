// Designing an ATM System
//
// ### Requirements
//
// - The ATM system should support basic operations such as balance inquiry, cash withdrawal, and cash deposit.
// - Users should be able to authenticate themselves using a card and a PIN (Personal Identification Number).
// - The system should interact with a bank's backend system to validate user accounts and perform transactions.
// - The ATM should have a cash dispenser to dispense cash to users.
// - The system should handle concurrent access and ensure data consistency.
// - The ATM should have a user-friendly interface for users to interact with.
//
// ### Classes, Interfaces and Enumerations
//
// - The Card class represents an ATM card with a card number and PIN.
// - The Account class represents a bank account with an account number and balance. It provides methods to debit and
// credit the account balance.
// - The Transaction class is an abstract base class for different types of transactions, such as withdrawal and deposit.
// - It is extended by WithdrawalTransaction and DepositTransaction classes.
// - The BankingService class manages the bank accounts and processes transactions. It uses a thread-safe ConcurrentHashMap
// to store and retrieve account information.
// - The CashDispenser class represents the ATM's cash dispenser and handles the dispensing of cash. It uses synchronization
// to ensure thread safety when dispensing cash.
// - The ATM class serves as the main interface for ATM operations. It interacts with the BankingService and CashDispenser
// to perform user authentication, balance inquiry, cash withdrawal, and cash deposit.
// - The ATMDriver class demonstrates the usage of the ATM system by creating sample accounts and performing ATM operations.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

// The requirements' Transaction/WithdrawalTransaction/DepositTransaction class
// hierarchy maps to an enum in Rust: exhaustive, no dynamic dispatch needed.
//
// Money is expressed in integer cents (smallest currency unit). f64 is wrong
// for money: 0.1 + 0.2 != 0.3 in binary floating point, and rounding errors
// accumulate across transactions.
#[derive(Debug, Clone)]
enum TransactionType {
    BalanceInquiry,
    DepositCash(i64),
    DepositCheck(i64),
    Withdraw(i64),
    Transfer(i64, u64), // Amount in cents, Destination Account ID
}

#[derive(Debug, Clone)]
enum AccountType {
    Saving,
    #[allow(unused)]
    Current,
    Checking,
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, w: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            AccountType::Saving => write!(w, "Saving"),
            AccountType::Checking => write!(w, "Checking"),
            AccountType::Current => write!(w, "Current"),
        }
    }
}

#[derive(Debug)]
struct Account {
    account_number: u64,
    balance: i64,          // cleared, spendable funds (cents)
    pending_deposits: i64, // checks deposited but not yet cleared (cents)
    account_type: AccountType,
}

impl Account {
    fn new(account_number: u64, account_type: AccountType) -> Self {
        Self {
            account_number,
            balance: 0,
            pending_deposits: 0,
            account_type,
        }
    }

    /// Immediate, spendable credit (cash deposit, transfer-in).
    fn credit(&mut self, amount: i64) -> anyhow::Result<()> {
        if amount < 0 {
            return Err(anyhow::anyhow!("cannot credit a negative amount"));
        }
        self.balance += amount;
        Ok(())
    }

    /// A check is NOT spendable until it clears: it only lands in the
    /// pending ledger. If the drawer's bank bounces it, the deposit is
    /// simply cancelled and nothing was ever spendable.
    fn deposit_check(&mut self, amount: i64) -> anyhow::Result<()> {
        if amount < 0 {
            return Err(anyhow::anyhow!("cannot deposit a negative check"));
        }
        self.pending_deposits += amount;
        Ok(())
    }

    /// Move a cleared amount from pending into the spendable balance.
    fn clear_pending(&mut self, amount: i64) -> anyhow::Result<()> {
        if amount < 0 {
            return Err(anyhow::anyhow!("cannot clear a negative amount"));
        }
        if amount > self.pending_deposits {
            return Err(anyhow::anyhow!("clearing more than the pending amount"));
        }
        self.pending_deposits -= amount;
        self.balance += amount;
        Ok(())
    }

    fn withdraw(&mut self, amount: i64) -> anyhow::Result<()> {
        if amount < 0 {
            return Err(anyhow::anyhow!("Cannot withdraw a negative amount"));
        }
        if self.balance >= amount {
            self.balance -= amount;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Insufficient funds"))
        }
    }
}

// A single customer can have many account
#[derive(Debug)]
struct Customer {
    name: String,
    accounts: HashMap<u64, Account>,
}

impl Customer {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            accounts: HashMap::new(),
        }
    }

    fn add_account(&mut self, account: Account) {
        self.accounts.insert(account.account_number, account);
    }
}

// NEW: Card + PIN authentication. A card is bound to exactly one customer.
#[derive(Debug, Clone)]
struct Card {
    card_number: u64,
    pin: u32,
    customer_id: u64,
}

impl Card {
    fn new(card_number: u64, pin: u32, customer_id: u64) -> Self {
        Self {
            card_number,
            pin,
            customer_id,
        }
    }
}

// The cash dispenser. Cash is a shared resource, so dispensing and receiving
// are serialized behind a Mutex — this satisfies the "cash dispenser uses
// synchronization" requirement. The reserve is tracked in cents.
struct CashDispenser {
    reserve: Mutex<i64>,
}

impl CashDispenser {
    fn new(initial_reserve_cents: i64) -> Self {
        Self {
            reserve: Mutex::new(initial_reserve_cents),
        }
    }

    fn dispense(&self, amount_cents: i64) -> anyhow::Result<()> {
        let mut reserve = self.reserve.lock().unwrap();
        if *reserve < amount_cents {
            return Err(anyhow::anyhow!("ATM has insufficient cash reserve"));
        }
        *reserve -= amount_cents;
        Ok(())
    }

    fn receive(&self, amount_cents: i64) {
        *self.reserve.lock().unwrap() += amount_cents;
    }

    fn reserve(&self) -> i64 {
        *self.reserve.lock().unwrap()
    }
}

// Named "ATM" to match the problem statement's class list; clippy prefers
// the lower-cased acronym spelling, which would break the spec's terminology.
#[allow(clippy::upper_case_acronyms)]
struct ATM {
    customers: HashMap<u64, Customer>, // Maps customer ID to Customer
    cards: HashMap<u64, Card>,         // Maps card number to Card
    dispenser: CashDispenser,
    next_customer_id: u64,
    next_account_id: u64,
}

impl ATM {
    fn new() -> Self {
        Self {
            customers: HashMap::new(),
            cards: HashMap::new(),
            dispenser: CashDispenser::new(1_000_000), // ₹10,000.00 in cents
            next_customer_id: 0,
            next_account_id: 0,
        }
    }

    fn register_customer(&mut self, name: &str) -> u64 {
        self.next_customer_id += 1;
        let id = self.next_customer_id;
        self.customers.insert(id, Customer::new(name));
        id
    }

    fn open_account(&mut self, customer_id: u64, account_type: AccountType) -> anyhow::Result<u64> {
        self.next_account_id += 1;
        let account_number = self.next_account_id;
        let customer = self
            .customers
            .get_mut(&customer_id)
            .ok_or(anyhow::anyhow!("Customer not found"))?;
        customer.add_account(Account::new(account_number, account_type));
        Ok(account_number)
    }

    fn issue_card(
        &mut self,
        card_number: u64,
        pin: u32,
        customer_id: u64,
    ) -> Result<(), &'static str> {
        if !self.customers.contains_key(&customer_id) {
            return Err("Customer not found");
        }
        self.cards
            .insert(card_number, Card::new(card_number, pin, customer_id));
        Ok(())
    }

    // NEW: the authentication step from the requirements — card + PIN.
    fn authenticate(&self, card_number: u64, pin: u32) -> anyhow::Result<&Card> {
        match self.cards.get(&card_number) {
            Some(card) if card.pin == pin => Ok(card),
            Some(_) | None => Err(anyhow::anyhow!("Invalid card/PIN: {card_number}")),
        }
    }

    /// Simulate the clearinghouse releasing all pending checks on an account
    /// (in reality this is a batch job driven by the clearing cycle, not the
    /// customer). Returns the amount now spendable.
    fn clear_checks(&mut self, customer_id: u64, account_id: u64) -> anyhow::Result<i64> {
        let customer = self
            .customers
            .get_mut(&customer_id)
            .ok_or(anyhow::anyhow!("Customer not found"))?;
        let account = customer
            .accounts
            .get_mut(&account_id)
            .ok_or(anyhow::anyhow!("Account not found"))?;
        let amount = account.pending_deposits;
        account.clear_pending(amount)?;
        Ok(amount)
    }

    fn perform_transaction(
        &mut self,
        card: &Card,
        account_id: u64,
        transaction: TransactionType,
    ) -> anyhow::Result<i64> {
        let customer = self
            .customers
            .get_mut(&card.customer_id)
            .ok_or(anyhow::anyhow!("Customer not found"))?;

        match transaction {
            TransactionType::BalanceInquiry => {
                let account = customer
                    .accounts
                    .get(&account_id)
                    .ok_or(anyhow::anyhow!("Account not found"))?;
                Ok(account.balance)
            }
            TransactionType::DepositCash(amount) => {
                let account = customer
                    .accounts
                    .get_mut(&account_id)
                    .ok_or(anyhow::anyhow!("Account not found"))?;
                account.credit(amount)?;
                // Cash physically enters the machine.
                self.dispenser.receive(amount);
                Ok(account.balance)
            }
            TransactionType::DepositCheck(amount) => {
                let account = customer
                    .accounts
                    .get_mut(&account_id)
                    .ok_or(anyhow::anyhow!("Account not found"))?;
                // Goes to the pending ledger: not spendable until cleared.
                account.deposit_check(amount)?;
                Ok(account.balance)
            }
            TransactionType::Withdraw(amount) => {
                // Dispense the cash first; only debit the account once the
                // machine is guaranteed to hand the money over. A dispenser
                // failure then leaves the account untouched.
                self.dispenser.dispense(amount)?;
                let account = customer
                    .accounts
                    .get_mut(&account_id)
                    .ok_or(anyhow::anyhow!("Account not found"))?;
                account.withdraw(amount)?;
                Ok(account.balance)
            }
            TransactionType::Transfer(amount, dest_account_id) => {
                if account_id == dest_account_id {
                    return Err(anyhow::anyhow!("Cannot transfer to the same account"));
                }
                // std::HashMap::get_many_mut is nightly-only, so do the two
                // lookups sequentially. The customer is already behind a Mutex
                // and the whole ATM behind another, so no other thread can
                // interleave between the debit and the credit.
                let source = customer
                    .accounts
                    .get_mut(&account_id)
                    .ok_or(anyhow::anyhow!("Source account not found"))?;
                source.withdraw(amount)?;
                let balance = source.balance;
                let dest = customer
                    .accounts
                    .get_mut(&dest_account_id)
                    .ok_or(anyhow::anyhow!("Destination account not found"))?;
                dest.credit(amount)?;
                Ok(balance)
            }
        }
    }
}

struct ATMDriver;

impl ATMDriver {
    fn run() {
        let atm = Arc::new(Mutex::new(ATM::new()));

        // ---- Seed: two customers, their accounts and cards ----
        let (alice_checking, alice_savings, bob_checking) = {
            let mut atm = atm.lock().unwrap();
            let alice = atm.register_customer("Alice");
            let alice_checking = atm.open_account(alice, AccountType::Checking).unwrap();
            let alice_savings = atm.open_account(alice, AccountType::Saving).unwrap();
            atm.issue_card(1111_2222_3333_4444, 1234, alice).unwrap();

            let bob = atm.register_customer("Bob");
            let bob_checking = atm.open_account(bob, AccountType::Checking).unwrap();
            atm.issue_card(5555_6666_7777_8888, 4321, bob).unwrap();

            println!(
                "Seeded: Alice(Checking {alice_checking}, Savings {alice_savings}), Bob(Checking {bob_checking}) — card 1111..4444 PIN 1234, card 5555..8888 PIN 4321"
            );
            (alice_checking, alice_savings, bob_checking)
        };

        let (alice_card, bob_card) = {
            let guard = atm.lock().unwrap();
            (
                guard
                    .authenticate(1111_2222_3333_4444, 1234)
                    .unwrap()
                    .clone(),
                guard
                    .authenticate(5555_6666_7777_8888, 4321)
                    .unwrap()
                    .clone(),
            )
        };
        println!(
            "  authenticated cards: {} (Alice), {} (Bob)",
            alice_card.card_number, bob_card.card_number
        );

        // ---- Sequential operations ----
        let run_op = |atm: &mut ATM, card: &Card, account: u64, tx: TransactionType| {
            println!(
                "  {:?} → {:?}",
                tx,
                atm.perform_transaction(card, account, tx.clone())
            )
        };
        println!("\n== Sequential ops (Alice's Checking = {alice_checking}) ==");
        let mut guard = atm.lock().unwrap();
        run_op(
            &mut guard,
            &alice_card,
            alice_checking,
            TransactionType::DepositCash(50_000),
        ); // ₹500.00
        run_op(
            &mut guard,
            &alice_card,
            alice_checking,
            TransactionType::BalanceInquiry,
        );
        run_op(
            &mut guard,
            &alice_card,
            alice_checking,
            TransactionType::Withdraw(20_000),
        ); // ₹200.00
        run_op(
            &mut guard,
            &alice_card,
            alice_checking,
            TransactionType::Transfer(10_000, alice_savings),
        ); // ₹100.00
        run_op(
            &mut guard,
            &bob_card,
            bob_checking,
            TransactionType::DepositCash(100_000),
        ); // ₹1,000.00
        run_op(
            &mut guard,
            &bob_card,
            bob_checking,
            TransactionType::DepositCheck(25_000),
        ); // ₹250.00
        {
            let bob_account = guard
                .customers
                .get(&2)
                .unwrap()
                .accounts
                .get(&bob_checking)
                .unwrap();
            println!(
                "  Bob's Checking ({bob_checking}) after CHECK deposit → cleared {}, pending {}",
                fmt_money(bob_account.balance),
                fmt_money(bob_account.pending_deposits)
            );
        }
        // The check has NOT cleared: Bob cannot spend it yet.
        run_op(
            &mut guard,
            &bob_card,
            bob_checking,
            TransactionType::Withdraw(110_000),
        ); // ₹1,100.00 — more than cleared ₹1,000.00 → must fail
        // Clearinghouse releases the pending check; only now is it spendable.
        let cleared = guard.clear_checks(2, bob_checking).unwrap();
        println!("  clearinghouse cleared {} → spendable", fmt_money(cleared));
        run_op(
            &mut guard,
            &bob_card,
            bob_checking,
            TransactionType::Withdraw(110_000),
        ); // now succeeds: ₹1,000 cleared + ₹250 cleared check
        println!("  cash reserve: {}", fmt_money(guard.dispenser.reserve()));
        drop(guard);

        let seeded_total = {
            let guard = atm.lock().unwrap();
            let balances: i64 = guard
                .customers
                .values()
                .flat_map(|customer| customer.accounts.values())
                .map(|account| account.balance)
                .sum();
            balances + guard.dispenser.reserve()
        };
        println!(
            "  total (balances + reserve) before concurrent phase: {}",
            fmt_money(seeded_total)
        );

        // ---- Concurrency: many threads, one ATM ----
        // Only transfers and inquiries here: a transfer moves money between
        // accounts without crossing the ATM boundary, so the invariant
        // sum(balances) + cash reserve is exactly conserved. (Deposits and
        // withdrawals inject/remove cash into/from the machine, which is why
        // the closed-system check must exclude them.)
        println!("\n== Concurrent ops: 8 threads x 200 transfers/inquiries ==");
        let handles: Vec<_> = (0..8)
            .map(|t| {
                let atm = Arc::clone(&atm);
                let card = alice_card.clone();
                thread::spawn(move || {
                    for i in 0..200 {
                        let mut atm = atm.lock().unwrap();
                        let (source, dest) = if (i + t) % 2 == 0 {
                            (alice_checking, alice_savings)
                        } else {
                            (alice_savings, alice_checking)
                        };
                        let tx = if (i + t) % 3 == 0 {
                            TransactionType::BalanceInquiry
                        } else {
                            TransactionType::Transfer(500, dest) // ₹5.00
                        };
                        let _ = atm.perform_transaction(&card, source, tx);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let guard = atm.lock().unwrap();
        let balances: i64 = guard
            .customers
            .values()
            .flat_map(|customer| customer.accounts.values())
            .map(|account| account.balance)
            .sum();
        let reserve = guard.dispenser.reserve();
        println!("  sum of balances: {}", fmt_money(balances));
        println!("  cash reserve:    {}", fmt_money(reserve));
        println!("  total:           {}", fmt_money(balances + reserve));
        println!("\n  Final account states:");
        for customer in guard.customers.values() {
            println!("  customer: {}", customer.name);
            for account in customer.accounts.values() {
                println!(
                    "    account {} ({}) : {} (+{} pending)",
                    account.account_number,
                    account.account_type,
                    fmt_money(account.balance),
                    fmt_money(account.pending_deposits)
                );
            }
        }
        assert_eq!(
            balances + reserve,
            seeded_total,
            "money conservation violated: {} != {}",
            fmt_money(balances + reserve),
            fmt_money(seeded_total)
        );
        println!("  money conservation: OK");
    }
}

fn main() {
    ATMDriver::run();
}

/// Format an integer cent amount as ₹X.YY (non-negative values only).
fn fmt_money(cents: i64) -> String {
    format!("₹{}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncleared_check_is_not_spendable() {
        let mut account = Account::new(1, AccountType::Checking);
        account.credit(100_00).unwrap();
        account.deposit_check(50_00).unwrap();

        // ₹100 cleared; the ₹50 check has not cleared.
        assert!(account.withdraw(100_00).is_ok());
        assert!(account.withdraw(50_00).is_err());
        assert_eq!(account.pending_deposits, 50_00);
    }

    #[test]
    fn cleared_check_becomes_spendable() {
        let mut account = Account::new(1, AccountType::Checking);
        account.deposit_check(50_00).unwrap();
        assert!(account.withdraw(50_00).is_err()); // pending, not cleared

        account.clear_pending(50_00).unwrap();
        assert_eq!(account.pending_deposits, 0);
        assert_eq!(account.balance, 50_00);
        assert!(account.withdraw(50_00).is_ok()); // spendable now
    }

    #[test]
    fn cannot_clear_more_than_pending() {
        let mut account = Account::new(1, AccountType::Checking);
        account.deposit_check(10_00).unwrap();
        assert!(account.clear_pending(20_00).is_err());
        assert_eq!(account.pending_deposits, 10_00);
    }

    #[test]
    fn negative_amounts_are_rejected() {
        let mut account = Account::new(1, AccountType::Checking);
        assert!(account.credit(-1).is_err());
        assert!(account.deposit_check(-1).is_err());
        assert!(account.clear_pending(-1).is_err());
        assert!(account.withdraw(-1).is_err());
    }
}
