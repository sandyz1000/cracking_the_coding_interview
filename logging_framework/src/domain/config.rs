use super::level::LogLevel;

/// Per-logger settings. The destination itself is owned by the `Logger`, not
/// the config, so a single config can be reused across loggers.
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub level: LogLevel,
    pub prefix: String,
}

impl LogConfig {
    pub fn new(level: LogLevel, prefix: impl Into<String>) -> Self {
        Self {
            level,
            prefix: prefix.into(),
        }
    }
}
