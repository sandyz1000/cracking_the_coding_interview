use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{CarError, CarResult};
use crate::locks::{rd, wr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaymentMethod {
    CreditCard,
    PayPal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaymentStatus {
    Pending,
    Completed,
    PartiallyRefunded,
    Refunded,
    Failed,
}

#[derive(Clone, Debug)]
pub struct Payment {
    pub payment_id: u64,
    pub reservation_number: String,
    pub amount: u32,
    pub refunded: u32,
    pub method: PaymentMethod,
    pub txn_id: Option<String>,
    pub status: PaymentStatus,
}

/// Extension seam: swap the implementation for a real PSP without touching
/// the domain. Charge/refund errors are provider strings, mapped to
/// `CarError` at the boundary.
pub trait PaymentGateway: Send + Sync {
    fn charge(&self, amount: u32, method: PaymentMethod) -> std::result::Result<String, String>;
    fn refund(&self, txn_id: &str, amount: u32) -> std::result::Result<(), String>;
}

pub struct PaymentProcessor {
    next_payment: AtomicU64,
    pub(crate) payments: RwLock<HashMap<u64, Payment>>,
    gateway: Box<dyn PaymentGateway>,
}

impl PaymentProcessor {
    pub fn new(gateway: Box<dyn PaymentGateway>) -> Self {
        Self {
            next_payment: AtomicU64::new(0),
            payments: RwLock::new(HashMap::new()),
            gateway,
        }
    }

    pub fn process(
        &self,
        reservation_number: &str,
        amount: u32,
        method: PaymentMethod,
    ) -> CarResult<Payment> {
        let payment_id = self.next_payment.fetch_add(1, Ordering::Relaxed) + 1;
        let (txn_id, status) = match self.gateway.charge(amount, method) {
            Ok(txn) => (Some(txn), PaymentStatus::Completed),
            Err(_) => (None, PaymentStatus::Failed),
        };
        let payment = Payment {
            payment_id,
            reservation_number: reservation_number.to_string(),
            amount,
            refunded: 0,
            method,
            txn_id,
            status,
        };
        // Failed charges are recorded too: they are the audit trail.
        wr(&self.payments).insert(payment_id, payment.clone());
        if payment.status == PaymentStatus::Failed {
            Err(CarError::PaymentFailed(format!(
                "charge for reservation {reservation_number} failed"
            )))
        } else {
            Ok(payment)
        }
    }

    /// LIFO refund: the most recent charge is refunded first. Returns the
    /// amount actually applied; supports partial refunds.
    pub fn refund(&self, reservation_number: &str, amount: u32) -> CarResult<u32> {
        let mut candidates: Vec<Payment> = rd(&self.payments)
            .values()
            .filter(|payment| payment.reservation_number == reservation_number)
            .cloned()
            .collect();
        candidates.sort_by_key(|payment| payment.payment_id);

        let mut remaining = amount;
        let mut applied = 0;
        for mut payment in candidates.into_iter().rev() {
            if remaining == 0 {
                break;
            }
            if !matches!(
                payment.status,
                PaymentStatus::Completed | PaymentStatus::PartiallyRefunded
            ) {
                continue;
            }
            let take = remaining.min(payment.amount - payment.refunded);
            if take == 0 {
                continue;
            }
            let txn_id = payment.txn_id.as_deref().ok_or(CarError::PaymentFailed(
                "payment has no gateway transaction".into(),
            ))?;
            self.gateway
                .refund(txn_id, take)
                .map_err(CarError::PaymentFailed)?;
            payment.refunded += take;
            payment.status = if payment.amount - payment.refunded == 0 {
                PaymentStatus::Refunded
            } else {
                PaymentStatus::PartiallyRefunded
            };
            wr(&self.payments).insert(payment.payment_id, payment);
            remaining -= take;
            applied += take;
        }
        Ok(applied)
    }
}
