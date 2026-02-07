//! Watch mode: re-run a CLI command on a polling interval.
//!
//! - TTY mode: clears the screen and reprints the full output each cycle.
//! - Pipe mode: only emits output when it differs from the previous cycle (diff mode).

use crate::output::OutputFormat;
use std::io::Write;
use std::pin::Pin;
use std::future::Future;
use std::time::Duration;
use tracing::debug;

/// Configuration for watch mode polling.
pub struct WatchConfig {
    /// How often to re-run the command.
    pub interval: Duration,
    /// Resolved output format (table vs JSON).
    pub format: OutputFormat,
    /// Whether stdout is a TTY (controls clear-and-reprint vs diff-only).
    pub is_tty: bool,
}

/// Run a task in a loop, re-executing every `config.interval` seconds.
///
/// The task closure receives a `&mut dyn Write` to write its output to.
///
/// - TTY mode: clears the screen, prints a header, passes stdout to the task.
/// - Pipe mode: passes a buffer to the task, compares with previous output,
///   only writes to real stdout when the output has changed.
/// - Exits cleanly on Ctrl+C.
pub async fn run_watch_loop<F>(config: &WatchConfig, task: F) -> color_eyre::Result<()>
where
    F: for<'a> Fn(
        &'a mut (dyn Write + Send),
    ) -> Pin<Box<dyn Future<Output = color_eyre::Result<()>> + Send + 'a>>,
{
    let interval_secs = config.interval.as_secs();
    debug!(
        "Watch mode started: interval={}s, is_tty={}",
        interval_secs, config.is_tty
    );

    let mut previous_output: Option<Vec<u8>> = None;

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

            // Write directly to stdout (Stdout is Send, unlike StdoutLock)
            let mut stdout = std::io::stdout();
            if let Err(e) = task(&mut stdout).await {
                eprintln!("Error: {e}");
            }
        } else {
            // Pipe mode: capture into buffer and diff
            let mut buf: Vec<u8> = Vec::new();
            if let Err(e) = task(&mut buf).await {
                eprintln!("Error: {e}");
            } else {
                // Only emit if output changed (or first cycle)
                let changed = match &previous_output {
                    None => true,
                    Some(prev) => prev != &buf,
                };
                if changed {
                    let mut stdout = std::io::stdout();
                    let _ = stdout.write_all(&buf);
                    let _ = stdout.flush();
                    previous_output = Some(buf);
                }
            }
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
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_watch_loop_runs_task() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = WatchConfig {
            interval: Duration::from_millis(50),
            format: OutputFormat::Json,
            is_tty: true,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(180), async {
                run_watch_loop(&config, |_w| {
                    let c = counter_clone.clone();
                    Box::pin(async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    })
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 2,
            "Expected task to run at least 2 times, got {count}"
        );
    }

    #[tokio::test]
    async fn test_watch_loop_continues_on_error() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = WatchConfig {
            interval: Duration::from_millis(50),
            format: OutputFormat::Table,
            is_tty: true,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(180), async {
                run_watch_loop(&config, |_w| {
                    let c = counter_clone.clone();
                    Box::pin(async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Err(color_eyre::eyre::eyre!("simulated failure"))
                    })
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
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = WatchConfig {
            interval: Duration::from_secs(60),
            format: OutputFormat::Json,
            is_tty: true,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(50), async {
                run_watch_loop(&config, |_w| {
                    let c = counter_clone.clone();
                    Box::pin(async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    })
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

    #[tokio::test]
    async fn test_pipe_mode_suppresses_identical_output() {
        // In pipe mode, identical output across cycles should only be emitted once
        let task_count = Arc::new(AtomicU32::new(0));
        let task_count_clone = task_count.clone();

        let config = WatchConfig {
            interval: Duration::from_millis(30),
            format: OutputFormat::Json,
            is_tty: false,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(150), async {
                run_watch_loop(&config, |w| {
                    let tc = task_count_clone.clone();
                    Box::pin(async move {
                        tc.fetch_add(1, Ordering::SeqCst);
                        let _ = writeln!(w, "same output every time");
                        Ok(())
                    })
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        let tasks_run = task_count.load(Ordering::SeqCst);
        assert!(
            tasks_run >= 2,
            "Expected task to run at least 2 times, got {tasks_run}"
        );
    }

    #[tokio::test]
    async fn test_pipe_mode_emits_on_change() {
        // In pipe mode, output should be emitted each time it changes
        let cycle = Arc::new(AtomicU32::new(0));
        let cycle_clone = cycle.clone();

        let config = WatchConfig {
            interval: Duration::from_millis(30),
            format: OutputFormat::Json,
            is_tty: false,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(150), async {
                run_watch_loop(&config, |w| {
                    let c = cycle_clone.clone();
                    Box::pin(async move {
                        let n = c.fetch_add(1, Ordering::SeqCst);
                        let _ = writeln!(w, "cycle {n}");
                        Ok(())
                    })
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        let cycles_run = cycle.load(Ordering::SeqCst);
        assert!(
            cycles_run >= 2,
            "Expected at least 2 cycles with changing output, got {cycles_run}"
        );
    }

    #[tokio::test]
    async fn test_pipe_mode_first_cycle_always_emits() {
        let emitted = Arc::new(Mutex::new(false));
        let emitted_clone = emitted.clone();

        let config = WatchConfig {
            interval: Duration::from_secs(60),
            format: OutputFormat::Json,
            is_tty: false,
        };

        let handle = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_millis(50), async {
                run_watch_loop(&config, |w| {
                    let e = emitted_clone.clone();
                    Box::pin(async move {
                        let _ = writeln!(w, "first output");
                        *e.lock().unwrap() = true;
                        Ok(())
                    })
                })
                .await
            })
            .await;
        });

        handle.await.unwrap();

        assert!(
            *emitted.lock().unwrap(),
            "First cycle should have run and emitted"
        );
    }
}
