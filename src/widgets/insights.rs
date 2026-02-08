use crate::app::LoadState;
use crate::domain::{InsightFinding, InsightsResult, InsightsScanPhase};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// Renders a table of insight findings from a health scan
pub struct InsightsWidget<'a> {
    insights: &'a LoadState<InsightsResult>,
    filtered_indices: Option<&'a [usize]>,
    progress: Option<&'a InsightsScanPhase>,
    scanning: bool,
    date_label: Option<&'a str>,
}

impl<'a> InsightsWidget<'a> {
    pub fn new(insights: &'a LoadState<InsightsResult>) -> Self {
        Self {
            insights,
            filtered_indices: None,
            progress: None,
            scanning: false,
            date_label: None,
        }
    }

    pub fn filtered_indices(mut self, indices: Option<&'a [usize]>) -> Self {
        self.filtered_indices = indices;
        self
    }

    pub fn progress(mut self, progress: Option<&'a InsightsScanPhase>) -> Self {
        self.progress = progress;
        self
    }

    pub fn scanning(mut self, scanning: bool) -> Self {
        self.scanning = scanning;
        self
    }

    pub fn date_label(mut self, label: Option<&'a str>) -> Self {
        self.date_label = label;
        self
    }

    fn build_title(
        result: &InsightsResult,
        scanning: bool,
        progress: Option<&InsightsScanPhase>,
        date_label: Option<&str>,
    ) -> String {
        let date_suffix = date_label
            .map(|l| format!(" | {}", l))
            .unwrap_or_default();

        if scanning {
            match progress {
                Some(InsightsScanPhase::SamplingHistories { scanned, total }) => {
                    format!(
                        "Insights ({} finding{} | sampling {}/{} histories{})",
                        result.findings.len(),
                        if result.findings.len() == 1 { "" } else { "s" },
                        scanned,
                        total,
                        date_suffix,
                    )
                }
                _ => {
                    format!(
                        "Insights ({} finding{} | scanning...{})",
                        result.findings.len(),
                        if result.findings.len() == 1 { "" } else { "s" },
                        date_suffix,
                    )
                }
            }
        } else {
            let duration_ms = result.scan_duration.num_milliseconds();
            let duration_str = if duration_ms < 1000 {
                format!("{}ms", duration_ms)
            } else {
                format!("{:.1}s", duration_ms as f64 / 1000.0)
            };

            format!(
                "Insights ({} finding{} | {} scanned | {} histories | {}{})",
                result.findings.len(),
                if result.findings.len() == 1 { "" } else { "s" },
                result.workflows_scanned,
                result.histories_fetched,
                duration_str,
                date_suffix,
            )
        }
    }

    fn finding_to_row(finding: &InsightFinding) -> Row<'static> {
        let sev_style = Style::default()
            .fg(finding.severity.color())
            .add_modifier(Modifier::BOLD);

        Row::new(vec![
            Cell::from(finding.severity.label().to_string()).style(sev_style),
            Cell::from(finding.category.label().to_string()),
            Cell::from(finding.title.clone()),
        ])
    }
}

impl StatefulWidget for InsightsWidget<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.insights {
            LoadState::Loaded(ref result) => {
                let title = Self::build_title(result, self.scanning, self.progress, self.date_label);

                if result.findings.is_empty() {
                    let (msg, style) = if self.scanning {
                        (
                            "No findings yet — sampling histories for deeper analysis...".to_string(),
                            Style::default().add_modifier(Modifier::DIM),
                        )
                    } else {
                        (
                            "No issues found — your workflows look healthy!".to_string(),
                            Style::default().fg(Color::Green),
                        )
                    };
                    let empty = Paragraph::new(msg)
                        .style(style)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title),
                        );
                    empty.render(area, buf);
                    return;
                }

                let total_len = match self.filtered_indices {
                    Some(indices) => indices.len(),
                    None => result.findings.len(),
                };

                if total_len == 0 {
                    let empty = Paragraph::new("No matching findings")
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title),
                        );
                    empty.render(area, buf);
                    return;
                }

                let header = Row::new(vec![
                    Cell::from("SEV").style(Style::default().bold()),
                    Cell::from("CATEGORY").style(Style::default().bold()),
                    Cell::from("FINDING").style(Style::default().bold()),
                ])
                .style(Style::default().fg(Color::Cyan))
                .bottom_margin(0);

                let widths = [
                    ratatui::layout::Constraint::Length(6),
                    ratatui::layout::Constraint::Length(18),
                    ratatui::layout::Constraint::Fill(1),
                ];

                // Virtual viewport rendering
                let viewport_height = area.height.saturating_sub(3) as usize;
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
                        let data_idx = match self.filtered_indices {
                            Some(indices) => indices[visible_idx],
                            None => visible_idx,
                        };
                        Self::finding_to_row(&result.findings[data_idx])
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
                    .highlight_symbol(">> ");

                let mut local_state = TableState::default();
                if selected >= adjusted_offset && selected < end {
                    local_state.select(Some(selected - adjusted_offset));
                }
                StatefulWidget::render(table, area, buf, &mut local_state);
            }
            LoadState::Loading => {
                let date_hint = if self.date_label.is_some() {
                    String::new()
                } else {
                    "\n\nTip: press 'd' to set a date range and speed up the scan".to_string()
                };
                let msg = match self.progress {
                    Some(InsightsScanPhase::FetchingWorkflows { fetched }) => {
                        format!("Fetching workflows... {}{}", fetched, date_hint)
                    }
                    Some(InsightsScanPhase::SamplingHistories { scanned, total }) => {
                        format!("Sampling histories... {}/{}", scanned, total)
                    }
                    None => format!("Scanning workflows for health insights...{}", date_hint),
                };
                let title = self
                    .date_label
                    .map(|l| format!("Insights ({})", l))
                    .unwrap_or_else(|| "Insights".to_string());
                let loading = Paragraph::new(msg)
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(title),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Insights"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty =
                    Paragraph::new("Press 'I' from Workflow List to run a health scan")
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Insights"),
                        );
                empty.render(area, buf);
            }
        }
    }
}
