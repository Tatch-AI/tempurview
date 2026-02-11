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
    date_label: Option<&'a str>,
    name_filter: Option<&'a str>,
    filtered_indices: Option<&'a [usize]>,
}

impl<'a> TypeListWidget<'a> {
    pub fn new(
        type_stats: &'a LoadState<Vec<TypeStat>>,
        sort: &'a Option<(TypeListColumn, SortDirection)>,
    ) -> Self {
        Self {
            type_stats,
            sort,
            date_label: None,
            name_filter: None,
            filtered_indices: None,
        }
    }

    pub fn date_label(mut self, label: Option<&'a str>) -> Self {
        self.date_label = label;
        self
    }

    pub fn name_filter(mut self, filter: Option<&'a str>) -> Self {
        self.name_filter = filter;
        self
    }

    pub fn filtered_indices(mut self, indices: Option<&'a [usize]>) -> Self {
        self.filtered_indices = indices;
        self
    }

    fn build_title(&self, display_count: usize) -> String {
        let mut title = format!("Workflow Types ({})", display_count);

        let mut parts = Vec::new();
        if let Some(label) = self.date_label {
            parts.push(format!("since:{}", label));
        }
        if let Some(filter) = self.name_filter {
            if !filter.is_empty() {
                parts.push(format!("name:{}*", filter));
            }
        }
        if !parts.is_empty() {
            title.push_str(" - ");
            title.push_str(&parts.join(", "));
        }
        title
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
                // Apply client-side name filter (from 'f' key — separate from search)
                let name_filtered: Vec<usize> = stats
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        if let Some(filter) = self.name_filter {
                            if !filter.is_empty()
                                && !s
                                    .workflow_type
                                    .to_lowercase()
                                    .contains(&filter.to_lowercase())
                            {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|(i, _)| i)
                    .collect();

                // If search filtered_indices are provided, intersect with name filter
                let effective_indices: Vec<usize> = if let Some(search_indices) =
                    self.filtered_indices
                {
                    // search_indices are indices into the stats array
                    // name_filtered are also indices into stats array
                    // We need the intersection
                    search_indices
                        .iter()
                        .filter(|idx| name_filtered.contains(idx))
                        .copied()
                        .collect()
                } else {
                    name_filtered
                };

                let total_len = effective_indices.len();
                let title = self.build_title(total_len);

                if total_len == 0 {
                    let empty = Paragraph::new("No workflow types found")
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title),
                        );
                    empty.render(area, buf);
                    return;
                }

                let header = self.build_header();
                let widths = self.build_widths();

                // Virtual viewport rendering
                let viewport_height = area.height.saturating_sub(4) as usize;
                let offset = state.offset();
                let selected = state.selected().unwrap_or(0);

                let adjusted_offset = if selected < offset {
                    selected
                } else if viewport_height > 0 && selected >= offset + viewport_height {
                    selected.saturating_sub(viewport_height - 1)
                } else {
                    offset
                };
                *state.offset_mut() = adjusted_offset;

                let end = (adjusted_offset + viewport_height).min(total_len);

                let rows: Vec<Row> = (adjusted_offset..end)
                    .map(|visible_idx| {
                        let data_idx = effective_indices[visible_idx];
                        self.stat_to_row(&stats[data_idx])
                    })
                    .collect();

                let table = Table::new(rows, widths)
                    .header(header)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(title),
                    )
                    .row_highlight_style(
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");

                let mut local_state = TableState::default();
                if selected >= adjusted_offset && selected < end {
                    local_state.select(Some(selected - adjusted_offset));
                }
                StatefulWidget::render(table, area, buf, &mut local_state);
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Loading workflow types...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title(0)),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title(0)),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'T' to load workflow types")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.build_title(0)),
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
