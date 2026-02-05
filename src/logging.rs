//! Logging configuration for Tempurview
//!
//! Logs are written to `~/.tempurview/logs/` with daily rotation.

use std::fs;
use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Get the tempurview config directory (~/.tempurview)
pub fn config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tempurview"))
}

/// Get the logs directory (~/.tempurview/logs)
pub fn logs_dir() -> Option<PathBuf> {
    config_dir().map(|c| c.join("logs"))
}

/// Get the path to the current log file
pub fn current_log_file() -> Option<PathBuf> {
    logs_dir().map(|l| l.join("tempurview.log"))
}

/// Initialize logging to file
///
/// Logs are written to `~/.tempurview/logs/tempurview.YYYY-MM-DD.log`
/// with daily rotation. Old logs are kept indefinitely.
///
/// Log level can be controlled via RUST_LOG environment variable.
/// Default level is `info` for tempurview, `warn` for dependencies.
pub fn init() -> Result<(), LoggingError> {
    let logs_dir = logs_dir().ok_or(LoggingError::NoHomeDir)?;

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir).map_err(|e| LoggingError::CreateDir(e.to_string()))?;

    // Set up file appender with daily rotation
    let file_appender = RollingFileAppender::new(Rotation::DAILY, &logs_dir, "tempurview.log");

    // Create the file layer
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false) // No colors in file
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    // Set up filter from RUST_LOG or use defaults
    // Default: info for everything (can be noisy with deps, but ensures all our logs are captured)
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    tracing::info!("Tempurview logging initialized");
    tracing::info!("Log directory: {}", logs_dir.display());

    Ok(())
}

/// Initialize logging for tests (no-op or minimal)
pub fn init_for_tests() {
    // Only init once, ignore errors if already initialized
    let _ = tracing_subscriber::fmt()
        .with_env_filter("tempurview=debug")
        .with_test_writer()
        .try_init();
}

#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("Could not determine home directory")]
    NoHomeDir,

    #[error("Failed to create log directory: {0}")]
    CreateDir(String),
}

/// Log an error with context
#[macro_export]
macro_rules! log_error {
    ($msg:expr) => {
        tracing::error!("{}", $msg)
    };
    ($msg:expr, $($arg:tt)*) => {
        tracing::error!($msg, $($arg)*)
    };
}

/// Log a warning with context
#[macro_export]
macro_rules! log_warn {
    ($msg:expr) => {
        tracing::warn!("{}", $msg)
    };
    ($msg:expr, $($arg:tt)*) => {
        tracing::warn!($msg, $($arg)*)
    };
}

/// Log info
#[macro_export]
macro_rules! log_info {
    ($msg:expr) => {
        tracing::info!("{}", $msg)
    };
    ($msg:expr, $($arg:tt)*) => {
        tracing::info!($msg, $($arg)*)
    };
}

/// Log debug info
#[macro_export]
macro_rules! log_debug {
    ($msg:expr) => {
        tracing::debug!("{}", $msg)
    };
    ($msg:expr, $($arg:tt)*) => {
        tracing::debug!($msg, $($arg)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir() {
        let dir = config_dir();
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.ends_with(".tempurview"));
    }

    #[test]
    fn test_logs_dir() {
        let dir = logs_dir();
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.ends_with("logs"));
    }
}
