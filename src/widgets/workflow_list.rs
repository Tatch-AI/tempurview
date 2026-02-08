use crate::action::TableColumn;
use crate::app::LoadState;
use crate::domain::{SortDirection, WorkflowFilter, WorkflowSummary};
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
    sort: &'a Option<(TableColumn, SortDirection)>,
    date_label: Option<&'a str>,
    filtered_indices: Option<&'a [usize]>,
    loading: bool,
    total_count: Option<u64>,
}

impl<'a> WorkflowTableWidget<'a> {
    pub fn new(
        workflows: &'a LoadState<Vec<WorkflowSummary>>,
        filter: &'a WorkflowFilter,
        visible_columns: &'a HashSet<TableColumn>,
        sort: &'a Option<(TableColumn, SortDirection)>,
    ) -> Self {
        Self {
            workflows,
            filter,
            visible_columns,
            sort,
            date_label: None,
            filtered_indices: None,
            loading: false,
            total_count: None,
        }
    }

    pub fn date_label(mut self, label: Option<&'a str>) -> Self {
        self.date_label = label;
        self
    }

    pub fn filtered_indices(mut self, indices: Option<&'a [usize]>) -> Self {
        self.filtered_indices = indices;
        self
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn total_count(mut self, count: Option<u64>) -> Self {
        self.total_count = count;
        self
    }

    fn build_title(&self, total_len: usize) -> String {
        let filter_desc = self.filter.description_with_date_label(self.date_label);
        if self.loading {
            match self.total_count {
                Some(count) => format!(
                    "Workflows ({} / {} loading...) - {}",
                    total_len, count, filter_desc
                ),
                None => format!("Workflows ({} loading...) - {}", total_len, filter_desc),
            }
        } else {
            format!("Workflows ({}) - {}", total_len, filter_desc)
        }
    }

    fn sort_indicator_for(&self, col: TableColumn) -> &'static str {
        if let Some((ref sort_col, ref dir)) = self.sort {
            if *sort_col == col {
                return dir.indicator();
            }
        }
        ""
    }

    fn build_header(&self) -> Row<'static> {
        let mut cells = Vec::new();

        cells.push(Cell::from("#").style(Style::default().bold()));

        if self.visible_columns.contains(&TableColumn::Status) {
            let ind = self.sort_indicator_for(TableColumn::Status);
            cells.push(Cell::from(format!("Status{}", ind)).style(Style::default().bold()));
        }
        if self.visible_columns.contains(&TableColumn::Type) {
            let ind = self.sort_indicator_for(TableColumn::Type);
            cells.push(Cell::from(format!("Type{}", ind)).style(Style::default().bold()));
        }
        if self.visible_columns.contains(&TableColumn::WorkflowId) {
            let ind = self.sort_indicator_for(TableColumn::WorkflowId);
            cells.push(Cell::from(format!("Workflow ID{}", ind)).style(Style::default().bold()));
        }
        if self.visible_columns.contains(&TableColumn::Started) {
            let ind = self.sort_indicator_for(TableColumn::Started);
            cells.push(Cell::from(format!("Started{}", ind)).style(Style::default().bold()));
        }

        Row::new(cells)
            .style(Style::default().fg(Color::Cyan))
            .bottom_margin(1)
    }

    fn build_widths(&self) -> Vec<Constraint> {
        let mut widths = Vec::new();

        widths.push(Constraint::Length(6));

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

    fn workflow_to_row(&self, idx: usize, wf: &WorkflowSummary) -> Row<'static> {
        let mut cells = Vec::new();

        cells.push(
            Cell::from(format!("{}", idx + 1))
                .style(Style::default().add_modifier(Modifier::DIM)),
        );

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
                let total_len = match self.filtered_indices {
                    Some(indices) => indices.len(),
                    None => workflows.len(),
                };

                if total_len == 0 {
                    let msg = if self.filtered_indices.is_some() {
                        "No matching workflows"
                    } else {
                        "No workflows found"
                    };
                    let empty = Paragraph::new(msg)
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(self.build_title(total_len)),
                        );
                    empty.render(area, buf);
                    return;
                }

                let header = self.build_header();
                let widths = self.build_widths();

                // Compute visible window for virtual viewport rendering
                // borders(2) + header(1) + header bottom_margin(1) = 4
                let viewport_height = area.height.saturating_sub(4) as usize;
                let offset = state.offset();
                let selected = state.selected().unwrap_or(0);

                // Adjust offset to keep selected visible
                let adjusted_offset = if selected < offset {
                    selected
                } else if viewport_height > 0 && selected >= offset + viewport_height {
                    selected.saturating_sub(viewport_height - 1)
                } else {
                    offset
                };
                *state.offset_mut() = adjusted_offset;

                let end = (adjusted_offset + viewport_height).min(total_len);

                // Build ONLY visible rows
                let rows: Vec<Row> = (adjusted_offset..end)
                    .map(|visible_idx| {
                        let data_idx = match self.filtered_indices {
                            Some(indices) => indices[visible_idx],
                            None => visible_idx,
                        };
                        self.workflow_to_row(visible_idx, &workflows[data_idx])
                    })
                    .collect();

                let table = Table::new(rows, widths)
                    .header(header)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title(total_len)),
                    )
                    .row_highlight_style(
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");

                // Render with local state (selected relative to window)
                let mut local_state = TableState::default();
                if selected >= adjusted_offset && selected < end {
                    local_state.select(Some(selected - adjusted_offset));
                }
                StatefulWidget::render(table, area, buf, &mut local_state);
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Loading workflows...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Workflows"),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Workflows"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'r' to load workflows")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Workflows"),
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
