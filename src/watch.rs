//! Watch mode: re-run a CLI command on a polling interval.
//!
//! For TTY output, clears the screen and reprints each cycle.
//! For piped output, appends JSON each cycle (no clearing).

use crate::output::OutputFormat;
use std::future::Future;
use std::time::Duration;
use tracing::debug;

/// Configuration for watch mode polling.
pub struct WatchConfig {
    /// How often to re-run the command.
    pub interval: Duration,
    /// Resolved output format (table vs JSON).
    pub format: OutputFormat,
    /// Whether stdout is a TTY (controls clear-and-reprint vs append).
    pub is_tty: bool,
}

/// Run a task in a loop, re-executing every `config.interval` seconds.
///
/// - TTY mode: clears the screen and prints a header with timestamp before each run.
/// - Pipe mode: runs the task and appends output each cycle.
/// - Exits cleanly on Ctrl+C.
pub async fn run_watch_loop<F, Fut>(config: &WatchConfig, task: F) -> color_eyre::Result<()>
where
    F: Fn() -> Fut,
    Fut: Future<Output = color_eyre::Result<()>>,
{
    let interval_secs = config.interval.as_secs();
    debug!(
        "Watch mode started: interval={}s, is_tty={}",
        interval_secs, config.is_tty
    );

    loop {
        if config.is_tty {
            // Clear screen and move cursor to top-left
            print!("\x1B[2J\x1B[H");

            // Print header with timestamp
            let now = chrono::Local::now();
            eprintln!(
                "[{}] Refreshing every {}s... (Ctrl+C to stop)\n",
                now.format("%Y-%m-%d %H:%M:%S"),
                interval_secs,
            );
        }

        // Run the command; log errors but keep looping
        if let Err(e) = task().await {
            eprintln!("Error: {e}");
        }

        // Sleep, but exit immediately on Ctrl+C
        tokio::select! {
            _ = tokio::time::sleep(config.interval) => {
                // Continue to next iteration
            }
            _ = tokio::signal::ctrl_c() => {
                if config.is_tty {
                    eprintln!("\nWatch mode stopped.");
                }
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_watch_loop_runs_task() {
        // Verify the task closure is actually invoked
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = WatchConfig {
            interval: Duration::from_millis(50),
            format: OutputFormat::Json,
            is_tty: false,
        };

        // Run the loop in a background task and cancel it after a short delay
        let handle = tokio::spawn(async move {
            // We can't easily send ctrl_c in tests, so use a timeout wrapper
            let _ = tokio::time::timeout(Duration::from_millis(180), async {
                run_watch_loop(&config, || {
                    let c = counter_clone.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        // Should have run at least twice in ~180ms with 50ms interval
        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 2,
            "Expected task to run at least 2 times, got {count}"
        );
    }

    #[tokio::test]
    async fn test_watch_loop_continues_on_error() {
        // Verify the loop keeps going even when the task returns an error
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = WatchConfig {
            interval: Duration::from_millis(50),
            format: OutputFormat::Table,
            is_tty: false,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(180), async {
                run_watch_loop(&config, || {
                    let c = counter_clone.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Err(color_eyre::eyre::eyre!("simulated failure"))
                    }
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 2,
            "Expected task to run at least 2 times despite errors, got {count}"
        );
    }

    #[tokio::test]
    async fn test_watch_loop_single_iteration_with_zero_timeout() {
        // With a very short timeout, should run at least once
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = WatchConfig {
            interval: Duration::from_secs(60), // Long interval
            format: OutputFormat::Json,
            is_tty: false,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(50), async {
                run_watch_loop(&config, || {
                    let c = counter_clone.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 1,
            "Expected task to run at least once, got {count}"
        );
    }
}
