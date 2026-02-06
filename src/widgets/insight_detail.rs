use crate::domain::InsightFinding;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Full-screen scrollable detail view for a single insight finding
pub struct InsightDetailWidget<'a> {
    finding: &'a InsightFinding,
    scroll: u16,
    selected_entity: Option<usize>,
}

impl<'a> InsightDetailWidget<'a> {
    pub fn new(finding: &'a InsightFinding, scroll: u16) -> Self {
        Self {
            finding,
            scroll,
            selected_entity: None,
        }
    }

    pub fn selected_entity(mut self, index: Option<usize>) -> Self {
        self.selected_entity = index;
        self
    }

    fn build_lines(finding: &InsightFinding, selected_entity: Option<usize>) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Severity + Category header
        lines.push(Line::from(vec![
            Span::styled("Severity: ", Style::default().bold()),
            Span::styled(
                finding.severity.label().to_string(),
                Style::default()
                    .fg(finding.severity.color())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("Category: ", Style::default().bold()),
            Span::raw(finding.category.label().to_string()),
        ]));

        lines.push(Line::from(""));

        // Finding title
        lines.push(Line::from(vec![
            Span::styled("Finding: ", Style::default().fg(Color::Cyan).bold()),
            Span::raw(finding.title.clone()),
        ]));

        lines.push(Line::from(""));

        // Detail text
        if !finding.detail.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Detail:",
                Style::default().fg(Color::Cyan).bold(),
            )]));
            // Split detail into wrapped lines
            for detail_line in finding.detail.lines() {
                lines.push(Line::from(Span::styled(
                    detail_line.to_string(),
                    Style::default().fg(Color::Gray),
                )));
            }
            lines.push(Line::from(""));
        }

        // Affected entities
        if !finding.affected_entities.is_empty() {
            let header_suffix = if selected_entity.is_some() {
                " (n/p to navigate, Enter to view)"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("Affected Entities ({}):", finding.affected_entities.len()),
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Span::styled(
                    header_suffix.to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            for (i, entity) in finding.affected_entities.iter().enumerate() {
                let is_selected = selected_entity == Some(i);
                if is_selected {
                    lines.push(Line::from(Span::styled(
                        format!("  >> {}. {}", i + 1, entity),
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    )));
                } else {
                    lines.push(Line::from(format!("  {}. {}", i + 1, entity)));
                }
            }
        }

        lines
    }
}

impl Widget for InsightDetailWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let sev_style = Style::default()
            .fg(self.finding.severity.color())
            .add_modifier(Modifier::BOLD);

        let title = format!(
            " {} | {} | {} ",
            self.finding.severity.label(),
            self.finding.category.label(),
            truncate_title(&self.finding.title, area.width.saturating_sub(30) as usize),
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, sev_style));

        let lines = Self::build_lines(self.finding, self.selected_entity);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        paragraph.render(area, buf);
    }
}

fn truncate_title(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
