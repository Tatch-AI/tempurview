//! Tempurview - A terminal interface for Temporal workflows
//!
//! This library provides the core functionality for the Tempurview TUI application.
//! It can also be used as a library for building custom Temporal workflow tools.

pub mod action;
pub mod app;
pub mod cli_worker;
pub mod client;
pub mod config;
pub mod domain;
pub mod event;
pub mod logging;
pub mod proto;
pub mod tui;
pub mod widgets;

// Re-export commonly used types at the crate root for convenience
pub use action::{Action, DataPayload};
pub use app::{App, Effect, InputMode, LoadState, View};
pub use cli_worker::{CliHandle, CliRequest, CliWorker};
pub use client::{
    CliTemporalClient, ClientError, ClientResult, GrpcTemporalClient, MockTemporalClient,
    TemporalClient,
};
pub use config::{Config, ConfigError};
pub use domain::{
    ActivityExecution, ActivityStatus, FailureInfo, HistoryEvent, StatusCounts, WorkflowDetail,
    WorkflowFilter, WorkflowStatus, WorkflowSummary,
};

/// Test utilities for building tests
#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        // Verify key types are accessible
        let _ = WorkflowStatus::Running;
        let _ = WorkflowFilter::new();
        let _ = App::new();
    }
}
