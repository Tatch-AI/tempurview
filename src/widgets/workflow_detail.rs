use crate::app::LoadState;
use crate::domain::WorkflowDetail;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Renders detailed view of a single workflow
pub struct WorkflowDetailWidget<'a> {
    detail: &'a LoadState<WorkflowDetail>,
}

impl<'a> WorkflowDetailWidget<'a> {
    pub fn new(detail: &'a LoadState<WorkflowDetail>) -> Self {
        Self { detail }
    }
}

impl Widget for WorkflowDetailWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.detail {
            LoadState::Loaded(detail) => {
                let layout = Layout::vertical([
                    Constraint::Length(8), // Metadata
                    Constraint::Length(8), // Input
                    Constraint::Length(8), // Output
                    Constraint::Fill(1),   // Failure (if any)
                ])
                .split(area);

                // Render metadata section
                render_metadata_section(detail, layout[0], buf);

                // Render input section
                render_json_section("Input", &detail.input, layout[1], buf);

                // Render output section
                render_json_section("Output", &detail.output, layout[2], buf);

                // Render failure section if present
                if let Some(ref failure) = detail.failure {
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(Span::styled("Failure", Style::default().fg(Color::Red)))
                        .border_style(Style::default().fg(Color::Red));

                    let inner = block.inner(layout[3]);
                    block.render(layout[3], buf);

                    let failure_text = format!(
                        "Type: {}\nMessage: {}\n{}",
                        failure.failure_type,
                        failure.message,
                        failure
                            .stack_trace
                            .as_deref()
                            .map(|s| format!("\nStack Trace:\n{}", s))
                            .unwrap_or_default()
                    );

                    let paragraph = Paragraph::new(failure_text)
                        .style(Style::default().fg(Color::Red))
                        .wrap(Wrap { trim: true });
                    paragraph.render(inner, buf);
                }
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Loading workflow details...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Workflow Details"),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Workflow Details"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("No workflow selected")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Workflow Details"),
                    );
                empty.render(area, buf);
            }
        }
    }
}

fn render_metadata_section(detail: &WorkflowDetail, area: Rect, buf: &mut Buffer) {
    let metadata = format_metadata(detail);

    let block = Block::default().borders(Borders::ALL).title(format!(
        "Workflow: {} ({})",
        detail.summary.workflow_type, detail.summary.status
    ));

    let inner = block.inner(area);
    block.render(area, buf);

    let lines: Vec<Line> = metadata
        .iter()
        .map(|(key, value)| {
            Line::from(vec![
                Span::styled(
                    format!("{}: ", key),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(value.clone()),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    paragraph.render(inner, buf);
}

fn render_json_section(
    title: &str,
    value: &Option<serde_json::Value>,
    area: Rect,
    buf: &mut Buffer,
) {
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    block.render(area, buf);

    let content = match value {
        Some(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| "Invalid JSON".to_string()),
        None => "(none)".to_string(),
    };

    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: true });
    paragraph.render(inner, buf);
}

/// Pure function: format workflow metadata as key-value pairs
pub fn format_metadata(detail: &WorkflowDetail) -> Vec<(String, String)> {
    vec![
        (
            "Workflow ID".to_string(),
            detail.summary.workflow_id.clone(),
        ),
        ("Run ID".to_string(), detail.summary.run_id.clone()),
        ("Type".to_string(), detail.summary.workflow_type.clone()),
        ("Status".to_string(), format!("{}", detail.summary.status)),
        ("Task Queue".to_string(), detail.summary.task_queue.clone()),
        (
            "Started".to_string(),
            detail.summary.start_time.to_rfc3339(),
        ),
        (
            "Closed".to_string(),
            detail
                .summary
                .close_time
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "(running)".to_string()),
        ),
        (
            "History Events".to_string(),
            detail.history_length.to_string(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{WorkflowStatus, WorkflowSummary};
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_detail() -> WorkflowDetail {
        WorkflowDetail {
            summary: WorkflowSummary {
                workflow_id: "wf-123".to_string(),
                run_id: "run-456".to_string(),
                workflow_type: "TestWorkflow".to_string(),
                status: WorkflowStatus::Completed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: "default".to_string(),
            },
            input: Some(serde_json::json!({"key": "value"})),
            output: Some(serde_json::json!({"result": 42})),
            failure: None,
            history_length: 10,
            memo: HashMap::new(),
            search_attributes: HashMap::new(),
        }
    }

    #[test]
    fn test_format_metadata() {
        let detail = make_detail();
        let metadata = format_metadata(&detail);

        assert!(metadata
            .iter()
            .any(|(k, v)| k == "Workflow ID" && v == "wf-123"));
        assert!(metadata
            .iter()
            .any(|(k, v)| k == "Run ID" && v == "run-456"));
        assert!(metadata
            .iter()
            .any(|(k, v)| k == "Type" && v == "TestWorkflow"));
        assert!(metadata
            .iter()
            .any(|(k, v)| k == "Status" && v == "Completed"));
    }
}
