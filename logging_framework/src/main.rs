//! Demo: concurrent logging through a configured console logger plus a file.

use std::sync::Arc;

use logging_framework::adapters::writers::{ConsoleWriter, FileWriter};
use logging_framework::domain::config::LogConfig;
use logging_framework::domain::level::LogLevel;
use logging_framework::domain::logger::Logger;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log_path = std::env::temp_dir().join("lf_demo.log");
    let _ = std::fs::remove_file(&log_path);

    let console = Logger::new(LogConfig::new(LogLevel::Info, "console"), ConsoleWriter);
    let file = Logger::new(
        LogConfig::new(LogLevel::Debug, "file"),
        FileWriter::open(&log_path)?,
    );

    console.debug("hidden on console (threshold is Info)")?;
    console.info("console sees this")?;

    let file = Arc::new(file);
    let mut handles = Vec::new();
    for i in 0..8 {
        let file = Arc::clone(&file);
        handles.push(std::thread::spawn(move || {
            file.debug(format!("worker {i} debug"))?;
            file.error(format!("worker {i} error"))
        }));
    }
    for h in handles {
        h.join().unwrap()?;
    }

    let written = std::fs::read_to_string(&log_path)?;
    println!("file log wrote {} lines", written.lines().count());
    Ok(())
}
