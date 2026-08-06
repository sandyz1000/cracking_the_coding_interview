use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{AmsError, AmsResult};
use crate::locks::{rd, wr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaymentMethod {
    Card,
    Upi,
    Wallet,
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
    pub booking_number: u64,
    pub amount: f64,
    pub refunded: f64,
    pub method: PaymentMethod,
    pub txn_id: Option<String>,
    pub status: PaymentStatus,
}

/// Extension seam: swap the implementation for a real PSP without touching
/// the domain. Charge/refund errors are provider strings, mapped to
/// `AmsError::PaymentFailed` at the boundary.
pub trait PaymentGateway: Send + Sync {
    fn charge(&self, amount: f64, method: PaymentMethod) -> std::result::Result<String, String>;
    fn refund(&self, txn_id: &str, amount: f64) -> std::result::Result<(), String>;
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
        booking_number: u64,
        amount: f64,
        method: PaymentMethod,
    ) -> AmsResult<Payment> {
        let payment_id = self.next_payment.fetch_add(1, Ordering::Relaxed) + 1;
        let (txn_id, status) = match self.gateway.charge(amount, method) {
            Ok(txn) => (Some(txn), PaymentStatus::Completed),
            Err(_) => (None, PaymentStatus::Failed),
        };
        let payment = Payment {
            payment_id,
            booking_number,
            amount,
            refunded: 0.0,
            method,
            txn_id,
            status,
        };
        // Failed charges are recorded too: they are the audit trail.
        wr(&self.payments).insert(payment_id, payment.clone());
        if payment.status == PaymentStatus::Failed {
            Err(AmsError::PaymentFailed(format!(
                "charge for booking {booking_number} failed"
            )))
        } else {
            Ok(payment)
        }
    }

    /// LIFO refund: most recent charge is refunded first. Returns the amount
    /// actually applied; supports partial refunds.
    pub fn refund(&self, booking_number: u64, amount: f64) -> AmsResult<f64> {
        let mut candidates: Vec<Payment> = rd(&self.payments)
            .values()
            .filter(|payment| payment.booking_number == booking_number)
            .cloned()
            .collect();
        candidates.sort_by_key(|payment| payment.payment_id);

        let mut remaining = amount;
        let mut applied = 0.0;
        for mut payment in candidates.into_iter().rev() {
            if remaining <= f64::EPSILON {
                break;
            }
            if !matches!(
                payment.status,
                PaymentStatus::Completed | PaymentStatus::PartiallyRefunded
            ) {
                continue;
            }
            let take = remaining.min(payment.amount - payment.refunded);
            if take <= f64::EPSILON {
                continue;
            }
            let txn_id = payment.txn_id.as_deref().ok_or(AmsError::PaymentFailed(
                "payment has no gateway transaction".into(),
            ))?;
            self.gateway
                .refund(txn_id, take)
                .map_err(AmsError::PaymentFailed)?;
            payment.refunded += take;
            payment.status = if (payment.amount - payment.refunded).abs() < f64::EPSILON {
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
