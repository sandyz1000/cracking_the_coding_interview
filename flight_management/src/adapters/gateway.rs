use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::payments::{PaymentGateway, PaymentMethod};

/// Simulates an external PSP. Swap for a real provider; the trait boundary
/// is the only thing the domain sees.
#[derive(Default)]
pub struct MockGateway {
    next_txn: AtomicU64,
}

impl PaymentGateway for MockGateway {
    fn charge(&self, _amount: f64, _method: PaymentMethod) -> std::result::Result<String, String> {
        Ok(format!(
            "TXN-{:06}",
            self.next_txn.fetch_add(1, Ordering::Relaxed) + 1
        ))
    }

    fn refund(&self, _txn_id: &str, _amount: f64) -> std::result::Result<(), String> {
        Ok(())
    }
}
