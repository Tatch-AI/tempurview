use super::{WorkflowStatus, WorkflowSummary};
use std::collections::HashMap;

/// Aggregated workflow counts by status
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusCounts {
    counts: HashMap<WorkflowStatus, u64>,
}

impl StatusCounts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, status: WorkflowStatus, count: u64) {
        self.counts.insert(status, count);
    }

    pub fn get(&self, status: WorkflowStatus) -> u64 {
        self.counts.get(&status).copied().unwrap_or(0)
    }

    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Returns (status, count) pairs sorted by count descending
    pub fn sorted_by_count(&self) -> Vec<(WorkflowStatus, u64)> {
        let mut items: Vec<_> = self.counts.iter().map(|(k, v)| (*k, *v)).collect();
        items.sort_by(|a, b| b.1.cmp(&a.1));
        items
    }

    /// Returns statuses with non-zero counts
    pub fn non_zero(&self) -> Vec<(WorkflowStatus, u64)> {
        self.counts
            .iter()
            .filter(|(_, v)| **v > 0)
            .map(|(k, v)| (*k, *v))
            .collect()
    }

    /// Build counts from a list of workflow summaries
    pub fn from_workflows(workflows: &[WorkflowSummary]) -> Self {
        let mut counts = Self::new();
        for wf in workflows {
            let current = counts.get(wf.status);
            counts.set(wf.status, current + 1);
        }
        counts
    }
}

/// Workflow type distribution
#[derive(Debug, Clone, Default)]
pub struct TypeDistribution {
    counts: HashMap<String, u64>,
}

impl TypeDistribution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_workflows(workflows: &[WorkflowSummary]) -> Self {
        let mut dist = Self::new();
        for wf in workflows {
            let count = dist.counts.get(&wf.workflow_type).copied().unwrap_or(0);
            dist.counts.insert(wf.workflow_type.clone(), count + 1);
        }
        dist
    }

    pub fn top_n(&self, n: usize) -> Vec<(&str, u64)> {
        let mut items: Vec<_> = self.counts.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        items.sort_by(|a, b| b.1.cmp(&a.1));
        items.truncate(n);
        items
    }

    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_workflow(status: WorkflowStatus, workflow_type: &str) -> WorkflowSummary {
        WorkflowSummary {
            workflow_id: "test".to_string(),
            run_id: "run".to_string(),
            workflow_type: workflow_type.to_string(),
            status,
            start_time: Utc::now(),
            close_time: None,
            task_queue: "default".to_string(),
        }
    }

    #[test]
    fn test_status_counts_basic() {
        let mut counts = StatusCounts::new();
        assert_eq!(counts.get(WorkflowStatus::Running), 0);

        counts.set(WorkflowStatus::Running, 42);
        assert_eq!(counts.get(WorkflowStatus::Running), 42);

        counts.set(WorkflowStatus::Failed, 5);
        assert_eq!(counts.total(), 47);
    }

    #[test]
    fn test_sorted_by_count() {
        let mut counts = StatusCounts::new();
        counts.set(WorkflowStatus::Running, 10);
        counts.set(WorkflowStatus::Failed, 50);
        counts.set(WorkflowStatus::Completed, 30);

        let sorted = counts.sorted_by_count();
        assert_eq!(sorted[0], (WorkflowStatus::Failed, 50));
        assert_eq!(sorted[1], (WorkflowStatus::Completed, 30));
        assert_eq!(sorted[2], (WorkflowStatus::Running, 10));
    }

    #[test]
    fn test_non_zero() {
        let mut counts = StatusCounts::new();
        counts.set(WorkflowStatus::Running, 10);
        counts.set(WorkflowStatus::Failed, 0);
        counts.set(WorkflowStatus::Completed, 5);

        let non_zero = counts.non_zero();
        assert_eq!(non_zero.len(), 2);
        assert!(non_zero.iter().any(|(s, _)| *s == WorkflowStatus::Running));
        assert!(non_zero.iter().any(|(s, _)| *s == WorkflowStatus::Completed));
    }

    #[test]
    fn test_from_workflows() {
        let workflows = vec![
            make_workflow(WorkflowStatus::Running, "Type1"),
            make_workflow(WorkflowStatus::Running, "Type2"),
            make_workflow(WorkflowStatus::Failed, "Type1"),
        ];

        let counts = StatusCounts::from_workflows(&workflows);
        assert_eq!(counts.get(WorkflowStatus::Running), 2);
        assert_eq!(counts.get(WorkflowStatus::Failed), 1);
        assert_eq!(counts.get(WorkflowStatus::Completed), 0);
    }

    #[test]
    fn test_type_distribution() {
        let workflows = vec![
            make_workflow(WorkflowStatus::Running, "TypeA"),
            make_workflow(WorkflowStatus::Running, "TypeA"),
            make_workflow(WorkflowStatus::Running, "TypeB"),
            make_workflow(WorkflowStatus::Running, "TypeC"),
            make_workflow(WorkflowStatus::Running, "TypeC"),
            make_workflow(WorkflowStatus::Running, "TypeC"),
        ];

        let dist = TypeDistribution::from_workflows(&workflows);
        let top2 = dist.top_n(2);

        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0], ("TypeC", 3));
        assert_eq!(top2[1], ("TypeA", 2));
    }
}
