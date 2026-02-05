//! Logging configuration for Tempurview
//!
//! Logs are written to `~/.tempurview/logs/` with unique timestamp per run.

use chrono::Local;
use std::fs::{self, File};
use std::path::PathBuf;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
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
/// Logs are written to `~/.tempurview/logs/tempurview.YYYY-MM-DDTHH-MM-SS.log`
/// with a unique file per run.
///
/// Log level can be controlled via RUST_LOG environment variable.
/// Default level is `info` for tempurview, `warn` for dependencies.
///
/// Returns a guard that must be kept alive for the duration of the program.
pub fn init() -> Result<WorkerGuard, LoggingError> {
    let logs_dir = logs_dir().ok_or(LoggingError::NoHomeDir)?;

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir).map_err(|e| LoggingError::CreateDir(e.to_string()))?;

    // Create log file with ISO timestamp (using - instead of : for filesystem compatibility)
    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
    let log_filename = format!("tempurview.{}.log", timestamp);
    let log_path = logs_dir.join(&log_filename);

    let file = File::create(&log_path).map_err(|e| LoggingError::CreateDir(e.to_string()))?;

    // Use non_blocking writer for performance
    let (non_blocking, guard) = NonBlocking::new(file);

    // Create the file layer
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // No colors in file
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    // Set up filter from RUST_LOG or use defaults
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Initialize the subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    tracing::info!("Tempurview logging initialized");
    tracing::info!("Log file: {}", log_path.display());

    Ok(guard)
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
