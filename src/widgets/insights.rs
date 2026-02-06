use crate::app::LoadState;
use crate::domain::{InsightFinding, InsightsResult};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// Renders a table of insight findings from a health scan
pub struct InsightsWidget<'a> {
    insights: &'a LoadState<InsightsResult>,
    expanded: Option<usize>,
}

impl<'a> InsightsWidget<'a> {
    pub fn new(insights: &'a LoadState<InsightsResult>) -> Self {
        Self {
            insights,
            expanded: None,
        }
    }

    pub fn expanded(mut self, expanded: Option<usize>) -> Self {
        self.expanded = expanded;
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

    fn render_expanded_detail(
        finding: &InsightFinding,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let mut lines: Vec<Line> = Vec::new();

        // Detail text
        if !finding.detail.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    finding.detail.clone(),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }

        // Affected entities
        if !finding.affected_entities.is_empty() {
            let entities_str = if finding.affected_entities.len() <= 5 {
                finding.affected_entities.join(", ")
            } else {
                let shown: Vec<_> = finding.affected_entities.iter().take(5).cloned().collect();
                format!(
                    "{} (+{} more)",
                    shown.join(", "),
                    finding.affected_entities.len() - 5
                )
            };
            lines.push(Line::from(vec![
                Span::styled("  Affected: ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(entities_str),
            ]));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (no additional details)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }

        let paragraph = Paragraph::new(lines).style(Style::default().bg(Color::Black));
        paragraph.render(area, buf);
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

                if let Some(expanded_idx) = self.expanded {
                    // Render with expanded detail
                    let detail_height = if expanded_idx < result.findings.len() {
                        let finding = &result.findings[expanded_idx];
                        let mut lines = 0u16;
                        if !finding.detail.is_empty() {
                            lines += 1;
                        }
                        if !finding.affected_entities.is_empty() {
                            lines += 1;
                        }
                        if lines == 0 {
                            lines = 1;
                        }
                        lines + 1 // padding
                    } else {
                        0
                    };

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(title);
                    let inner = block.inner(area);
                    block.render(area, buf);

                    if inner.height < 3 + detail_height {
                        // Not enough space, render without expansion
                        let rows: Vec<Row> = result
                            .findings
                            .iter()
                            .map(Self::finding_to_row)
                            .collect();

                        let table = Table::new(rows, widths)
                            .header(header)
                            .row_highlight_style(
                                Style::default()
                                    .add_modifier(Modifier::REVERSED)
                                    .add_modifier(Modifier::BOLD),
                            )
                            .highlight_symbol(">> ");

                        StatefulWidget::render(table, inner, buf, state);
                    } else {
                        let table_height = inner.height.saturating_sub(detail_height);
                        let table_area = Rect {
                            x: inner.x,
                            y: inner.y,
                            width: inner.width,
                            height: table_height,
                        };
                        let detail_area = Rect {
                            x: inner.x,
                            y: inner.y + table_height,
                            width: inner.width,
                            height: detail_height,
                        };

                        let rows: Vec<Row> = result
                            .findings
                            .iter()
                            .map(Self::finding_to_row)
                            .collect();

                        let table = Table::new(rows, widths)
                            .header(header)
                            .row_highlight_style(
                                Style::default()
                                    .add_modifier(Modifier::REVERSED)
                                    .add_modifier(Modifier::BOLD),
                            )
                            .highlight_symbol(">> ");

                        StatefulWidget::render(table, table_area, buf, state);

                        if expanded_idx < result.findings.len() {
                            Self::render_expanded_detail(
                                &result.findings[expanded_idx],
                                detail_area,
                                buf,
                            );
                        }
                    }
                } else {
                    let rows: Vec<Row> = result
                        .findings
                        .iter()
                        .map(Self::finding_to_row)
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
