use chrono::{DateTime, TimeDelta, Utc};
use ratatui::style::Color;

/// Severity level for an insight finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InsightSeverity {
    Info,
    Warning,
    Critical,
}

impl InsightSeverity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Critical => "CRIT",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Info => Color::Cyan,
            Self::Warning => Color::Yellow,
            Self::Critical => Color::Red,
        }
    }
}

impl std::fmt::Display for InsightSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Category of an insight finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsightCategory {
    FailureRate,
    RetryStorm,
    ActivityRetry,
    QueueLatency,
    StuckWorkflow,
    ActivityFailure,
    TypeAnomaly,
    LongRunningActivity,
    ErrorInOutput,
    ChildWorkflowFailure,
    ChildWorkflowLatency,
}

impl InsightCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FailureRate => "Failure Rate",
            Self::RetryStorm => "Retry Storm",
            Self::ActivityRetry => "Activity Retry",
            Self::QueueLatency => "Queue Latency",
            Self::StuckWorkflow => "Stuck Workflow",
            Self::ActivityFailure => "Activity Failure",
            Self::TypeAnomaly => "Type Anomaly",
            Self::LongRunningActivity => "Long Activity",
            Self::ErrorInOutput => "Error in I/O",
            Self::ChildWorkflowFailure => "Child WF Failure",
            Self::ChildWorkflowLatency => "Child WF Latency",
        }
    }
}

impl std::fmt::Display for InsightCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A single insight finding
#[derive(Debug, Clone)]
pub struct InsightFinding {
    pub severity: InsightSeverity,
    pub category: InsightCategory,
    pub title: String,
    pub detail: String,
    pub affected_entities: Vec<String>,
    pub computed_at: DateTime<Utc>,
}

/// Result of an insights scan
#[derive(Debug, Clone)]
pub struct InsightsResult {
    pub findings: Vec<InsightFinding>,
    pub workflows_scanned: usize,
    pub histories_fetched: usize,
    pub computed_at: DateTime<Utc>,
    pub scan_duration: TimeDelta,
}

/// Threshold constants for insight detection
pub struct InsightThresholds;

impl InsightThresholds {
    // Failure rate thresholds (for types with >= MIN_WORKFLOWS_FOR_RATE)
    pub const MIN_WORKFLOWS_FOR_RATE: usize = 3;
    pub const FAILURE_RATE_INFO: f64 = 0.10;
    pub const FAILURE_RATE_WARNING: f64 = 0.25;
    pub const FAILURE_RATE_CRITICAL: f64 = 0.50;

    // All-failed type (minimum workflows to trigger)
    pub const ALL_FAILED_MIN_WORKFLOWS: usize = 2;

    // Stuck workflow thresholds (elapsed hours)
    pub const STUCK_WARNING_HOURS: i64 = 2;
    pub const STUCK_CRITICAL_HOURS: i64 = 6;

    // Retry storm thresholds
    pub const RETRY_MIN_INSTANCES: usize = 3;
    pub const RETRY_WARNING_AVG: f64 = 2.0;
    pub const RETRY_CRITICAL_AVG: f64 = 3.0;

    // Queue latency thresholds (milliseconds)
    pub const QUEUE_LATENCY_WARNING_MS: i64 = 1000;
    pub const QUEUE_LATENCY_CRITICAL_MS: i64 = 5000;

    // Activity failure hotspot thresholds
    pub const ACTIVITY_FAILURE_WARNING: usize = 3;
    pub const ACTIVITY_FAILURE_CRITICAL: usize = 5;

    // Long-running activity thresholds (minutes)
    pub const LONG_ACTIVITY_WARNING_MINS: i64 = 30;
    pub const LONG_ACTIVITY_CRITICAL_MINS: i64 = 120;

    // Activity retry: any activity with attempt >= this is flagged
    pub const ACTIVITY_RETRY_MIN_ATTEMPT: i32 = 2;

    // Error-in-output: minimum matches to trigger
    pub const ERROR_IN_OUTPUT_WARNING: usize = 2;
    pub const ERROR_IN_OUTPUT_CRITICAL: usize = 5;

    // Child workflow failure thresholds (grouped by child type)
    pub const CHILD_WF_FAILURE_WARNING: usize = 2;
    pub const CHILD_WF_FAILURE_CRITICAL: usize = 5;

    // Child workflow start latency thresholds (milliseconds)
    pub const CHILD_WF_LATENCY_WARNING_MS: i64 = 2000;
    pub const CHILD_WF_LATENCY_CRITICAL_MS: i64 = 10000;

    // Sampling
    pub const MAX_HISTORY_SAMPLES: usize = 30;

    // Error patterns to scan for in activity I/O (case-insensitive)
    pub const ERROR_PATTERNS: &'static [&'static str] = &[
        "error",
        "exception",
        "failed",
        "failure",
        "timed out",
        "timeout",
        "panic",
        "fatal",
        "unhandled",
        "traceback",
        "stack trace",
        "errno",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(InsightSeverity::Info < InsightSeverity::Warning);
        assert!(InsightSeverity::Warning < InsightSeverity::Critical);
    }

    #[test]
    fn test_severity_labels() {
        assert_eq!(InsightSeverity::Info.label(), "INFO");
        assert_eq!(InsightSeverity::Warning.label(), "WARN");
        assert_eq!(InsightSeverity::Critical.label(), "CRIT");
    }

    #[test]
    fn test_category_labels() {
        assert_eq!(InsightCategory::FailureRate.label(), "Failure Rate");
        assert_eq!(InsightCategory::RetryStorm.label(), "Retry Storm");
        assert_eq!(InsightCategory::ActivityRetry.label(), "Activity Retry");
        assert_eq!(InsightCategory::StuckWorkflow.label(), "Stuck Workflow");
        assert_eq!(InsightCategory::ErrorInOutput.label(), "Error in I/O");
        assert_eq!(InsightCategory::ChildWorkflowFailure.label(), "Child WF Failure");
        assert_eq!(InsightCategory::ChildWorkflowLatency.label(), "Child WF Latency");
    }
}
