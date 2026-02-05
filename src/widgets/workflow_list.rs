use crate::app::LoadState;
use crate::domain::{WorkflowFilter, WorkflowSummary};
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};

/// Renders a list of workflows with selection
pub struct WorkflowListWidget<'a> {
    workflows: &'a LoadState<Vec<WorkflowSummary>>,
    filter: &'a WorkflowFilter,
}

impl<'a> WorkflowListWidget<'a> {
    pub fn new(workflows: &'a LoadState<Vec<WorkflowSummary>>, filter: &'a WorkflowFilter) -> Self {
        Self { workflows, filter }
    }

    fn build_title(&self) -> String {
        let filter_desc = self.filter.description();
        if let LoadState::Loaded(wfs) = self.workflows {
            format!("Workflows ({}) - {}", wfs.len(), filter_desc)
        } else {
            format!("Workflows - {}", filter_desc)
        }
    }
}

impl StatefulWidget for WorkflowListWidget<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.workflows {
            LoadState::Loaded(workflows) => {
                if workflows.is_empty() {
                    let empty = Paragraph::new("No workflows found")
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(self.build_title()),
                        );
                    empty.render(area, buf);
                    return;
                }

                let items: Vec<ListItem> = workflows
                    .iter()
                    .map(|wf| workflow_to_list_item(wf))
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title()),
                    )
                    .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
                    .highlight_symbol("> ");

                StatefulWidget::render(list, area, buf, state);
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Loading workflows...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title()),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(ratatui::style::Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title()),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'r' to load workflows")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title()),
                    );
                empty.render(area, buf);
            }
        }
    }
}

/// Pure function: convert workflow to list item
fn workflow_to_list_item(wf: &WorkflowSummary) -> ListItem<'static> {
    let status_span = Span::styled(
        format!("{:>6}", wf.status.short_name()),
        Style::default().fg(wf.status.color()),
    );

    let time_span = Span::styled(
        format!("{:>10}", format_duration_since(wf.start_time)),
        Style::default().add_modifier(Modifier::DIM),
    );

    // Truncate workflow type if too long
    let wf_type = if wf.workflow_type.len() > 30 {
        format!("{}...", &wf.workflow_type[..27])
    } else {
        wf.workflow_type.clone()
    };

    // Truncate workflow ID if too long
    let wf_id = if wf.workflow_id.len() > 40 {
        format!("{}...", &wf.workflow_id[..37])
    } else {
        wf.workflow_id.clone()
    };

    let line = Line::from(vec![
        status_span,
        Span::raw(" | "),
        Span::raw(format!("{:<30}", wf_type)),
        Span::raw(" | "),
        Span::styled(wf_id, Style::default().add_modifier(Modifier::DIM)),
        Span::raw(" | "),
        time_span,
    ]);

    ListItem::new(line)
}

/// Pure function: format duration since start
pub fn format_duration_since(start: DateTime<Utc>) -> String {
    let now = Utc::now();
    if start > now {
        return "future".to_string();
    }

    let duration = now - start;

    if duration.num_days() > 0 {
        format!("{}d ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m ago", duration.num_minutes())
    } else {
        "just now".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::WorkflowStatus;
    use chrono::Duration;

    #[test]
    fn test_format_duration_since() {
        let now = Utc::now();

        assert_eq!(format_duration_since(now), "just now");
        assert_eq!(format_duration_since(now - Duration::minutes(5)), "5m ago");
        assert_eq!(format_duration_since(now - Duration::hours(2)), "2h ago");
        assert_eq!(format_duration_since(now - Duration::days(3)), "3d ago");
    }

    #[test]
    fn test_workflow_to_list_item() {
        let wf = WorkflowSummary {
            workflow_id: "test-123".to_string(),
            run_id: "run-456".to_string(),
            workflow_type: "TestWorkflow".to_string(),
            status: WorkflowStatus::Running,
            start_time: Utc::now(),
            close_time: None,
            task_queue: "default".to_string(),
        };

        let item = workflow_to_list_item(&wf);
        // Just verify it doesn't panic
        let _ = item;
    }
}
