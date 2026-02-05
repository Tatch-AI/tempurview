//! Common test utilities shared across integration tests

use chrono::{Duration, Utc};
use tempurview::domain::{WorkflowStatus, WorkflowSummary};

/// Create a test workflow with a specific status
pub fn make_test_workflow(id: u64, status: WorkflowStatus) -> WorkflowSummary {
    WorkflowSummary {
        workflow_id: format!("integration-test-{}", id),
        run_id: format!("run-{}", id),
        workflow_type: "IntegrationTestWorkflow".to_string(),
        status,
        start_time: Utc::now() - Duration::hours(id as i64),
        close_time: if status == WorkflowStatus::Running {
            None
        } else {
            Some(Utc::now())
        },
        task_queue: "integration-test-queue".to_string(),
    }
}
