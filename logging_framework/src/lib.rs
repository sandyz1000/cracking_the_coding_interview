//! Logging framework — log levels, configurable destinations, thread-safe and
//! extensible. Design decisions live in `DESIGN.md`.

pub mod adapters;
pub mod domain;
pub mod error;

pub use adapters::writers::{ConsoleWriter, FileWriter};
pub use domain::config::LogConfig;
pub use domain::level::LogLevel;
pub use domain::logger::Logger;
pub use domain::message::LogMessage;
pub use domain::writer::LogWriter;
pub use error::{LogError, LogResult};
