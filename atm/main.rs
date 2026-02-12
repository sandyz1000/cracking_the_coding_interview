use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone)]
enum TransactionType {
    BalanceInquiry,
    DepositCash(f64),
    DepositCheck(f64),
    Withdraw(f64),
    Transfer(f64, u64), // Amount, Destination Account ID
}

#[derive(Debug)]
struct Account {
    account_number: u64,
    balance: f64,
    account_type: String,
}

impl Account {
    fn new(account_number: u64, account_type: &str) -> Self {
        Self {
            account_number,
            balance: 0.0,
            account_type: account_type.to_string(),
        }
    }

    fn deposit(&mut self, amount: f64) {
        self.balance += amount;
    }

    fn withdraw(&mut self, amount: f64) -> Result<(), &'static str> {
        if self.balance >= amount {
            self.balance -= amount;
            Ok(())
        } else {
            Err("Insufficient funds")
        }
    }

    fn transfer(&mut self, amount: f64, recipient: &mut Account) -> Result<(), &'static str> {
        if self.balance >= amount {
            self.balance -= amount;
            recipient.deposit(amount);
            Ok(())
        } else {
            Err("Insufficient funds")
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

struct ATM {
    customers: HashMap<u64, Customer>, // Maps customer ID to Customer
    cash_reserve: f64,
}

impl ATM {
    fn new() -> Self {
        Self {
            customers: HashMap::new(),
            cash_reserve: 10000.0,
        }
    }

    fn authenticate_customer(&self, customer_id: u64) -> Option<&Customer> {
        self.customers.get(&customer_id)
    }

    fn perform_transaction(
        &mut self,
        customer_id: u64,
        account_id: u64,
        transaction: TransactionType,
    ) -> Result<f64, &'static str> {
        let customer = self
            .customers
            .get_mut(&customer_id)
            .ok_or("Customer not found")?;

        match transaction {
            TransactionType::BalanceInquiry => {
                let account = customer
                    .accounts
                    .get(&account_id)
                    .ok_or("Account not found")?;
                Ok(account.balance)
            }
            TransactionType::DepositCash(amount) | TransactionType::DepositCheck(amount) => {
                let account = customer
                    .accounts
                    .get_mut(&account_id)
                    .ok_or("Account not found")?;
                account.deposit(amount);
                Ok(account.balance)
            }
            TransactionType::Withdraw(amount) => {
                if amount > self.cash_reserve {
                    return Err("ATM has insufficient cash reserve");
                }
                let account = customer
                    .accounts
                    .get_mut(&account_id)
                    .ok_or("Account not found")?;
                account.withdraw(amount)?;
                self.cash_reserve -= amount;
                Ok(account.balance)
            }
            TransactionType::Transfer(amount, dest_account_id) => {
                if account_id == dest_account_id {
                    return Err("Cannot transfer to the same account");
                }
                let (source_account, dest_account) =
                    get_two_mut_accounts(&customer.accounts, account_id, dest_account_id)?;
                source_account.transfer(amount, dest_account)?;
                Ok(source_account.balance)
            }
        }
    }
}

fn get_two_mut_accounts<'a>(
    accounts: &'a HashMap<u64, Account>,
    id1: u64,
    id2: u64,
) -> Result<(&'a mut Account, &'a mut Account), &'static str> {
    let account1 = accounts.get(&id1).ok_or("Source account not found")?;
    let account2 = accounts.get(&id2).ok_or("Destination account not found")?;

    if std::ptr::eq(account1, account2) {
        panic!("Two account cannot be same")
    }
    // SAFETY: We know these are different accounts because we checked earlier
    unsafe {
        let (account1_ptr, account2_ptr): (*mut Account, *mut Account) = {
            (
                account1 as *const Account as *mut Account,
                account2 as *const Account as *mut Account,
            )
        };
        Ok((&mut *account1_ptr, &mut *account2_ptr))
    }
}

fn main() {
    let mut atm = ATM::new();
    let mut customer = Customer::new("Alice");
    customer.add_account(Account::new(1001, "Checking"));
    atm.customers.insert(1, customer);

    match atm.perform_transaction(1, 1001, TransactionType::DepositCash(500.0)) {
        Ok(balance) => println!("Transaction successful. New balance: {}", balance),
        Err(e) => println!("Transaction failed: {}", e),
    }
}
