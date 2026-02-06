use super::{ClientError, ClientResult, TemporalClient};
use crate::domain::{
    FailureInfo, HistoryEvent, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use rand::Rng;
use std::collections::HashMap;
use std::time::Duration as StdDuration;

/// Mock client for testing UI without Temporal
pub struct MockTemporalClient {
    pub workflows: Vec<WorkflowSummary>,
    pub details: HashMap<String, WorkflowDetail>,
    pub should_fail: bool,
    pub latency: StdDuration,
}

impl MockTemporalClient {
    pub fn new() -> Self {
        Self {
            workflows: Vec::new(),
            details: HashMap::new(),
            should_fail: false,
            latency: StdDuration::from_millis(50),
        }
    }

    pub fn with_workflows(workflows: Vec<WorkflowSummary>) -> Self {
        let mut client = Self::new();
        client.workflows = workflows;
        client
    }

    /// Generate realistic test data
    pub fn with_random_data(count: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut workflows = Vec::with_capacity(count);

        let workflow_types = [
            "EmailGenerationWorkflow",
            "DataProcessingWorkflow",
            "UserOnboardingWorkflow",
            "ReportGenerationWorkflow",
            "NotificationWorkflow",
            "PaymentProcessingWorkflow",
            "InventoryUpdateWorkflow",
        ];

        let task_queues = ["default", "high-priority", "batch-processing", "background"];

        for i in 0..count {
            let status = match rng.gen_range(0..100) {
                0..=40 => WorkflowStatus::Completed,
                41..=60 => WorkflowStatus::Running,
                61..=75 => WorkflowStatus::Failed,
                76..=85 => WorkflowStatus::Canceled,
                86..=92 => WorkflowStatus::Terminated,
                93..=97 => WorkflowStatus::TimedOut,
                _ => WorkflowStatus::ContinuedAsNew,
            };

            let workflow_type = workflow_types[rng.gen_range(0..workflow_types.len())].to_string();
            let task_queue = task_queues[rng.gen_range(0..task_queues.len())].to_string();

            let start_offset = Duration::minutes(rng.gen_range(0..10080)); // Up to 7 days ago
            let start_time = Utc::now() - start_offset;

            let close_time = if status != WorkflowStatus::Running {
                Some(start_time + Duration::minutes(rng.gen_range(1..60)))
            } else {
                None
            };

            workflows.push(WorkflowSummary {
                workflow_id: format!(
                    "{}-{}",
                    workflow_type.to_lowercase().replace("workflow", ""),
                    i
                ),
                run_id: format!("run-{}-{}", i, rng.gen::<u32>()),
                workflow_type,
                status,
                start_time,
                close_time,
                task_queue,
            });
        }

        // Sort by start time descending (most recent first)
        workflows.sort_by(|a, b| b.start_time.cmp(&a.start_time));

        Self::with_workflows(workflows)
    }

    pub fn with_latency(mut self, latency: StdDuration) -> Self {
        self.latency = latency;
        self
    }

    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    async fn simulate_latency(&self) {
        tokio::time::sleep(self.latency).await;
    }

    fn check_failure(&self) -> ClientResult<()> {
        if self.should_fail {
            Err(ClientError::ConnectionError("Mock failure".into()))
        } else {
            Ok(())
        }
    }
}

impl Default for MockTemporalClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TemporalClient for MockTemporalClient {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64> {
        self.simulate_latency().await;
        self.check_failure()?;

        let count = if let Some(q) = query {
            // Simple query parsing for mock
            let q_upper = q.to_uppercase();
            if q_upper.contains("EXECUTIONSTATUS=") {
                // Extract status from query
                for status in WorkflowStatus::all() {
                    if q_upper.contains(&status.as_query_value().to_uppercase()) {
                        return Ok(self
                            .workflows
                            .iter()
                            .filter(|w| w.status == *status)
                            .count() as u64);
                    }
                }
                0
            } else {
                self.workflows.len() as u64
            }
        } else {
            self.workflows.len() as u64
        };

        Ok(count)
    }

    async fn list(
        &self,
        filter: &WorkflowFilter,
        limit: u32,
    ) -> ClientResult<Vec<WorkflowSummary>> {
        self.simulate_latency().await;
        self.check_failure()?;

        let filtered: Vec<WorkflowSummary> = self
            .workflows
            .iter()
            .filter(|wf| {
                if let Some(status) = &filter.status {
                    if wf.status != *status {
                        return false;
                    }
                }
                if let Some(wf_type) = &filter.workflow_type {
                    if wf.workflow_type != *wf_type {
                        return false;
                    }
                }
                if let Some(prefix) = &filter.workflow_id_prefix {
                    if !wf.workflow_id.starts_with(prefix) {
                        return false;
                    }
                }
                if let Some(after) = &filter.start_time_after {
                    if wf.start_time <= *after {
                        return false;
                    }
                }
                if let Some(before) = &filter.start_time_before {
                    if wf.start_time >= *before {
                        return false;
                    }
                }
                true
            })
            .take(limit as usize)
            .cloned()
            .collect();

        Ok(filtered)
    }

    async fn describe(
        &self,
        workflow_id: &str,
        _run_id: Option<&str>,
    ) -> ClientResult<WorkflowDetail> {
        self.simulate_latency().await;
        self.check_failure()?;

        // Check if we have pre-populated details
        if let Some(detail) = self.details.get(workflow_id) {
            return Ok(detail.clone());
        }

        // Find the workflow summary and generate detail
        let summary = self
            .workflows
            .iter()
            .find(|w| w.workflow_id == workflow_id)
            .ok_or_else(|| ClientError::NotFound(workflow_id.to_string()))?
            .clone();

        let failure = if summary.status == WorkflowStatus::Failed {
            Some(FailureInfo {
                message: "Mock failure message for testing".to_string(),
                failure_type: "ApplicationError".to_string(),
                stack_trace: Some("at mock.stack.trace\n  at line 42".to_string()),
                cause: None,
            })
        } else {
            None
        };

        Ok(WorkflowDetail {
            summary,
            input: Some(serde_json::json!({"mockInput": "value"})),
            output: Some(serde_json::json!({"mockOutput": "result"})),
            failure,
            history_length: 42,
            memo: HashMap::new(),
            search_attributes: HashMap::new(),
        })
    }

    async fn get_history(
        &self,
        workflow_id: &str,
        _run_id: Option<&str>,
    ) -> ClientResult<Vec<HistoryEvent>> {
        self.simulate_latency().await;
        self.check_failure()?;

        let workflow = self
            .workflows
            .iter()
            .find(|w| w.workflow_id == workflow_id)
            .ok_or_else(|| ClientError::NotFound(workflow_id.to_string()))?;

        let mut rng = rand::thread_rng();
        let mut events = vec![
            HistoryEvent {
                event_id: 1,
                event_type: "WorkflowExecutionStarted".to_string(),
                timestamp: workflow.start_time,
                details: serde_json::json!({"workflowType": workflow.workflow_type}),
            },
            HistoryEvent {
                event_id: 2,
                event_type: "WorkflowTaskScheduled".to_string(),
                timestamp: workflow.start_time,
                details: serde_json::json!({}),
            },
            HistoryEvent {
                event_id: 3,
                event_type: "WorkflowTaskStarted".to_string(),
                timestamp: workflow.start_time,
                details: serde_json::json!({}),
            },
            HistoryEvent {
                event_id: 4,
                event_type: "WorkflowTaskCompleted".to_string(),
                timestamp: workflow.start_time,
                details: serde_json::json!({}),
            },
        ];

        let activity_types = [
            "SendEmail",
            "ProcessPayment",
            "UpdateInventory",
            "GenerateReport",
            "ValidateInput",
        ];

        let num_activities = rng.gen_range(3..=5);
        let mut event_id = 5_i64;

        for i in 0..num_activities {
            let activity_type = activity_types[i % activity_types.len()];
            let scheduled_event_id = event_id;
            let sched_time = workflow.start_time + Duration::seconds(rng.gen_range(1..30) + (i as i64) * 10);

            // ActivityTaskScheduled
            events.push(HistoryEvent {
                event_id,
                event_type: "ActivityTaskScheduled".to_string(),
                timestamp: sched_time,
                details: serde_json::json!({
                    "activity_id": format!("{}", i + 1),
                    "activity_type": activity_type,
                    "task_queue": "default",
                    "input": {"data": format!("input-for-{}", activity_type)},
                }),
            });
            event_id += 1;

            // Decide activity outcome
            let outcome: u8 = match workflow.status {
                WorkflowStatus::Running if i == num_activities - 1 => {
                    // Last activity in a running workflow: 50% running, 50% pending
                    if rng.gen_bool(0.5) { 1 } else { 0 } // 0=pending, 1=running
                }
                _ => rng.gen_range(2..=5), // 2=completed, 3=failed, 4=timedout, 5=canceled
            };

            if outcome == 0 {
                // Pending: only scheduled, no started event
                continue;
            }

            let start_time = sched_time + Duration::milliseconds(rng.gen_range(5..200));
            let attempt: i32 = if outcome == 3 { rng.gen_range(1..=3) } else { 1 };

            // ActivityTaskStarted
            events.push(HistoryEvent {
                event_id,
                event_type: "ActivityTaskStarted".to_string(),
                timestamp: start_time,
                details: serde_json::json!({
                    "scheduled_event_id": scheduled_event_id,
                    "attempt": attempt,
                    "identity": "mock-worker@host",
                }),
            });
            let started_event_id = event_id;
            event_id += 1;

            if outcome == 1 {
                // Running: started but no closed event
                continue;
            }

            let close_time = start_time + Duration::milliseconds(rng.gen_range(100..5000));

            match outcome {
                2 => {
                    // Completed
                    events.push(HistoryEvent {
                        event_id,
                        event_type: "ActivityTaskCompleted".to_string(),
                        timestamp: close_time,
                        details: serde_json::json!({
                            "scheduled_event_id": scheduled_event_id,
                            "started_event_id": started_event_id,
                            "result": {"success": true, "data": format!("result-{}", activity_type)},
                            "identity": "mock-worker@host",
                        }),
                    });
                }
                3 => {
                    // Failed
                    events.push(HistoryEvent {
                        event_id,
                        event_type: "ActivityTaskFailed".to_string(),
                        timestamp: close_time,
                        details: serde_json::json!({
                            "scheduled_event_id": scheduled_event_id,
                            "started_event_id": started_event_id,
                            "failure": {
                                "message": format!("{} failed: mock error", activity_type),
                                "failure_type": "ApplicationFailure",
                                "stack_trace": "at mock.Worker.execute()\n  at line 42",
                            },
                            "identity": "mock-worker@host",
                        }),
                    });
                }
                4 => {
                    // TimedOut
                    events.push(HistoryEvent {
                        event_id,
                        event_type: "ActivityTaskTimedOut".to_string(),
                        timestamp: close_time,
                        details: serde_json::json!({
                            "scheduled_event_id": scheduled_event_id,
                            "started_event_id": started_event_id,
                        }),
                    });
                }
                _ => {
                    // Canceled
                    events.push(HistoryEvent {
                        event_id,
                        event_type: "ActivityTaskCanceled".to_string(),
                        timestamp: close_time,
                        details: serde_json::json!({
                            "scheduled_event_id": scheduled_event_id,
                            "started_event_id": started_event_id,
                            "identity": "mock-worker@host",
                        }),
                    });
                }
            }
            event_id += 1;
        }

        Ok(events)
    }

    async fn cancel(&self, workflow_id: &str, _run_id: Option<&str>) -> ClientResult<()> {
        self.simulate_latency().await;
        self.check_failure()?;

        if !self.workflows.iter().any(|w| w.workflow_id == workflow_id) {
            return Err(ClientError::NotFound(workflow_id.to_string()));
        }

        Ok(())
    }

    async fn terminate(
        &self,
        workflow_id: &str,
        _run_id: Option<&str>,
        _reason: &str,
    ) -> ClientResult<()> {
        self.simulate_latency().await;
        self.check_failure()?;

        if !self.workflows.iter().any(|w| w.workflow_id == workflow_id) {
            return Err(ClientError::NotFound(workflow_id.to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_count() {
        let client =
            MockTemporalClient::with_random_data(100).with_latency(StdDuration::from_millis(0));

        let count = client.count(None).await.unwrap();
        assert_eq!(count, 100);
    }

    #[tokio::test]
    async fn test_mock_count_with_filter() {
        let workflows = vec![
            WorkflowSummary {
                workflow_id: "wf-1".to_string(),
                run_id: "run-1".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                close_time: None,
                task_queue: "default".to_string(),
            },
            WorkflowSummary {
                workflow_id: "wf-2".to_string(),
                run_id: "run-2".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Failed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: "default".to_string(),
            },
        ];

        let client =
            MockTemporalClient::with_workflows(workflows).with_latency(StdDuration::from_millis(0));

        let running = client
            .count(Some("ExecutionStatus='Running'"))
            .await
            .unwrap();
        assert_eq!(running, 1);

        let failed = client
            .count(Some("ExecutionStatus='Failed'"))
            .await
            .unwrap();
        assert_eq!(failed, 1);
    }

    #[tokio::test]
    async fn test_mock_list_with_filter() {
        let workflows = vec![
            WorkflowSummary {
                workflow_id: "wf-1".to_string(),
                run_id: "run-1".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                close_time: None,
                task_queue: "default".to_string(),
            },
            WorkflowSummary {
                workflow_id: "wf-2".to_string(),
                run_id: "run-2".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Failed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: "default".to_string(),
            },
        ];

        let client =
            MockTemporalClient::with_workflows(workflows).with_latency(StdDuration::from_millis(0));

        let filter = WorkflowFilter::new().with_status(WorkflowStatus::Running);
        let result = client.list(&filter, 100).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].workflow_id, "wf-1");
    }

    #[tokio::test]
    async fn test_mock_failure() {
        let client = MockTemporalClient::new()
            .with_failure()
            .with_latency(StdDuration::from_millis(0));

        let result = client.count(None).await;
        assert!(result.is_err());
    }
}
