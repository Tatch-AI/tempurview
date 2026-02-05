use chrono::{DateTime, Utc};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution status of a workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Running,
    Completed,
    Failed,
    Canceled,
    Terminated,
    TimedOut,
    ContinuedAsNew,
}

impl WorkflowStatus {
    /// Parse from Temporal CLI output string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "RUNNING" => Some(Self::Running),
            "COMPLETED" => Some(Self::Completed),
            "FAILED" => Some(Self::Failed),
            "CANCELED" => Some(Self::Canceled),
            "TERMINATED" => Some(Self::Terminated),
            "TIMED_OUT" | "TIMEDOUT" => Some(Self::TimedOut),
            "CONTINUED_AS_NEW" | "CONTINUEDASNEW" => Some(Self::ContinuedAsNew),
            _ => None,
        }
    }

    /// Convert to query string format
    pub fn as_query_value(&self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Canceled => "Canceled",
            Self::Terminated => "Terminated",
            Self::TimedOut => "TimedOut",
            Self::ContinuedAsNew => "ContinuedAsNew",
        }
    }

    /// All possible statuses (for iteration)
    pub fn all() -> &'static [Self] {
        &[
            Self::Running,
            Self::Completed,
            Self::Failed,
            Self::Canceled,
            Self::Terminated,
            Self::TimedOut,
            Self::ContinuedAsNew,
        ]
    }

    /// Display color for this status
    pub fn color(&self) -> Color {
        match self {
            Self::Running => Color::Blue,
            Self::Completed => Color::Green,
            Self::Failed => Color::Red,
            Self::Canceled => Color::Yellow,
            Self::Terminated => Color::Magenta,
            Self::TimedOut => Color::LightRed,
            Self::ContinuedAsNew => Color::Cyan,
        }
    }

    /// Short display name
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Running => "RUN",
            Self::Completed => "OK",
            Self::Failed => "FAIL",
            Self::Canceled => "CANC",
            Self::Terminated => "TERM",
            Self::TimedOut => "TIME",
            Self::ContinuedAsNew => "CONT",
        }
    }
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_query_value())
    }
}

/// A workflow execution summary (from list command)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowSummary {
    pub workflow_id: String,
    pub run_id: String,
    pub workflow_type: String,
    pub status: WorkflowStatus,
    pub start_time: DateTime<Utc>,
    pub close_time: Option<DateTime<Utc>>,
    pub task_queue: String,
}

/// Detailed workflow information (from describe/show commands)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDetail {
    pub summary: WorkflowSummary,
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub failure: Option<FailureInfo>,
    pub history_length: u64,
    pub memo: HashMap<String, serde_json::Value>,
    pub search_attributes: HashMap<String, serde_json::Value>,
}

/// Failure information for failed workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureInfo {
    pub message: String,
    pub failure_type: String,
    pub stack_trace: Option<String>,
    pub cause: Option<Box<FailureInfo>>,
}

/// A history event from workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub event_id: i64,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub details: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_from_str() {
        assert_eq!(
            WorkflowStatus::from_str("RUNNING"),
            Some(WorkflowStatus::Running)
        );
        assert_eq!(
            WorkflowStatus::from_str("running"),
            Some(WorkflowStatus::Running)
        );
        assert_eq!(
            WorkflowStatus::from_str("COMPLETED"),
            Some(WorkflowStatus::Completed)
        );
        assert_eq!(
            WorkflowStatus::from_str("FAILED"),
            Some(WorkflowStatus::Failed)
        );
        assert_eq!(
            WorkflowStatus::from_str("TIMED_OUT"),
            Some(WorkflowStatus::TimedOut)
        );
        assert_eq!(
            WorkflowStatus::from_str("CONTINUED_AS_NEW"),
            Some(WorkflowStatus::ContinuedAsNew)
        );
        assert_eq!(WorkflowStatus::from_str("INVALID"), None);
    }

    #[test]
    fn test_status_query_value() {
        assert_eq!(WorkflowStatus::Running.as_query_value(), "Running");
        assert_eq!(WorkflowStatus::Failed.as_query_value(), "Failed");
        assert_eq!(WorkflowStatus::TimedOut.as_query_value(), "TimedOut");
    }

    #[test]
    fn test_all_statuses() {
        let all = WorkflowStatus::all();
        assert_eq!(all.len(), 7);
        assert!(all.contains(&WorkflowStatus::Running));
        assert!(all.contains(&WorkflowStatus::Failed));
    }
}
