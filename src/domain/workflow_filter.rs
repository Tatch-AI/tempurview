use super::WorkflowStatus;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

/// Preset date ranges for quick selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateRangePreset {
    LastHour,
    Last6Hours,
    Last24Hours,
    Last3Days,
    Last7Days,
    Last30Days,
}

impl DateRangePreset {
    pub fn duration(&self) -> TimeDelta {
        match self {
            Self::LastHour => TimeDelta::hours(1),
            Self::Last6Hours => TimeDelta::hours(6),
            Self::Last24Hours => TimeDelta::hours(24),
            Self::Last3Days => TimeDelta::days(3),
            Self::Last7Days => TimeDelta::days(7),
            Self::Last30Days => TimeDelta::days(30),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::LastHour => "Last 1 hour",
            Self::Last6Hours => "Last 6 hours",
            Self::Last24Hours => "Last 24 hours",
            Self::Last3Days => "Last 3 days",
            Self::Last7Days => "Last 7 days",
            Self::Last30Days => "Last 30 days",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::LastHour => "1h",
            Self::Last6Hours => "6h",
            Self::Last24Hours => "24h",
            Self::Last3Days => "3d",
            Self::Last7Days => "7d",
            Self::Last30Days => "30d",
        }
    }
}

/// Parse a user-provided date input string into a DateTime<Utc>.
///
/// Accepts:
/// - Relative: `30m`, `2h`, `3d`, `1w`
/// - RFC 3339: `2024-01-15T10:00:00Z`
/// - Date only: `2024-01-15`
pub fn parse_date_input(input: &str) -> Option<DateTime<Utc>> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    // Try relative: number followed by m/h/d/w
    if input.len() >= 2 {
        let (num_part, unit) = input.split_at(input.len() - 1);
        if let Ok(n) = num_part.parse::<i64>() {
            let delta = match unit {
                "m" => Some(TimeDelta::minutes(n)),
                "h" => Some(TimeDelta::hours(n)),
                "d" => Some(TimeDelta::days(n)),
                "w" => Some(TimeDelta::weeks(n)),
                _ => None,
            };
            if let Some(d) = delta {
                return Some(Utc::now() - d);
            }
        }
    }

    // Try RFC 3339
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try date only (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|ndt| ndt.and_utc());
    }

    None
}

/// A filter for querying workflows
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowFilter {
    pub status: Option<WorkflowStatus>,
    pub workflow_type: Option<String>,
    pub workflow_id_prefix: Option<String>,
    pub start_time_after: Option<DateTime<Utc>>,
    pub start_time_before: Option<DateTime<Utc>>,
    pub close_time_after: Option<DateTime<Utc>>,
    pub close_time_before: Option<DateTime<Utc>>,
}

