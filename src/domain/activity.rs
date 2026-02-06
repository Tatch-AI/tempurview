use crate::domain::{FailureInfo, HistoryEvent};
use chrono::{DateTime, TimeDelta, Utc};
use ratatui::style::Color;
use serde::Serialize;
use std::collections::HashMap;

/// Status of an activity execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Scheduled,
    Running,
    Completed,
    Failed,
    TimedOut,
    Canceled,
}

impl ActivityStatus {
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Scheduled => "PEND",
            Self::Running => "RUN",
            Self::Completed => "OK",
            Self::Failed => "FAIL",
            Self::TimedOut => "TIME",
            Self::Canceled => "CANC",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Scheduled => Color::DarkGray,
            Self::Running => Color::Blue,
            Self::Completed => Color::Green,
            Self::Failed => Color::Red,
            Self::TimedOut => Color::LightRed,
            Self::Canceled => Color::Yellow,
        }
    }
}

impl std::fmt::Display for ActivityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

/// A correlated activity execution (built from multiple history events)
#[derive(Debug, Clone, Serialize)]
pub struct ActivityExecution {
    pub activity_id: String,
    pub activity_type: String,
    pub status: ActivityStatus,
    pub task_queue: Option<String>,

    // Timestamps (from correlated events)
    pub scheduled_time: DateTime<Utc>,
    pub started_time: Option<DateTime<Utc>>,
    pub closed_time: Option<DateTime<Utc>>,

    // Computed durations
    #[serde(serialize_with = "serde_opt_timedelta_ms")]
    pub queue_wait: Option<TimeDelta>,
    #[serde(serialize_with = "serde_opt_timedelta_ms")]
    pub execution_time: Option<TimeDelta>,
    #[serde(serialize_with = "serde_opt_timedelta_ms")]
    pub total_time: Option<TimeDelta>,

    // Attempt/retry info
    pub attempt: i32,

    // Payloads (JSON)
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub failure: Option<FailureInfo>,

    // Event IDs for cross-reference
    pub scheduled_event_id: i64,
    pub started_event_id: Option<i64>,
    pub closed_event_id: Option<i64>,
}

fn serde_opt_timedelta_ms<S: serde::Serializer>(
    value: &Option<TimeDelta>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(td) => s.serialize_some(&td.num_milliseconds()),
        None => s.serialize_none(),
    }
}

/// Intermediate struct for correlating activity events during construction
struct ActivityBuilder {
    activity_id: String,
    activity_type: String,
    task_queue: Option<String>,
    scheduled_time: DateTime<Utc>,
    scheduled_event_id: i64,
    input: Option<serde_json::Value>,

    started_time: Option<DateTime<Utc>>,
    started_event_id: Option<i64>,
    attempt: i32,

    closed_time: Option<DateTime<Utc>>,
    closed_event_id: Option<i64>,
    status: ActivityStatus,
    output: Option<serde_json::Value>,
    failure: Option<FailureInfo>,
}

impl ActivityBuilder {
    fn build(self) -> ActivityExecution {
        let queue_wait = self
            .started_time
            .map(|s| s - self.scheduled_time);
        let execution_time = match (self.started_time, self.closed_time) {
            (Some(s), Some(c)) => Some(c - s),
            _ => None,
        };
        let total_time = self.closed_time.map(|c| c - self.scheduled_time);

        ActivityExecution {
            activity_id: self.activity_id,
            activity_type: self.activity_type,
            status: self.status,
            task_queue: self.task_queue,
            scheduled_time: self.scheduled_time,
            started_time: self.started_time,
            closed_time: self.closed_time,
            queue_wait,
            execution_time,
            total_time,
            attempt: self.attempt,
            input: self.input,
            output: self.output,
            failure: self.failure,
            scheduled_event_id: self.scheduled_event_id,
            started_event_id: self.started_event_id,
            closed_event_id: self.closed_event_id,
        }
    }
}

