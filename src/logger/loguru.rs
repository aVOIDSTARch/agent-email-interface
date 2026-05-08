// Loguru-style logger for panorama-mail.
//
// Outputs to stderr in the same format as Python's loguru:
//
//   2026-05-08 14:30:00.123 | INFO  | panorama-mail | message
//
// ANSI colors are enabled automatically when stderr is a terminal and
// disabled when piped (e.g. MCP stdio mode or log redirection).
//
// Swap this out by implementing the Logger trait for any other backend.

use std::io::IsTerminal;

use chrono::Local;

use super::{Level, Logger};

pub struct LoguruLogger {
    min_level: Level,
    color: bool,
}

impl LoguruLogger {
    /// Create a logger at DEBUG level. Colors are auto-detected from stderr.
    pub fn new() -> Self {
        Self {
            min_level: Level::Debug,
            color: std::io::stderr().is_terminal(),
        }
    }

    /// Create a logger with an explicit minimum level.
    pub fn with_level(level: Level) -> Self {
        Self {
            min_level: level,
            color: std::io::stderr().is_terminal(),
        }
    }
}

impl Default for LoguruLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger for LoguruLogger {
    fn log(&self, level: Level, message: &str) {
        if level < self.min_level {
            return;
        }

        let now = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

        if self.color {
            let (color, label) = match level {
                Level::Debug => ("\x1b[36m", "DEBUG"),
                Level::Info  => ("\x1b[32m", "INFO "),
                Level::Warn  => ("\x1b[33m", "WARN "),
                Level::Error => ("\x1b[31m", "ERROR"),
            };
            eprintln!("{now} | {color}{label}\x1b[0m | panorama-mail | {message}");
        } else {
            let label = match level {
                Level::Debug => "DEBUG",
                Level::Info  => "INFO ",
                Level::Warn  => "WARN ",
                Level::Error => "ERROR",
            };
            eprintln!("{now} | {label} | panorama-mail | {message}");
        }
    }
}
