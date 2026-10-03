use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AmsError {
    #[error("flight {0} not found")]
    FlightNotFound(String),
    #[error("seat {0} not found")]
    SeatNotFound(String),
    #[error("seat unavailable: {0}")]
    SeatNotAvailable(String),
    #[error("booking {0} not found")]
    BookingNotFound(u64),
    #[error("passenger {0} not found")]
    PassengerNotFound(u64),
    #[error("permission denied")]
    PermissionDenied,
    #[error("invalid state transition: {0}")]
    InvalidTransition(String),
    #[error("payment failed: {0}")]
    PaymentFailed(String),
}

pub type AmsResult<T> = Result<T, AmsError>;
