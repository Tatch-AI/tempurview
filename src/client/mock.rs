use super::{ClientError, ClientResult, TemporalClient};
use crate::domain::{
    FailureInfo, HistoryEvent, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
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

    /// Generate realistic test data with some deterministic patterns that
    /// reliably trigger insight findings (failure hotspots, stuck workflows, etc.)
    pub fn with_random_data(count: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut workflows = Vec::with_capacity(count);

        let workflow_types: Vec<Arc<str>> = [
            "EmailGenerationWorkflow",
            "DataProcessingWorkflow",
            "UserOnboardingWorkflow",
            "ReportGenerationWorkflow",
            "NotificationWorkflow",
            "PaymentProcessingWorkflow",
            "InventoryUpdateWorkflow",
        ]
        .iter()
        .map(|s| Arc::from(*s))
        .collect();

        let task_queues: Vec<Arc<str>> =
            ["default", "high-priority", "batch-processing", "background"]
                .iter()
                .map(|s| Arc::from(*s))
                .collect();

        for i in 0..count {
            let workflow_type = workflow_types[rng.gen_range(0..workflow_types.len())].clone();

            // PaymentProcessing: ~65% failure rate for insights "high failure rate" finding
            let status = if &*workflow_type == "PaymentProcessingWorkflow" {
                match rng.gen_range(0..100) {
                    0..=25 => WorkflowStatus::Completed,
                    26..=35 => WorkflowStatus::Running,
                    _ => WorkflowStatus::Failed,
                }
            } else {
                match rng.gen_range(0..100) {
                    0..=40 => WorkflowStatus::Completed,
                    41..=60 => WorkflowStatus::Running,
                    61..=75 => WorkflowStatus::Failed,
                    76..=85 => WorkflowStatus::Canceled,
                    86..=92 => WorkflowStatus::Terminated,
                    93..=97 => WorkflowStatus::TimedOut,
                    _ => WorkflowStatus::ContinuedAsNew,
                }
            };

            // Some Running workflows started many hours ago → stuck workflow finding
            let task_queue = if &*workflow_type == "DataProcessingWorkflow"
                && status == WorkflowStatus::Running
            {
                task_queues[2].clone() // "batch-processing"
            } else {
                task_queues[rng.gen_range(0..task_queues.len())].clone()
            };

            let start_offset = if status == WorkflowStatus::Running
                && &*workflow_type == "DataProcessingWorkflow"
            {
                // Make DataProcessing running workflows stuck (3-8 hours old)
                Duration::hours(rng.gen_range(3..9))
            } else {
                Duration::minutes(rng.gen_range(0..10080))
            };
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
                    if &*wf.workflow_type != wf_type {
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

        // Decision latency for first workflow task: varies by workflow type
        let first_wt_latency_ms = if &*workflow.workflow_type == "ReportGenerationWorkflow" {
            // Slow decision latency for report generation → triggers DecisionLatency finding
            rng.gen_range(600..1500)
        } else {
            rng.gen_range(5..100)
        };

        let mut events = vec![
            HistoryEvent {
                event_id: 1,
                event_type: "WorkflowExecutionStarted".to_string(),
                timestamp: workflow.start_time,
                details: serde_json::json!({"workflowType": &*workflow.workflow_type}),
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
                timestamp: workflow.start_time + Duration::milliseconds(first_wt_latency_ms / 2),
                details: serde_json::json!({"scheduled_event_id": 2}),
            },
            HistoryEvent {
                event_id: 4,
                event_type: "WorkflowTaskCompleted".to_string(),
                timestamp: workflow.start_time + Duration::milliseconds(first_wt_latency_ms),
                details: serde_json::json!({"scheduled_event_id": 2}),
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

            // Higher queue wait for batch-processing queue to trigger queue latency finding
            let queue_wait_ms = if &*workflow.task_queue == "batch-processing" {
                rng.gen_range(800..2500)
            } else {
                rng.gen_range(5..200)
            };
            let start_time = sched_time + Duration::milliseconds(queue_wait_ms);

            // Higher retry counts for SendEmail/ProcessPayment to trigger retry storm
            let attempt: i32 = if outcome == 3 {
                if activity_type == "SendEmail" || activity_type == "ProcessPayment" {
                    rng.gen_range(2..=5)
                } else {
                    rng.gen_range(1..=3)
                }
            } else if (activity_type == "SendEmail" || activity_type == "ProcessPayment")
                && rng.gen_bool(0.3)
            {
                rng.gen_range(2..=4)
            } else {
                1
            };

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
                    // Completed — some activities embed error signals in their output
                    // (simulating swallowed failures that are common in real workflows)
                    let result_json = if activity_type == "ProcessPayment" && rng.gen_bool(0.4) {
                        serde_json::json!({
                            "success": false,
                            "error": format!("Payment gateway error: connection timeout after 30s"),
                            "retryable": true,
                            "data": format!("result-{}", activity_type),
                        })
                    } else if activity_type == "SendEmail" && rng.gen_bool(0.3) {
                        serde_json::json!({
                            "sent": false,
                            "exception": "SmtpException: relay access denied",
                            "fallback": "queued for retry",
                            "data": format!("result-{}", activity_type),
                        })
                    } else if activity_type == "ValidateInput" && rng.gen_bool(0.25) {
                        serde_json::json!({
                            "valid": false,
                            "errors": ["field 'email' failed validation", "timeout checking external service"],
                            "data": format!("result-{}", activity_type),
                        })
                    } else {
                        serde_json::json!({
                            "success": true,
                            "data": format!("result-{}", activity_type),
                        })
                    };
                    events.push(HistoryEvent {
                        event_id,
                        event_type: "ActivityTaskCompleted".to_string(),
                        timestamp: close_time,
                        details: serde_json::json!({
                            "scheduled_event_id": scheduled_event_id,
                            "started_event_id": started_event_id,
                            "result": result_json,
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

        // Generate additional workflow task pairs at various points in the history
        // to provide more decision latency data points
        for _ in 0..rng.gen_range(1..=3) {
            let wt_offset = Duration::seconds(rng.gen_range(30..120));
            let wt_sched_ts = workflow.start_time + wt_offset;
            let wt_sched_eid = event_id;
            let wt_latency_ms = if &*workflow.workflow_type == "ReportGenerationWorkflow" {
                rng.gen_range(600..1500)
            } else {
                rng.gen_range(5..100)
            };

            events.push(HistoryEvent {
                event_id,
                event_type: "WorkflowTaskScheduled".to_string(),
                timestamp: wt_sched_ts,
                details: serde_json::json!({}),
            });
            event_id += 1;

            events.push(HistoryEvent {
                event_id,
                event_type: "WorkflowTaskStarted".to_string(),
                timestamp: wt_sched_ts + Duration::milliseconds(wt_latency_ms / 2),
                details: serde_json::json!({"scheduled_event_id": wt_sched_eid}),
            });
            event_id += 1;

            events.push(HistoryEvent {
                event_id,
                event_type: "WorkflowTaskCompleted".to_string(),
                timestamp: wt_sched_ts + Duration::milliseconds(wt_latency_ms),
                details: serde_json::json!({"scheduled_event_id": wt_sched_eid}),
            });
            event_id += 1;
        }

        // Generate signal events for NotificationWorkflow to trigger Signal Storm
        if &*workflow.workflow_type == "NotificationWorkflow" {
            let num_signals = rng.gen_range(50..=300);
            for s in 0..num_signals {
                events.push(HistoryEvent {
                    event_id,
                    event_type: "WorkflowExecutionSignaled".to_string(),
                    timestamp: workflow.start_time + Duration::seconds(s as i64),
                    details: serde_json::json!({
                        "signalName": "notify",
                        "input": {"target": format!("user-{}", s)},
                    }),
                });
                event_id += 1;
            }
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
                workflow_type: Arc::from("Test"),
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                close_time: None,
                task_queue: Arc::from("default"),
            },
            WorkflowSummary {
                workflow_id: "wf-2".to_string(),
                run_id: "run-2".to_string(),
                workflow_type: Arc::from("Test"),
                status: WorkflowStatus::Failed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: Arc::from("default"),
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
                workflow_type: Arc::from("Test"),
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                close_time: None,
                task_queue: Arc::from("default"),
            },
            WorkflowSummary {
                workflow_id: "wf-2".to_string(),
                run_id: "run-2".to_string(),
                workflow_type: Arc::from("Test"),
                status: WorkflowStatus::Failed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: Arc::from("default"),
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
