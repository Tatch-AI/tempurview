use crate::domain::{FailureInfo, HistoryEvent};
use chrono::{DateTime, TimeDelta, Utc};
use ratatui::style::Color;
use std::collections::HashMap;

/// Status of a child workflow execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildWorkflowStatus {
    Initiated,
    Started,
    Completed,
    Failed,
    TimedOut,
    Canceled,
    Terminated,
    StartFailed,
}

impl ChildWorkflowStatus {
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Initiated => "INIT",
            Self::Started => "RUN",
            Self::Completed => "OK",
            Self::Failed => "FAIL",
            Self::TimedOut => "TIME",
            Self::Canceled => "CANC",
            Self::Terminated => "TERM",
            Self::StartFailed => "SFAIL",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Initiated => Color::DarkGray,
            Self::Started => Color::Blue,
            Self::Completed => Color::Green,
            Self::Failed => Color::Red,
            Self::TimedOut => Color::LightRed,
            Self::Canceled => Color::Yellow,
            Self::Terminated => Color::Magenta,
            Self::StartFailed => Color::Red,
        }
    }
}

impl std::fmt::Display for ChildWorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

/// A correlated child workflow execution (built from multiple history events)
#[derive(Debug, Clone)]
pub struct ChildWorkflowExecution {
    pub workflow_id: String,
    pub workflow_type: String,
    pub status: ChildWorkflowStatus,
    pub namespace: Option<String>,
    pub run_id: Option<String>,

    // Timestamps
    pub initiated_time: DateTime<Utc>,
    pub started_time: Option<DateTime<Utc>>,
    pub closed_time: Option<DateTime<Utc>>,

    // Computed durations
    pub start_latency: Option<TimeDelta>,  // started - initiated
    pub execution_time: Option<TimeDelta>, // closed - started
    pub total_time: Option<TimeDelta>,     // closed - initiated

    // Failure info
    pub failure: Option<FailureInfo>,

    // Event IDs
    pub initiated_event_id: i64,
    pub started_event_id: Option<i64>,
    pub closed_event_id: Option<i64>,
}

/// Intermediate struct for correlating child workflow events during construction
struct ChildWorkflowBuilder {
    workflow_id: String,
    workflow_type: String,
    namespace: Option<String>,
    initiated_time: DateTime<Utc>,
    initiated_event_id: i64,

    run_id: Option<String>,
    started_time: Option<DateTime<Utc>>,
    started_event_id: Option<i64>,

    closed_time: Option<DateTime<Utc>>,
    closed_event_id: Option<i64>,
    status: ChildWorkflowStatus,
    failure: Option<FailureInfo>,
}

impl ChildWorkflowBuilder {
    fn build(self) -> ChildWorkflowExecution {
        let start_latency = self.started_time.map(|s| s - self.initiated_time);
        let execution_time = match (self.started_time, self.closed_time) {
            (Some(s), Some(c)) => Some(c - s),
            _ => None,
        };
        let total_time = self.closed_time.map(|c| c - self.initiated_time);

        ChildWorkflowExecution {
            workflow_id: self.workflow_id,
            workflow_type: self.workflow_type,
            status: self.status,
            namespace: self.namespace,
            run_id: self.run_id,
            initiated_time: self.initiated_time,
            started_time: self.started_time,
            closed_time: self.closed_time,
            start_latency,
            execution_time,
            total_time,
            failure: self.failure,
            initiated_event_id: self.initiated_event_id,
            started_event_id: self.started_event_id,
            closed_event_id: self.closed_event_id,
        }
    }
}

