// Logger abstraction for panorama-mail.
//
// Define the Logger trait here so any backend can be swapped in — the loguru-style
// default is in loguru.rs; the Panorama logger implements the same trait.
//
// Usage:
//   let logger: SharedLogger = Arc::new(LoguruLogger::new());
//   log_info!(logger, "Listening on port {}", port);

pub mod loguru;

use std::sync::Arc;

pub use loguru::LoguruLogger;

/// Severity levels, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

/// Core logging interface. Implement this trait to provide any logging backend.
///
/// Only `log` must be implemented; the level-specific helpers delegate to it.
pub trait Logger: Send + Sync {
    fn log(&self, level: Level, message: &str);

    fn debug(&self, message: &str) {
        self.log(Level::Debug, message);
    }
    fn info(&self, message: &str) {
        self.log(Level::Info, message);
    }
    fn warn(&self, message: &str) {
        self.log(Level::Warn, message);
    }
    fn error(&self, message: &str) {
        self.log(Level::Error, message);
    }
}

/// Convenience alias — pass `SharedLogger` to any component that needs to log.
pub type SharedLogger = Arc<dyn Logger>;

/// Log a DEBUG message with fmt-style formatting.
#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger.debug(&::std::format!($($arg)*))
    };
}

/// Log an INFO message with fmt-style formatting.
#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => {
        $logger.info(&::std::format!($($arg)*))
    };
}

/// Log a WARN message with fmt-style formatting.
#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => {
        $logger.warn(&::std::format!($($arg)*))
    };
}

/// Log an ERROR message with fmt-style formatting.
#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => {
        $logger.error(&::std::format!($($arg)*))
    };
}
