use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::domain::message::LogMessage;
use crate::domain::writer::LogWriter;
use crate::error::{LogError, LogResult};

/// Writes records to stdout.
#[derive(Debug, Default)]
pub struct ConsoleWriter;

impl LogWriter for ConsoleWriter {
    fn write(&mut self, message: &LogMessage) -> LogResult<()> {
        println!("{message}");
        Ok(())
    }
}

/// Appends records to a file, creating it on first use. The file handle is
/// the sync point shared across threads via the logger's mutex.
pub struct FileWriter {
    handle: File,
}

impl FileWriter {
    pub fn open(path: impl AsRef<Path>) -> LogResult<Self> {
        let handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.as_ref())
            .map_err(|_| LogError::WriterInit)?;
        Ok(Self { handle })
    }
}

impl LogWriter for FileWriter {
    fn write(&mut self, message: &LogMessage) -> LogResult<()> {
        writeln!(self.handle, "{message}").map_err(|_| LogError::Write)
    }
}

/// Test double that records every message instead of emitting it.
#[derive(Debug, Clone)]
pub struct CapturingWriter {
    pub captured: Arc<Mutex<Vec<LogMessage>>>,
}

impl CapturingWriter {
    pub fn new(captured: Arc<Mutex<Vec<LogMessage>>>) -> Self {
        Self { captured }
    }
}

impl LogWriter for CapturingWriter {
    fn write(&mut self, message: &LogMessage) -> LogResult<()> {
        self.captured
            .lock()
            .map_err(|_| LogError::Poisoned)?
            .push(message.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::level::LogLevel;

    #[test]
    fn test_file_appends() {
        let path = std::env::temp_dir().join("lf_test_appends.log");
        let _ = std::fs::remove_file(&path);
        let mut writer = FileWriter::open(&path).unwrap();
        writer
            .write(&LogMessage::new(LogLevel::Info, "p", "l1"))
            .unwrap();
        writer
            .write(&LogMessage::new(LogLevel::Error, "p", "l2"))
            .unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("l1"));
        assert!(contents.contains("l2"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_file_created() {
        let path = std::env::temp_dir().join("lf_test_created.log");
        let _ = std::fs::remove_file(&path);
        assert!(!path.exists());
        let mut writer = FileWriter::open(&path).unwrap();
        writer
            .write(&LogMessage::new(LogLevel::Debug, "p", "hi"))
            .unwrap();
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
    }
}