/// Build ChildWorkflowExecution list from raw HistoryEvents.
///
/// First pass: collect all StartChildWorkflowExecutionInitiated events into a HashMap keyed by event_id.
/// Second pass: match Started/Completed/Failed/Canceled/TimedOut/Terminated events to their
/// initiated event via the `initiated_event_id` field in their details JSON.
pub fn correlate_child_workflows(events: &[HistoryEvent]) -> Vec<ChildWorkflowExecution> {
    let mut builders: HashMap<i64, ChildWorkflowBuilder> = HashMap::new();

    // First pass: collect all initiated events
    for event in events {
        if event.event_type.contains("StartChildWorkflowExecutionInitiated") {
            let details = &event.details;
            let workflow_id = details
                .get("workflow_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let workflow_type = details
                .get("workflow_type")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let namespace = details
                .get("namespace")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            builders.insert(
                event.event_id,
                ChildWorkflowBuilder {
                    workflow_id,
                    workflow_type,
                    namespace,
                    initiated_time: event.timestamp,
                    initiated_event_id: event.event_id,
                    run_id: None,
                    started_time: None,
                    started_event_id: None,
                    closed_time: None,
                    closed_event_id: None,
                    status: ChildWorkflowStatus::Initiated,
                    failure: None,
                },
            );
        }
    }

    // Second pass: match subsequent events to their initiated event
    for event in events {
        let details = &event.details;
        let initiated_id = details
            .get("initiated_event_id")
            .and_then(|v| v.as_i64());

        if let Some(initiated_id) = initiated_id {
            if let Some(builder) = builders.get_mut(&initiated_id) {
                let run_id = details
                    .get("run_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Check StartChildWorkflowExecutionFailed before ChildWorkflowExecutionFailed
                // because the latter is a substring of the former
                if event
                    .event_type
                    .contains("StartChildWorkflowExecutionFailed")
                {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ChildWorkflowStatus::StartFailed;
                    builder.failure = Some(FailureInfo {
                        message: "Child workflow failed to start".to_string(),
                        failure_type: "StartFailed".to_string(),
                        stack_trace: None,
                        cause: None,
                    });
                } else if event.event_type.contains("ChildWorkflowExecutionStarted") {
                    builder.started_time = Some(event.timestamp);
                    builder.started_event_id = Some(event.event_id);
                    if let Some(rid) = run_id {
                        builder.run_id = Some(rid);
                    }
                    if builder.status == ChildWorkflowStatus::Initiated {
                        builder.status = ChildWorkflowStatus::Started;
                    }
                } else if event.event_type.contains("ChildWorkflowExecutionCompleted") {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ChildWorkflowStatus::Completed;
                } else if event
                    .event_type
                    .contains("ChildWorkflowExecutionFailed")
                {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ChildWorkflowStatus::Failed;
                    if let Some(failure_obj) = details.get("failure") {
                        builder.failure = Some(FailureInfo {
                            message: failure_obj
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Child workflow failed")
                                .to_string(),
                            failure_type: failure_obj
                                .get("failure_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            stack_trace: failure_obj
                                .get("stack_trace")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            cause: None,
                        });
                    } else {
                        builder.failure = Some(FailureInfo {
                            message: "Child workflow failed".to_string(),
                            failure_type: "Unknown".to_string(),
                            stack_trace: None,
                            cause: None,
                        });
                    }
                } else if event
                    .event_type
                    .contains("ChildWorkflowExecutionTimedOut")
                {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ChildWorkflowStatus::TimedOut;
                    builder.failure = Some(FailureInfo {
                        message: "Child workflow timed out".to_string(),
                        failure_type: "Timeout".to_string(),
                        stack_trace: None,
                        cause: None,
                    });
                } else if event
                    .event_type
                    .contains("ChildWorkflowExecutionCanceled")
                {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ChildWorkflowStatus::Canceled;
                } else if event
                    .event_type
                    .contains("ChildWorkflowExecutionTerminated")
                {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ChildWorkflowStatus::Terminated;
                }
            }
        }
    }

    // Collect and sort by initiated_event_id (chronological order)
    let mut child_workflows: Vec<ChildWorkflowExecution> = builders
        .into_values()
        .map(|b| b.build())
        .collect();
    child_workflows.sort_by_key(|c| c.initiated_event_id);
    child_workflows
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_event(
        event_id: i64,
        event_type: &str,
        timestamp: DateTime<Utc>,
        details: serde_json::Value,
    ) -> HistoryEvent {
        HistoryEvent {
            event_id,
            event_type: event_type.to_string(),
            timestamp,
            details,
        }
    }

    #[test]
    fn test_correlate_empty() {
        let child_workflows = correlate_child_workflows(&[]);
        assert!(child_workflows.is_empty());
    }

    #[test]
    fn test_correlate_single_completed_child() {
        let base = Utc::now();
        let events = vec![
            make_event(
                10,
                "StartChildWorkflowExecutionInitiated",
                base,
                serde_json::json!({
                    "workflow_id": "child-wf-1",
                    "workflow_type": "ProcessOrder",
                    "namespace": "default",
                }),
            ),
            make_event(
                11,
                "ChildWorkflowExecutionStarted",
                base + Duration::milliseconds(100),
                serde_json::json!({
                    "initiated_event_id": 10,
                    "run_id": "run-abc",
                }),
            ),
            make_event(
                15,
                "ChildWorkflowExecutionCompleted",
                base + Duration::seconds(5),
                serde_json::json!({
                    "initiated_event_id": 10,
                    "started_event_id": 11,
                    "result": {"order_id": 123},
                }),
            ),
        ];

        let child_workflows = correlate_child_workflows(&events);
        assert_eq!(child_workflows.len(), 1);

        let cw = &child_workflows[0];
        assert_eq!(cw.workflow_id, "child-wf-1");
        assert_eq!(cw.workflow_type, "ProcessOrder");
        assert_eq!(cw.status, ChildWorkflowStatus::Completed);
        assert_eq!(cw.namespace.as_deref(), Some("default"));
        assert_eq!(cw.run_id.as_deref(), Some("run-abc"));
        assert!(cw.start_latency.is_some());
        assert!(cw.execution_time.is_some());
        assert!(cw.total_time.is_some());
        assert!(cw.failure.is_none());
        assert_eq!(cw.initiated_event_id, 10);
        assert_eq!(cw.started_event_id, Some(11));
        assert_eq!(cw.closed_event_id, Some(15));
    }

    #[test]
    fn test_correlate_failed_child() {
        let base = Utc::now();
        let events = vec![
            make_event(
                10,
                "StartChildWorkflowExecutionInitiated",
                base,
                serde_json::json!({
                    "workflow_id": "child-wf-2",
                    "workflow_type": "ChargePayment",
                }),
            ),
            make_event(
                11,
                "ChildWorkflowExecutionStarted",
                base + Duration::milliseconds(50),
                serde_json::json!({
                    "initiated_event_id": 10,
                    "run_id": "run-def",
                }),
            ),
            make_event(
                14,
                "ChildWorkflowExecutionFailed",
                base + Duration::seconds(3),
                serde_json::json!({
                    "initiated_event_id": 10,
                    "started_event_id": 11,
                    "failure": {
                        "message": "Payment declined",
                        "failure_type": "ApplicationFailure",
                    },
                }),
            ),
        ];

        let child_workflows = correlate_child_workflows(&events);
        assert_eq!(child_workflows.len(), 1);

        let cw = &child_workflows[0];
        assert_eq!(cw.status, ChildWorkflowStatus::Failed);
        assert!(cw.failure.is_some());
        assert_eq!(cw.failure.as_ref().unwrap().message, "Payment declined");
        assert_eq!(
            cw.failure.as_ref().unwrap().failure_type,
            "ApplicationFailure"
        );
    }

    #[test]
    fn test_correlate_pending_and_running() {
        let base = Utc::now();
        let events = vec![
            // Initiated only → Initiated
            make_event(
                10,
                "StartChildWorkflowExecutionInitiated",
                base,
                serde_json::json!({
                    "workflow_id": "child-wf-pending",
                    "workflow_type": "SlowStart",
                }),
            ),
            // Initiated + Started → Started (Running)
            make_event(
                20,
                "StartChildWorkflowExecutionInitiated",
                base + Duration::seconds(1),
                serde_json::json!({
                    "workflow_id": "child-wf-running",
                    "workflow_type": "LongProcess",
                }),
            ),
            make_event(
                21,
                "ChildWorkflowExecutionStarted",
                base + Duration::seconds(2),
                serde_json::json!({
                    "initiated_event_id": 20,
                    "run_id": "run-ghi",
                }),
            ),
        ];

        let child_workflows = correlate_child_workflows(&events);
        assert_eq!(child_workflows.len(), 2);

        assert_eq!(child_workflows[0].workflow_id, "child-wf-pending");
        assert_eq!(child_workflows[0].status, ChildWorkflowStatus::Initiated);
        assert!(child_workflows[0].started_time.is_none());

        assert_eq!(child_workflows[1].workflow_id, "child-wf-running");
        assert_eq!(child_workflows[1].status, ChildWorkflowStatus::Started);
        assert!(child_workflows[1].started_time.is_some());
    }

    #[test]
    fn test_correlate_start_failed() {
        let base = Utc::now();
        let events = vec![
            make_event(
                10,
                "StartChildWorkflowExecutionInitiated",
                base,
                serde_json::json!({
                    "workflow_id": "child-wf-bad",
                    "workflow_type": "InvalidWorkflow",
                }),
            ),
            make_event(
                11,
                "StartChildWorkflowExecutionFailed",
                base + Duration::milliseconds(20),
                serde_json::json!({
                    "initiated_event_id": 10,
                    "workflow_id": "child-wf-bad",
                    "cause": 1,
                }),
            ),
        ];

        let child_workflows = correlate_child_workflows(&events);
        assert_eq!(child_workflows.len(), 1);

        let cw = &child_workflows[0];
        assert_eq!(cw.status, ChildWorkflowStatus::StartFailed);
        assert!(cw.failure.is_some());
        assert_eq!(
            cw.failure.as_ref().unwrap().message,
            "Child workflow failed to start"
        );
        assert!(cw.started_time.is_none());
        assert!(cw.closed_time.is_some());
    }
}
