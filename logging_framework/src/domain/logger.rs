use std::sync::{Mutex, MutexGuard};

use super::config::LogConfig;
use super::level::LogLevel;
use super::message::LogMessage;
use super::writer::LogWriter;
use crate::error::{LogError, LogResult};

pub fn mutex_guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Shared, thread-safe logger. The underlying writer is wrapped in a `Mutex` so
/// concurrent `log` calls serialize and never interleave mid-message.
pub struct Logger {
    config: LogConfig,
    writer: Mutex<Box<dyn LogWriter>>,
}

impl Logger {
    pub fn new(config: LogConfig, writer: impl LogWriter + 'static) -> Self {
        // let w: &dyn LogWriter = &crate::ConsoleWriter;
        Self {
            config,
            writer: Mutex::new(Box::new(writer)),
        }
    }

    /// Record `message` if its severity meets the configured threshold.
    pub fn log(&self, level: LogLevel, message: impl Into<String>) -> LogResult<()> {
        if level < self.config.level {
            return Ok(());
        }
        let record = LogMessage::new(level, self.config.prefix.clone(), message.into());
        let mut writer = mutex_guard(&self.writer);
        writer.write(&record)
    }

    pub fn debug(&self, message: impl Into<String>) -> LogResult<()> {
        self.log(LogLevel::Debug, message)
    }

    pub fn info(&self, message: impl Into<String>) -> LogResult<()> {
        self.log(LogLevel::Info, message)
    }

    pub fn warn(&self, message: impl Into<String>) -> LogResult<()> {
        self.log(LogLevel::Warn, message)
    }

    pub fn error(&self, message: impl Into<String>) -> LogResult<()> {
        self.log(LogLevel::Error, message)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::adapters::writers::CapturingWriter;

    fn logger_with(level: LogLevel, captures: Arc<Mutex<Vec<LogMessage>>>) -> Logger {
        let writer = CapturingWriter::new(captures);
        Logger::new(LogConfig::new(level, "t"), writer)
    }

    #[test]
    fn test_level_ordering() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_threshold_filters() {
        let msgs = Arc::new(Mutex::new(Vec::new()));
        let logger = logger_with(LogLevel::Warn, Arc::clone(&msgs));
        logger.debug("d").unwrap();
        logger.info("i").unwrap();
        logger.warn("w").unwrap();
        logger.error("e").unwrap();
        let got: Vec<String> = msgs
            .lock()
            .unwrap()
            .iter()
            .map(|m| m.message.clone())
            .collect();
        assert_eq!(got, vec!["w".to_string(), "e".to_string()]);
    }

    #[test]
    fn test_debug_allows_all() {
        let msgs = Arc::new(Mutex::new(Vec::new()));
        let logger = logger_with(LogLevel::Debug, Arc::clone(&msgs));
        for lvl in [
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ] {
            logger.log(lvl, "x").unwrap();
        }
        assert_eq!(msgs.lock().unwrap().len(), 4);
    }

    #[test]
    fn test_level_tagging() {
        let msgs = Arc::new(Mutex::new(Vec::new()));
        let logger = logger_with(LogLevel::Debug, Arc::clone(&msgs));
        logger.warn("w").unwrap();
        assert_eq!(msgs.lock().unwrap()[0].level, LogLevel::Warn);
    }

    #[test]
    fn test_prefix_set() {
        let msgs = Arc::new(Mutex::new(Vec::new()));
        let logger = logger_with(LogLevel::Debug, Arc::clone(&msgs));
        logger.info("hi").unwrap();
        assert_eq!(msgs.lock().unwrap()[0].prefix, "t");
    }

    #[test]
    fn test_concurrent_logging() {
        let msgs = Arc::new(Mutex::new(Vec::new()));
        let logger = logger_with(LogLevel::Debug, Arc::clone(&msgs));
        let logger = Arc::new(logger);
        let mut handles = Vec::new();
        for i in 0..8 {
            let logger = Arc::clone(&logger);
            handles.push(std::thread::spawn(move || logger.info(format!("t{i}"))));
        }
        for h in handles {
            assert!(h.join().is_ok());
        }
        assert_eq!(msgs.lock().unwrap().len(), 8);
    }
}
