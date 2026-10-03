use thiserror::Error;

/// Errors produced by the logging pipeline.
#[derive(Debug, Error)]
pub enum LogError {
    /// A writer could not be created (e.g. file not openable).
    #[error("failed to open log destination")]
    WriterInit,
    /// A writer could not persist a message (e.g. disk full).
    #[error("failed to write log message")]
    Write,
    /// The poison-learnt lock on the shared writer is unusable.
    #[error("log writer lock is poisoned")]
    Poisoned,
}

/// Convenient `Result` alias scoped to this crate.
pub type LogResult<T> = Result<T, LogError>;
