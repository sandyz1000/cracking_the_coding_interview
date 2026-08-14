use std::time::SystemTime;

use super::level::LogLevel;

/// A single record ready to be rendered by a writer.
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub level: LogLevel,
    pub prefix: String,
    pub message: String,
    pub timestamp: SystemTime,
}

impl LogMessage {
    pub fn new(level: LogLevel, prefix: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level,
            prefix: prefix.into(),
            message: message.into(),
            timestamp: SystemTime::now(),
        }
    }
}

impl std::fmt::Display for LogMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {:?} - {}", self.prefix, self.level, self.message)
    }
}
