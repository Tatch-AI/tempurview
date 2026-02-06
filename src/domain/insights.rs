use chrono::{DateTime, TimeDelta, Utc};
use ratatui::style::Color;
use serde::Serialize;

/// Severity level for an insight finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize)]
pub struct InsightFinding {
    pub severity: InsightSeverity,
    pub category: InsightCategory,
    pub title: String,
    pub detail: String,
    pub affected_entities: Vec<String>,
    pub computed_at: DateTime<Utc>,
    /// The specific values that triggered this finding, used for highlighting in the detail view
    pub trigger_terms: Vec<String>,
}

/// Result of an insights scan
#[derive(Debug, Clone, Serialize)]
pub struct InsightsResult {
    pub findings: Vec<InsightFinding>,
    pub workflows_scanned: usize,
    pub histories_fetched: usize,
    pub computed_at: DateTime<Utc>,
    #[serde(serialize_with = "serde_timedelta_ms")]
    pub scan_duration: TimeDelta,
}

fn serde_timedelta_ms<S: serde::Serializer>(
    value: &TimeDelta,
    s: S,
) -> Result<S::Ok, S::Error> {
    s.serialize_i64(value.num_milliseconds())
}

/// User-configurable settings for insights analysis.
/// Loaded from ~/.tempurview/config.toml [insights] section.
#[derive(Debug, Clone, Default)]
pub struct InsightsConfig {
    /// Phrases that suppress error-pattern matches (case-insensitive).
    /// If any allowlisted phrase appears in the same text as an error pattern,
    /// the match is skipped.
    pub allowlist: Vec<String>,
}

impl InsightsConfig {
    pub fn is_allowlisted(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();
        self.allowlist
            .iter()
            .any(|phrase| text_lower.contains(phrase.to_lowercase().as_str()))
    }
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

    #[test]
    fn test_allowlist_empty_allows_everything() {
        let config = InsightsConfig::default();
        assert!(!config.is_allowlisted("some text with error in it"));
    }

    #[test]
    fn test_allowlist_matches_case_insensitive() {
        let config = InsightsConfig {
            allowlist: vec!["errors and omissions".to_string()],
        };
        assert!(config.is_allowlisted("reviewing errors and omissions policy"));
        assert!(config.is_allowlisted("reviewing Errors and Omissions policy"));
        assert!(config.is_allowlisted("reviewing ERRORS AND OMISSIONS policy"));
    }

    #[test]
    fn test_allowlist_no_match() {
        let config = InsightsConfig {
            allowlist: vec!["errors and omissions".to_string()],
        };
        assert!(!config.is_allowlisted("there was an error in processing"));
    }
}
