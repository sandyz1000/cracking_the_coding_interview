/// Severity of a log message. Derived ordering (Debug < Info < Warn < Error)
/// is what drives level filtering: a message must be `>=` the configured
/// threshold to be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}
