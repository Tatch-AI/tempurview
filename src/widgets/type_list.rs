use crate::app::LoadState;
use crate::domain::{SortDirection, TypeListColumn, TypeStat, WorkflowStatus};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// Status columns displayed in the TypeList table
const STATUS_COLUMNS: [WorkflowStatus; 7] = [
    WorkflowStatus::Running,
    WorkflowStatus::Completed,
    WorkflowStatus::Failed,
    WorkflowStatus::Canceled,
    WorkflowStatus::Terminated,
    WorkflowStatus::TimedOut,
    WorkflowStatus::ContinuedAsNew,
];

/// Renders a table of workflow types with per-status count breakdown
pub struct TypeListWidget<'a> {
    type_stats: &'a LoadState<Vec<TypeStat>>,
    sort: &'a Option<(TypeListColumn, SortDirection)>,
}

impl<'a> TypeListWidget<'a> {
    pub fn new(
        type_stats: &'a LoadState<Vec<TypeStat>>,
        sort: &'a Option<(TypeListColumn, SortDirection)>,
    ) -> Self {
        Self { type_stats, sort }
    }

    fn build_title(&self) -> String {
        if let LoadState::Loaded(stats) = self.type_stats {
            format!("Workflow Types ({})", stats.len())
        } else {
            "Workflow Types".to_string()
        }
    }

    fn sort_indicator_for(&self, col: &TypeListColumn) -> &'static str {
        if let Some((ref sort_col, ref dir)) = self.sort {
            if sort_col == col {
                return dir.indicator();
            }
        }
        ""
    }

    fn build_header(&self) -> Row<'static> {
        let type_ind = self.sort_indicator_for(&TypeListColumn::TypeName);
        let total_ind = self.sort_indicator_for(&TypeListColumn::Total);

        let mut cells = vec![
            Cell::from(format!("Type{}", type_ind)).style(Style::default().bold()),
            Cell::from(format!("Total{}", total_ind)).style(Style::default().bold()),
        ];

        for status in STATUS_COLUMNS.iter() {
            let ind = self.sort_indicator_for(&TypeListColumn::StatusCount(*status));
            cells.push(
                Cell::from(format!("{}{}", status.short_name(), ind))
                    .style(Style::default().bold().fg(status.color())),
            );
        }

        Row::new(cells)
            .style(Style::default().fg(Color::Cyan))
            .bottom_margin(1)
    }

    fn build_widths(&self) -> Vec<Constraint> {
        let mut widths = vec![
            Constraint::Percentage(30), // Type
            Constraint::Length(8),       // Total
        ];
        for _ in &STATUS_COLUMNS {
            widths.push(Constraint::Length(6)); // Status count columns
        }
        widths
    }

    fn stat_to_row(&self, stat: &TypeStat) -> Row<'static> {
        let mut cells = vec![
            Cell::from(truncate_string(&stat.workflow_type, 40)),
            Cell::from(format!("{:>6}", stat.total)).style(Style::default().bold()),
        ];

        for status in &STATUS_COLUMNS {
            let count = stat.get_status_count(*status);
            let cell = if count > 0 {
                Cell::from(format!("{:>4}", count)).style(Style::default().fg(status.color()))
            } else {
                Cell::from(format!("{:>4}", "-"))
                    .style(Style::default().add_modifier(Modifier::DIM))
            };
            cells.push(cell);
        }

        Row::new(cells)
    }
}

impl StatefulWidget for TypeListWidget<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.type_stats {
            LoadState::Loaded(stats) => {
                if stats.is_empty() {
                    let empty = Paragraph::new("No workflow types found")
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

                let rows: Vec<Row> = stats.iter().map(|s| self.stat_to_row(s)).collect();

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
                let loading = Paragraph::new("Loading workflow types...")
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
                let empty = Paragraph::new("Press 'T' to load workflow types")
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

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
