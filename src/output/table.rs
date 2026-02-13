use crate::domain::{
    ActivityExecution, HistoryEvent, InsightFinding, InsightsResult, StatusCounts, WorkflowDetail,
    WorkflowStatus, WorkflowSummary,
};
use crate::domain::activity::format_duration;
use comfy_table::{presets::UTF8_FULL_CONDENSED, Attribute, Cell, Color, ContentArrangement, Table};

pub trait TableDisplay {
    fn to_table(&self) -> Table;
}

// --- WorkflowSummary list ---

impl TableDisplay for Vec<WorkflowSummary> {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec!["STATUS", "TYPE", "WORKFLOW ID", "STARTED", "TASK QUEUE"]);
        for wf in self {
            table.add_row(vec![
                status_cell(wf.status),
                Cell::new(&*wf.workflow_type),
                Cell::new(&wf.workflow_id),
                Cell::new(wf.start_time.format("%Y-%m-%d %H:%M:%S").to_string()),
                Cell::new(&*wf.task_queue),
            ]);
        }
        table
    }
}

// --- WorkflowDetail ---

impl TableDisplay for WorkflowDetail {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec!["FIELD", "VALUE"]);
        table.add_row(vec!["Workflow ID", &self.summary.workflow_id]);
        table.add_row(vec!["Run ID", &self.summary.run_id]);
        table.add_row(vec!["Type", &*self.summary.workflow_type]);
        table.add_row(vec![
            Cell::new("Status"),
            status_cell(self.summary.status),
        ]);
        table.add_row(vec![
            "Started",
            &self.summary.start_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ]);
        if let Some(ref ct) = self.summary.close_time {
            table.add_row(vec![
                "Closed",
                &ct.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            ]);
        }
        table.add_row(vec!["Task Queue", &*self.summary.task_queue]);
        table.add_row(vec!["History Length", &self.history_length.to_string()]);
        if let Some(ref input) = self.input {
            table.add_row(vec!["Input", &compact_json(input)]);
        }
        if let Some(ref output) = self.output {
            table.add_row(vec!["Output", &compact_json(output)]);
        }
        if let Some(ref failure) = self.failure {
            table.add_row(vec!["Failure", &failure.message]);
            table.add_row(vec!["Failure Type", &failure.failure_type]);
        }
        table
    }
}

// --- ActivityExecution list ---

impl TableDisplay for Vec<ActivityExecution> {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec![
            "ID", "TYPE", "STATUS", "ATTEMPT", "QUEUE WAIT", "EXEC TIME",
        ]);
        for a in self {
            table.add_row(vec![
                Cell::new(&a.activity_id),
                Cell::new(&a.activity_type),
                Cell::new(a.status.short_name()),
                Cell::new(a.attempt),
                Cell::new(
                    a.queue_wait
                        .as_ref()
                        .map(format_duration)
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::new(
                    a.execution_time
                        .as_ref()
                        .map(format_duration)
                        .unwrap_or_else(|| "-".into()),
                ),
            ]);
        }
        table
    }
}

// --- HistoryEvent list ---

impl TableDisplay for Vec<HistoryEvent> {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec!["EVENT ID", "TYPE", "TIMESTAMP"]);
        for e in self {
            table.add_row(vec![
                Cell::new(e.event_id),
                Cell::new(&e.event_type),
                Cell::new(e.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()),
            ]);
        }
        table
    }
}

// --- InsightsResult ---

impl TableDisplay for InsightsResult {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec!["SEVERITY", "CATEGORY", "TYPE", "TITLE", "LAST SEEN", "AFFECTED"]);
        for f in &self.findings {
            let wf_type = f.workflow_type.as_deref().unwrap_or("-");
            let last_seen = f
                .last_observed
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "-".into());
            table.add_row(vec![
                severity_cell(f),
                Cell::new(f.category.label()),
                Cell::new(wf_type),
                Cell::new(&f.title),
                Cell::new(last_seen),
                Cell::new(f.affected_entities.len()),
            ]);
        }
        // Summary footer (stderr so it doesn't interfere with stdout capture)
        let dur = format_duration(&self.scan_duration);
        eprintln!(
            "Scanned {} workflows, fetched {} histories in {}. {} findings.",
            self.workflows_scanned,
            self.histories_fetched,
            dur,
            self.findings.len()
        );
        table
    }
}

// --- StatusCounts ---

impl TableDisplay for StatusCounts {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec!["STATUS", "COUNT"]);
        for status in WorkflowStatus::all() {
            let count = self.get(*status);
            if count > 0 {
                table.add_row(vec![
                    status_cell(*status),
                    Cell::new(count),
                ]);
            }
        }
        table.add_row(vec![
            Cell::new("Total").add_attribute(Attribute::Bold),
            Cell::new(self.total()).add_attribute(Attribute::Bold),
        ]);
        table
    }
}

// --- Config display ---

impl TableDisplay for crate::config::Config {
    fn to_table(&self) -> Table {
        let mut table = new_table();
        table.set_header(vec!["SETTING", "VALUE"]);
        table.add_row(vec![
            "Active Profile",
            self.active_profile.as_deref().unwrap_or("(none)"),
        ]);
        table.add_row(vec!["Temporal Address", &self.temporal_address]);
        table.add_row(vec!["Temporal Namespace", &self.temporal_namespace]);
        table.add_row(vec![
            "API Key",
            if self.temporal_api_key.is_some() {
                "(set, hidden)"
            } else {
                "(not set)"
            },
        ]);
        table.add_row(vec!["Default Limit", &self.default_limit.to_string()]);
        table.add_row(vec!["Mock Mode", &self.use_mock.to_string()]);
        table.add_row(vec![
            "Mock Workflow Count",
            &self.mock_workflow_count.to_string(),
        ]);
        if !self.insights.allowlist.is_empty() {
            table.add_row(vec![
                "Insights Allowlist",
                &self.insights.allowlist.join(", "),
            ]);
        }
        table
    }
}

// --- Helpers ---

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn status_cell(status: WorkflowStatus) -> Cell {
    let color = match status {
        WorkflowStatus::Running => Color::Blue,
        WorkflowStatus::Completed => Color::Green,
        WorkflowStatus::Failed => Color::Red,
        WorkflowStatus::Canceled => Color::Yellow,
        WorkflowStatus::Terminated => Color::Magenta,
        WorkflowStatus::TimedOut => Color::Red,
        WorkflowStatus::ContinuedAsNew => Color::Cyan,
    };
    Cell::new(status.as_query_value()).fg(color)
}

fn severity_cell(finding: &InsightFinding) -> Cell {
    let color = match finding.severity {
        crate::domain::InsightSeverity::Info => Color::Cyan,
        crate::domain::InsightSeverity::Warning => Color::Yellow,
        crate::domain::InsightSeverity::Critical => Color::Red,
    };
    Cell::new(finding.severity.label()).fg(color)
}

fn compact_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "?".into())
}