/// Build ActivityExecution list from raw HistoryEvents.
///
/// First pass: collect all ActivityTaskScheduled events into a HashMap keyed by event_id.
/// Second pass: match Started/Completed/Failed/TimedOut/Canceled events to their scheduled
/// event via the `scheduled_event_id` field in their details JSON.
/// Finally: compute durations and determine final status.
pub fn correlate_activities(events: &[HistoryEvent]) -> Vec<ActivityExecution> {
    let mut builders: HashMap<i64, ActivityBuilder> = HashMap::new();

    // First pass: collect all scheduled events
    for event in events {
        if event.event_type.contains("ActivityTaskScheduled") {
            let details = &event.details;
            let activity_id = details
                .get("activity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let activity_type = details
                .get("activity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let task_queue = details
                .get("task_queue")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let input = details.get("input").cloned();

            builders.insert(
                event.event_id,
                ActivityBuilder {
                    activity_id,
                    activity_type,
                    task_queue,
                    scheduled_time: event.timestamp,
                    scheduled_event_id: event.event_id,
                    input,
                    started_time: None,
                    started_event_id: None,
                    attempt: 0,
                    closed_time: None,
                    closed_event_id: None,
                    status: ActivityStatus::Scheduled,
                    output: None,
                    failure: None,
                },
            );
        }
    }

    // Second pass: match started/closed events to their scheduled event
    for event in events {
        let details = &event.details;
        let sched_id = details
            .get("scheduled_event_id")
            .and_then(|v| v.as_i64());

        if let Some(sched_id) = sched_id {
            if let Some(builder) = builders.get_mut(&sched_id) {
                if event.event_type.contains("ActivityTaskStarted") {
                    builder.started_time = Some(event.timestamp);
                    builder.started_event_id = Some(event.event_id);
                    builder.attempt = details
                        .get("attempt")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1) as i32;
                    if builder.status == ActivityStatus::Scheduled {
                        builder.status = ActivityStatus::Running;
                    }
                } else if event.event_type.contains("ActivityTaskCompleted") {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.output = details.get("result").cloned();
                    builder.status = ActivityStatus::Completed;
                } else if event.event_type.contains("ActivityTaskFailed") {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ActivityStatus::Failed;
                    // Extract failure info from details
                    if let Some(failure_obj) = details.get("failure") {
                        builder.failure = Some(FailureInfo {
                            message: failure_obj
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Activity failed")
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
                            message: "Activity failed".to_string(),
                            failure_type: "Unknown".to_string(),
                            stack_trace: None,
                            cause: None,
                        });
                    }
                } else if event.event_type.contains("ActivityTaskTimedOut") {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ActivityStatus::TimedOut;
                    builder.failure = Some(FailureInfo {
                        message: "Activity timed out".to_string(),
                        failure_type: "Timeout".to_string(),
                        stack_trace: None,
                        cause: None,
                    });
                } else if event.event_type.contains("ActivityTaskCanceled") {
                    builder.closed_time = Some(event.timestamp);
                    builder.closed_event_id = Some(event.event_id);
                    builder.status = ActivityStatus::Canceled;
                }
            }
        }
    }

    // Collect and sort by scheduled_event_id (chronological order)
    let mut activities: Vec<ActivityExecution> = builders
        .into_values()
        .map(|b| b.build())
        .collect();
    activities.sort_by_key(|a| a.scheduled_event_id);
    activities
}

/// Format a TimeDelta as a human-friendly string
pub fn format_duration(d: &TimeDelta) -> String {
    let total_secs = d.num_seconds();
    let millis = d.num_milliseconds() % 1000;

    if total_secs == 0 {
        format!("{}ms", millis)
    } else if total_secs < 60 {
        if millis > 0 {
            format!("{}.{}s", total_secs, millis / 100)
        } else {
            format!("{}s", total_secs)
        }
    } else if total_secs < 3600 {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if secs > 0 {
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}m", mins)
        }
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        if mins > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}h", hours)
        }
    }
}

