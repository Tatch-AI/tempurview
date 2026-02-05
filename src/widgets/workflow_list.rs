use crate::action::TableColumn;
use crate::app::LoadState;
use crate::domain::{WorkflowFilter, WorkflowSummary};
use chrono::{DateTime, Utc};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};
use std::collections::HashSet;

/// Renders a table of workflows with selection
pub struct WorkflowTableWidget<'a> {
    workflows: &'a LoadState<Vec<WorkflowSummary>>,
    filter: &'a WorkflowFilter,
    visible_columns: &'a HashSet<TableColumn>,
}

impl<'a> WorkflowTableWidget<'a> {
    pub fn new(
        workflows: &'a LoadState<Vec<WorkflowSummary>>,
        filter: &'a WorkflowFilter,
        visible_columns: &'a HashSet<TableColumn>,
    ) -> Self {
        Self {
            workflows,
            filter,
            visible_columns,
        }
    }

    fn build_title(&self) -> String {
        let filter_desc = self.filter.description();
        if let LoadState::Loaded(wfs) = self.workflows {
            format!("Workflows ({}) - {}", wfs.len(), filter_desc)
        } else {
            format!("Workflows - {}", filter_desc)
        }
    }

    fn build_header(&self) -> Row<'static> {
        let mut cells = Vec::new();

        if self.visible_columns.contains(&TableColumn::Status) {
            cells.push(Cell::from("Status").style(Style::default().bold()));
        }
        if self.visible_columns.contains(&TableColumn::Type) {
            cells.push(Cell::from("Type").style(Style::default().bold()));
        }
        if self.visible_columns.contains(&TableColumn::WorkflowId) {
            cells.push(Cell::from("Workflow ID").style(Style::default().bold()));
        }
        if self.visible_columns.contains(&TableColumn::Started) {
            cells.push(Cell::from("Started").style(Style::default().bold()));
        }

        Row::new(cells)
            .style(Style::default().fg(Color::Cyan))
            .bottom_margin(1)
    }

    fn build_widths(&self) -> Vec<Constraint> {
        let mut widths = Vec::new();

        if self.visible_columns.contains(&TableColumn::Status) {
            widths.push(Constraint::Length(8));
        }
        if self.visible_columns.contains(&TableColumn::Type) {
            widths.push(Constraint::Percentage(35));
        }
        if self.visible_columns.contains(&TableColumn::WorkflowId) {
            widths.push(Constraint::Percentage(45));
        }
        if self.visible_columns.contains(&TableColumn::Started) {
            widths.push(Constraint::Length(12));
        }

        widths
    }

    fn workflow_to_row(&self, wf: &WorkflowSummary) -> Row<'static> {
        let mut cells = Vec::new();

        if self.visible_columns.contains(&TableColumn::Status) {
            cells.push(
                Cell::from(format!("{:>6}", wf.status.short_name()))
                    .style(Style::default().fg(wf.status.color())),
            );
        }
        if self.visible_columns.contains(&TableColumn::Type) {
            cells.push(Cell::from(truncate_string(&wf.workflow_type, 35)));
        }
        if self.visible_columns.contains(&TableColumn::WorkflowId) {
            cells.push(
                Cell::from(truncate_string(&wf.workflow_id, 45))
                    .style(Style::default().add_modifier(Modifier::DIM)),
            );
        }
        if self.visible_columns.contains(&TableColumn::Started) {
            cells.push(
                Cell::from(format_duration_since(wf.start_time))
                    .style(Style::default().add_modifier(Modifier::DIM)),
            );
        }

        Row::new(cells)
    }
}

impl StatefulWidget for WorkflowTableWidget<'_> {
    type State = TableState;

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

                let header = self.build_header();
                let widths = self.build_widths();

                let rows: Vec<Row> = workflows
                    .iter()
                    .map(|wf| self.workflow_to_row(wf))
                    .collect();

                let table = Table::new(rows, widths)
                    .header(header)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title()),
                    )
                    .row_highlight_style(
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");

                StatefulWidget::render(table, area, buf, state);
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
                    .style(Style::default().fg(Color::Red))
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

/// Truncate a string to max length with ellipsis
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
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

// Re-export for backwards compatibility during transition
pub type WorkflowListWidget<'a> = WorkflowTableWidget<'a>;

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
    fn test_truncate_string() {
        assert_eq!(truncate_string("short", 10), "short");
        assert_eq!(truncate_string("this is a long string", 10), "this is...");
        assert_eq!(truncate_string("exactly10!", 10), "exactly10!");
    }
}
