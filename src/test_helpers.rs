//! Test helpers and fixtures for Tempurview tests
//!
//! This module provides utilities for creating test data and common test setups.

use crate::app::{App, LoadState};
use crate::domain::{FailureInfo, StatusCounts, WorkflowDetail, WorkflowStatus, WorkflowSummary};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;

/// Create a test workflow summary with the given status
pub fn make_workflow(status: WorkflowStatus) -> WorkflowSummary {
    make_workflow_with_type(status, "TestWorkflow")
}

/// Create a test workflow summary with the given status and type
pub fn make_workflow_with_type(status: WorkflowStatus, workflow_type: &str) -> WorkflowSummary {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    WorkflowSummary {
        workflow_id: format!("test-workflow-{}", id),
        run_id: format!("run-{}", id),
        workflow_type: Arc::from(workflow_type),
        status,
        start_time: Utc::now() - Duration::minutes(id as i64),
        close_time: if status == WorkflowStatus::Running {
            None
        } else {
            Some(Utc::now())
        },
        task_queue: Arc::from("test-queue"),
    }
}

/// Create multiple test workflows with various statuses
pub fn make_workflows(statuses: &[WorkflowStatus]) -> Vec<WorkflowSummary> {
    statuses.iter().map(|s| make_workflow(*s)).collect()
}

/// Create a test workflow detail
pub fn make_workflow_detail(status: WorkflowStatus) -> WorkflowDetail {
    let summary = make_workflow(status);
    let failure = if status == WorkflowStatus::Failed {
        Some(FailureInfo {
            message: "Test failure message".to_string(),
            failure_type: "TestError".to_string(),
            stack_trace: Some("at test.rs:42\n  at main.rs:10".to_string()),
            cause: None,
        })
    } else {
        None
    };

    WorkflowDetail {
        summary,
        input: Some(serde_json::json!({"test_input": "value"})),
        output: if status == WorkflowStatus::Completed {
            Some(serde_json::json!({"test_output": "result"}))
        } else {
            None
        },
        failure,
        history_length: 10,
        memo: HashMap::new(),
        search_attributes: HashMap::new(),
    }
}

/// Create test status counts
pub fn make_status_counts(running: u64, completed: u64, failed: u64) -> StatusCounts {
    let mut counts = StatusCounts::new();
    counts.set(WorkflowStatus::Running, running);
    counts.set(WorkflowStatus::Completed, completed);
    counts.set(WorkflowStatus::Failed, failed);
    counts
}

/// Create an App instance pre-loaded with test data
pub fn make_app_with_workflows(workflows: Vec<WorkflowSummary>) -> App {
    let mut app = App::new();
    app.workflows = LoadState::Loaded(workflows);
    app
}

/// Create an App instance with status counts
pub fn make_app_with_counts(counts: StatusCounts) -> App {
    let mut app = App::new();
    app.status_counts = LoadState::Loaded(counts);
    app
}

/// Builder pattern for creating test Apps
pub struct AppBuilder {
    app: App,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self { app: App::new() }
    }

    pub fn with_workflows(mut self, workflows: Vec<WorkflowSummary>) -> Self {
        self.app.workflows = LoadState::Loaded(workflows);
        self
    }

    pub fn with_counts(mut self, counts: StatusCounts) -> Self {
        self.app.status_counts = LoadState::Loaded(counts);
        self
    }

    pub fn with_status_filter(mut self, status: WorkflowStatus) -> Self {
        self.app.filter.status = Some(status);
        self
    }

    pub fn build(self) -> App {
        self.app
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_workflow() {
        let wf = make_workflow(WorkflowStatus::Running);
        assert_eq!(wf.status, WorkflowStatus::Running);
        assert!(wf.workflow_id.starts_with("test-workflow-"));
        assert!(wf.close_time.is_none());
    }

    #[test]
    fn test_make_workflow_completed_has_close_time() {
        let wf = make_workflow(WorkflowStatus::Completed);
        assert!(wf.close_time.is_some());
    }

    #[test]
    fn test_make_workflows() {
        let wfs = make_workflows(&[
            WorkflowStatus::Running,
            WorkflowStatus::Failed,
            WorkflowStatus::Completed,
        ]);
        assert_eq!(wfs.len(), 3);
        assert_eq!(wfs[0].status, WorkflowStatus::Running);
        assert_eq!(wfs[1].status, WorkflowStatus::Failed);
        assert_eq!(wfs[2].status, WorkflowStatus::Completed);
    }

    #[test]
    fn test_make_workflow_detail_failed_has_failure() {
        let detail = make_workflow_detail(WorkflowStatus::Failed);
        assert!(detail.failure.is_some());
    }

    #[test]
    fn test_app_builder() {
        let app = AppBuilder::new()
            .with_workflows(make_workflows(&[WorkflowStatus::Running]))
            .with_counts(make_status_counts(1, 0, 0))
            .with_status_filter(WorkflowStatus::Running)
            .build();

        assert!(matches!(app.workflows, LoadState::Loaded(_)));
        assert!(matches!(app.status_counts, LoadState::Loaded(_)));
        assert_eq!(app.filter.status, Some(WorkflowStatus::Running));
    }
}