/// Format elapsed time since a given timestamp
pub fn format_elapsed_since(since: &DateTime<Utc>) -> String {
    let elapsed = Utc::now() - *since;
    format!("({})", format_duration(&elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_event(event_id: i64, event_type: &str, timestamp: DateTime<Utc>, details: serde_json::Value) -> HistoryEvent {
        HistoryEvent {
            event_id,
            event_type: event_type.to_string(),
            timestamp,
            details,
        }
    }

    #[test]
    fn test_correlate_empty_events() {
        let activities = correlate_activities(&[]);
        assert!(activities.is_empty());
    }

    #[test]
    fn test_correlate_single_completed_activity() {
        let base = Utc::now();
        let events = vec![
            make_event(5, "ActivityTaskScheduled", base, serde_json::json!({
                "activity_id": "1",
                "activity_type": "SendEmail",
                "task_queue": "default",
                "input": {"to": "user@example.com"}
            })),
            make_event(6, "ActivityTaskStarted", base + Duration::milliseconds(50), serde_json::json!({
                "scheduled_event_id": 5,
                "attempt": 1
            })),
            make_event(7, "ActivityTaskCompleted", base + Duration::seconds(2), serde_json::json!({
                "scheduled_event_id": 5,
                "started_event_id": 6,
                "result": {"sent": true}
            })),
        ];

        let activities = correlate_activities(&events);
        assert_eq!(activities.len(), 1);

        let a = &activities[0];
        assert_eq!(a.activity_id, "1");
        assert_eq!(a.activity_type, "SendEmail");
        assert_eq!(a.status, ActivityStatus::Completed);
        assert_eq!(a.attempt, 1);
        assert!(a.queue_wait.is_some());
        assert!(a.execution_time.is_some());
        assert!(a.total_time.is_some());
        assert!(a.output.is_some());
        assert!(a.failure.is_none());
    }

    #[test]
    fn test_correlate_failed_activity() {
        let base = Utc::now();
        let events = vec![
            make_event(5, "ActivityTaskScheduled", base, serde_json::json!({
                "activity_id": "2",
                "activity_type": "ProcessPayment",
            })),
            make_event(6, "ActivityTaskStarted", base + Duration::milliseconds(10), serde_json::json!({
                "scheduled_event_id": 5,
                "attempt": 3
            })),
            make_event(7, "ActivityTaskFailed", base + Duration::seconds(3), serde_json::json!({
                "scheduled_event_id": 5,
                "started_event_id": 6,
                "failure": {
                    "message": "Payment declined",
                    "failure_type": "ApplicationFailure"
                }
            })),
        ];

        let activities = correlate_activities(&events);
        assert_eq!(activities.len(), 1);
        let a = &activities[0];
        assert_eq!(a.status, ActivityStatus::Failed);
        assert_eq!(a.attempt, 3);
        assert!(a.failure.is_some());
        assert_eq!(a.failure.as_ref().unwrap().message, "Payment declined");
    }

    #[test]
    fn test_correlate_pending_and_running_activities() {
        let base = Utc::now();
        let events = vec![
            // Scheduled only → Pending
            make_event(5, "ActivityTaskScheduled", base, serde_json::json!({
                "activity_id": "1",
                "activity_type": "GenerateReport",
            })),
            // Scheduled + Started → Running
            make_event(8, "ActivityTaskScheduled", base + Duration::seconds(1), serde_json::json!({
                "activity_id": "2",
                "activity_type": "UpdateInventory",
            })),
            make_event(9, "ActivityTaskStarted", base + Duration::seconds(2), serde_json::json!({
                "scheduled_event_id": 8,
                "attempt": 1
            })),
        ];

        let activities = correlate_activities(&events);
        assert_eq!(activities.len(), 2);
        assert_eq!(activities[0].status, ActivityStatus::Scheduled);
        assert_eq!(activities[1].status, ActivityStatus::Running);
    }

    #[test]
    fn test_correlate_multiple_activities() {
        let base = Utc::now();
        let events = vec![
            make_event(5, "ActivityTaskScheduled", base, serde_json::json!({
                "activity_id": "1",
                "activity_type": "A",
            })),
            make_event(6, "ActivityTaskStarted", base + Duration::milliseconds(5), serde_json::json!({
                "scheduled_event_id": 5,
                "attempt": 1
            })),
            make_event(7, "ActivityTaskCompleted", base + Duration::seconds(1), serde_json::json!({
                "scheduled_event_id": 5,
                "started_event_id": 6,
            })),
            make_event(10, "ActivityTaskScheduled", base + Duration::seconds(2), serde_json::json!({
                "activity_id": "2",
                "activity_type": "B",
            })),
            make_event(11, "ActivityTaskStarted", base + Duration::seconds(3), serde_json::json!({
                "scheduled_event_id": 10,
                "attempt": 1
            })),
            make_event(12, "ActivityTaskTimedOut", base + Duration::seconds(60), serde_json::json!({
                "scheduled_event_id": 10,
                "started_event_id": 11,
            })),
        ];

        let activities = correlate_activities(&events);
        assert_eq!(activities.len(), 2);
        assert_eq!(activities[0].activity_type, "A");
        assert_eq!(activities[0].status, ActivityStatus::Completed);
        assert_eq!(activities[1].activity_type, "B");
        assert_eq!(activities[1].status, ActivityStatus::TimedOut);
    }

    #[test]
    fn test_format_duration_millis() {
        let d = TimeDelta::milliseconds(42);
        assert_eq!(format_duration(&d), "42ms");
    }

    #[test]
    fn test_format_duration_seconds() {
        let d = TimeDelta::milliseconds(1200);
        assert_eq!(format_duration(&d), "1.2s");
    }

    #[test]
    fn test_format_duration_minutes() {
        let d = TimeDelta::seconds(125);
        assert_eq!(format_duration(&d), "2m 5s");
    }

    #[test]
    fn test_format_duration_hours() {
        let d = TimeDelta::seconds(3661);
        assert_eq!(format_duration(&d), "1h 1m");
    }
}
