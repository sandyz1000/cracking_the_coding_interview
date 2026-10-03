//! Payment methods backed by a card or a linked bank account. See DESIGN.md.

#[derive(Clone, Debug)]
pub enum PaymentMethod {
    CreditCard(CardDetails),
    BankAccount(BankDetails),
}

impl PaymentMethod {
    pub fn id(&self) -> u64 {
        match self {
            PaymentMethod::CreditCard(card) => card.id,
            PaymentMethod::BankAccount(bank) => bank.id,
        }
    }

    pub fn with_id(self, id: u64) -> Self {
        match self {
            PaymentMethod::CreditCard(mut card) => {
                card.id = id;
                PaymentMethod::CreditCard(card)
            }
            PaymentMethod::BankAccount(mut bank) => {
                bank.id = id;
                PaymentMethod::BankAccount(bank)
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            PaymentMethod::CreditCard(card) => format!("Card •••• {}", card.last_four),
            PaymentMethod::BankAccount(bank) => format!("Bank •••• {}", bank.last_four),
        }
    }
}

/// No PAN, CVV, or full expiry is retained.
#[derive(Clone, Debug)]
pub struct CardDetails {
    pub id: u64,
    pub last_four: String,
    /// What the processor charges; the card number never lands here.
    pub processor_token: String,
    pub expiry_yy: u8,
    pub expiry_mm: u8,
}

impl CardDetails {
    pub fn new(
        id: u64,
        last_four: &str,
        processor_token: &str,
        expiry_yy: u8,
        expiry_mm: u8,
    ) -> Self {
        Self {
            id,
            last_four: last_four.to_string(),
            processor_token: processor_token.to_string(),
            expiry_yy,
            expiry_mm,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BankDetails {
    pub id: u64,
    pub last_four: String,
    pub routing_code: String,
}

impl BankDetails {
    pub fn new(id: u64, last_four: &str, routing_code: &str) -> Self {
        Self {
            id,
            last_four: last_four.to_string(),
            routing_code: routing_code.to_string(),
        }
    }
}
