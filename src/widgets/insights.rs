use crate::app::LoadState;
use crate::domain::{InsightFinding, InsightsResult};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// Renders a table of insight findings from a health scan
pub struct InsightsWidget<'a> {
    insights: &'a LoadState<InsightsResult>,
    search_query: Option<&'a str>,
}

impl<'a> InsightsWidget<'a> {
    pub fn new(insights: &'a LoadState<InsightsResult>) -> Self {
        Self {
            insights,
            search_query: None,
        }
    }

    pub fn search_query(mut self, query: Option<&'a str>) -> Self {
        self.search_query = query;
        self
    }

    fn build_title(result: &InsightsResult) -> String {
        let duration_ms = result.scan_duration.num_milliseconds();
        let duration_str = if duration_ms < 1000 {
            format!("{}ms", duration_ms)
        } else {
            format!("{:.1}s", duration_ms as f64 / 1000.0)
        };

        format!(
            "Insights ({} finding{} | {} scanned | {} histories | {})",
            result.findings.len(),
            if result.findings.len() == 1 { "" } else { "s" },
            result.workflows_scanned,
            result.histories_fetched,
            duration_str,
        )
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
                let title = Self::build_title(result);

                if result.findings.is_empty() {
                    let empty = Paragraph::new("No issues found — your workflows look healthy!")
                        .style(Style::default().fg(Color::Green))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title),
                        );
                    empty.render(area, buf);
                    return;
                }

                // Apply search filter
                let filtered: Vec<&InsightFinding> =
                    if let Some(query) = self.search_query.filter(|q| !q.is_empty()) {
                        let lower = query.to_lowercase();
                        result
                            .findings
                            .iter()
                            .filter(|f| {
                                format!(
                                    "{} {} {}",
                                    f.severity.label(),
                                    f.category.label(),
                                    f.title
                                )
                                .to_lowercase()
                                .contains(&lower)
                            })
                            .collect()
                    } else {
                        result.findings.iter().collect()
                    };

                if filtered.is_empty() {
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

                let rows: Vec<Row> = filtered
                    .iter()
                    .map(|f| Self::finding_to_row(f))
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

                StatefulWidget::render(table, area, buf, state);
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Scanning workflows for health insights...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Insights"),
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