impl WorkflowFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(mut self, status: WorkflowStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_type(mut self, workflow_type: impl Into<String>) -> Self {
        self.workflow_type = Some(workflow_type.into());
        self
    }

    pub fn with_workflow_id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.workflow_id_prefix = Some(prefix.into());
        self
    }

    pub fn with_start_time_after(mut self, time: DateTime<Utc>) -> Self {
        self.start_time_after = Some(time);
        self
    }

    pub fn with_start_time_before(mut self, time: DateTime<Utc>) -> Self {
        self.start_time_before = Some(time);
        self
    }

    /// Build Temporal query string from filter
    /// Returns None if filter is empty
    pub fn to_query(&self) -> Option<String> {
        let mut conditions = Vec::new();

        if let Some(status) = &self.status {
            conditions.push(format!("ExecutionStatus='{}'", status.as_query_value()));
        }

        if let Some(workflow_type) = &self.workflow_type {
            conditions.push(format!("WorkflowType='{}'", workflow_type));
        }

        if let Some(prefix) = &self.workflow_id_prefix {
            conditions.push(format!("WorkflowId STARTS_WITH '{}'", prefix));
        }

        if let Some(time) = &self.start_time_after {
            conditions.push(format!("StartTime > '{}'", time.to_rfc3339()));
        }

        if let Some(time) = &self.start_time_before {
            conditions.push(format!("StartTime < '{}'", time.to_rfc3339()));
        }

        if let Some(time) = &self.close_time_after {
            conditions.push(format!("CloseTime > '{}'", time.to_rfc3339()));
        }

        if let Some(time) = &self.close_time_before {
            conditions.push(format!("CloseTime < '{}'", time.to_rfc3339()));
        }

        if conditions.is_empty() {
            None
        } else {
            Some(conditions.join(" AND "))
        }
    }

    /// Parse a raw query string into a filter (best effort)
    pub fn from_query(query: &str) -> Self {
        let mut filter = Self::new();

        // Simple parsing - look for known patterns
        let query_upper = query.to_uppercase();

        // Parse ExecutionStatus
        if let Some(start) = query_upper.find("EXECUTIONSTATUS=") {
            let rest = &query[start + 17..]; // Skip "ExecutionStatus='"
            if let Some(end) = rest.find('\'') {
                let status_str = &rest[..end];
                if let Ok(status) = status_str.parse() {
                    filter.status = Some(status);
                }
            }
        }

        // Parse WorkflowType
        if let Some(start) = query_upper.find("WORKFLOWTYPE=") {
            let rest = &query[start + 14..]; // Skip "WorkflowType='"
            if let Some(end) = rest.find('\'') {
                filter.workflow_type = Some(rest[..end].to_string());
            }
        }

        filter
    }

    /// Check if any date range field is set
    pub fn has_date_range(&self) -> bool {
        self.start_time_after.is_some()
            || self.start_time_before.is_some()
            || self.close_time_after.is_some()
            || self.close_time_before.is_some()
    }

    /// Check if the filter has any conditions (sort order doesn't count as a filter condition)
    pub fn is_empty(&self) -> bool {
        self.status.is_none()
            && self.workflow_type.is_none()
            && self.workflow_id_prefix.is_none()
            && self.start_time_after.is_none()
            && self.start_time_before.is_none()
            && self.close_time_after.is_none()
            && self.close_time_before.is_none()
    }

    /// Get a human-readable description of the filter.
    /// Pass an optional date range label (e.g. "24h ago") for display.
    pub fn description(&self) -> String {
        self.description_with_date_label(None)
    }

    pub fn description_with_date_label(&self, date_label: Option<&str>) -> String {
        let mut parts = Vec::new();

        if let Some(status) = &self.status {
            parts.push(format!("{}", status));
        }

        if let Some(wf_type) = &self.workflow_type {
            parts.push(format!("type:{}", wf_type));
        }

        if let Some(prefix) = &self.workflow_id_prefix {
            parts.push(format!("id:{}*", prefix));
        }

        if let Some(label) = date_label {
            parts.push(format!("since:{}", label));
        }

        if parts.is_empty() {
            "All workflows".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_empty_filter_produces_no_query() {
        let filter = WorkflowFilter::new();
        assert_eq!(filter.to_query(), None);
        assert!(filter.is_empty());
    }

    #[test]
    fn test_status_filter() {
        let filter = WorkflowFilter::new().with_status(WorkflowStatus::Failed);
        assert_eq!(filter.to_query(), Some("ExecutionStatus='Failed'".into()));
        assert!(!filter.is_empty());
    }

    #[test]
    fn test_type_filter() {
        let filter = WorkflowFilter::new().with_type("TestWorkflow");
        assert_eq!(
            filter.to_query(),
            Some("WorkflowType='TestWorkflow'".into())
        );
    }

    #[test]
    fn test_combined_filter() {
        let filter = WorkflowFilter::new()
            .with_status(WorkflowStatus::Failed)
            .with_type("EmailGenerationWorkflow");
        assert_eq!(
            filter.to_query(),
            Some("ExecutionStatus='Failed' AND WorkflowType='EmailGenerationWorkflow'".into())
        );
    }

    #[test]
    fn test_workflow_id_prefix_filter() {
        let filter = WorkflowFilter::new().with_workflow_id_prefix("user-123");
        assert_eq!(
            filter.to_query(),
            Some("WorkflowId STARTS_WITH 'user-123'".into())
        );
    }

    #[test]
    fn test_filter_description() {
        let filter = WorkflowFilter::new();
        assert_eq!(filter.description(), "All workflows");

        let filter = WorkflowFilter::new().with_status(WorkflowStatus::Running);
        assert_eq!(filter.description(), "Running");

        let filter = WorkflowFilter::new()
            .with_status(WorkflowStatus::Failed)
            .with_type("TestWorkflow");
        assert_eq!(filter.description(), "Failed, type:TestWorkflow");
    }

    #[test]
    fn test_from_query_parses_status() {
        let filter = WorkflowFilter::from_query("ExecutionStatus='Failed'");
        assert_eq!(filter.status, Some(WorkflowStatus::Failed));
    }

    #[test]
    fn test_date_range_preset_duration() {
        assert_eq!(DateRangePreset::LastHour.duration(), TimeDelta::hours(1));
        assert_eq!(DateRangePreset::Last7Days.duration(), TimeDelta::days(7));
    }

    #[test]
    fn test_date_range_preset_labels() {
        assert_eq!(DateRangePreset::Last24Hours.label(), "Last 24 hours");
        assert_eq!(DateRangePreset::Last24Hours.short_label(), "24h");
        assert_eq!(DateRangePreset::Last3Days.short_label(), "3d");
    }

    #[test]
    fn test_parse_date_input_relative() {
        let now = Utc::now();

        let result = parse_date_input("2h").unwrap();
        let diff = (now - result).num_minutes();
        assert!((118..=122).contains(&diff), "Expected ~120min, got {}", diff);

        let result = parse_date_input("3d").unwrap();
        let diff = (now - result).num_hours();
        assert!((71..=73).contains(&diff), "Expected ~72h, got {}", diff);

        let result = parse_date_input("30m").unwrap();
        let diff = (now - result).num_minutes();
        assert!((29..=31).contains(&diff), "Expected ~30min, got {}", diff);

        let result = parse_date_input("1w").unwrap();
        let diff = (now - result).num_days();
        assert!((6..=8).contains(&diff), "Expected ~7d, got {}", diff);
    }

    #[test]
    fn test_parse_date_input_rfc3339() {
        let result = parse_date_input("2024-01-15T10:00:00Z").unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 1);
        assert_eq!(result.day(), 15);
    }

    #[test]
    fn test_parse_date_input_date_only() {
        let result = parse_date_input("2024-01-15").unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 1);
        assert_eq!(result.day(), 15);
    }

    #[test]
    fn test_parse_date_input_invalid() {
        assert!(parse_date_input("").is_none());
        assert!(parse_date_input("garbage").is_none());
        assert!(parse_date_input("abc123").is_none());
    }

    #[test]
    fn test_has_date_range() {
        let filter = WorkflowFilter::new();
        assert!(!filter.has_date_range());

        let filter = WorkflowFilter::new().with_start_time_after(Utc::now());
        assert!(filter.has_date_range());
    }

    #[test]
    fn test_description_with_date_label() {
        let filter = WorkflowFilter::new();
        assert_eq!(
            filter.description_with_date_label(Some("24h ago")),
            "since:24h ago"
        );

        let filter = WorkflowFilter::new().with_status(WorkflowStatus::Failed);
        assert_eq!(
            filter.description_with_date_label(Some("3d ago")),
            "Failed, since:3d ago"
        );

        let filter = WorkflowFilter::new();
        assert_eq!(filter.description_with_date_label(None), "All workflows");
    }

}
