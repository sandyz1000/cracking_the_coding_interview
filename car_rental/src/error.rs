use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CarError {
    #[error("vehicle {0} not found")]
    VehicleNotFound(String),
    #[error("location {0} not found")]
    LocationNotFound(String),
    #[error("customer {0} not found")]
    CustomerNotFound(u64),
    #[error("reservation {0} not found")]
    ReservationNotFound(String),
    #[error("vehicle unavailable: {0}")]
    VehicleNotAvailable(String),
    #[error("invalid dates: {0}")]
    InvalidDates(String),
    #[error("invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("permission denied")]
    PermissionDenied,
    #[error("payment failed: {0}")]
    PaymentFailed(String),
    #[error("refund failed: {0}")]
    RefundFailed(String),
}

pub type CarResult<T> = Result<T, CarError>;
