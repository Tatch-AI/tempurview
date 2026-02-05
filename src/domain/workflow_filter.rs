use super::WorkflowStatus;
use chrono::{DateTime, Utc};

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
                if let Some(status) = WorkflowStatus::from_str(status_str) {
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

    /// Check if the filter has any conditions
    pub fn is_empty(&self) -> bool {
        self.status.is_none()
            && self.workflow_type.is_none()
            && self.workflow_id_prefix.is_none()
            && self.start_time_after.is_none()
            && self.start_time_before.is_none()
            && self.close_time_after.is_none()
            && self.close_time_before.is_none()
    }

    /// Get a human-readable description of the filter
    pub fn description(&self) -> String {
        if self.is_empty() {
            return "All workflows".to_string();
        }

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

        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(filter.to_query(), Some("WorkflowType='TestWorkflow'".into()));
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
}
