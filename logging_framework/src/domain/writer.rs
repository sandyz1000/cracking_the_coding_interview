use super::message::LogMessage;
use crate::error::LogResult;

/// Extension point for output destinations. Implementing this trait is how new
/// backends (e.g. a database sink) plug into the framework without touching the
/// logger. Send is required so loggers can be shared across threads.
pub trait LogWriter: Send {
    fn write(&mut self, message: &LogMessage) -> LogResult<()>;
}
