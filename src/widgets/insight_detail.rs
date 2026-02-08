use crate::domain::{highlight_search_matches, InsightFinding};
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
    search_query: Option<&'a str>,
    search_current_match: usize,
    search_match_count: usize,
}

impl<'a> InsightDetailWidget<'a> {
    pub fn new(finding: &'a InsightFinding, scroll: u16) -> Self {
        Self {
            finding,
            scroll,
            selected_entity: None,
            search_query: None,
            search_current_match: 0,
            search_match_count: 0,
        }
    }

    pub fn selected_entity(mut self, index: Option<usize>) -> Self {
        self.selected_entity = index;
        self
    }

    pub fn search(
        mut self,
        query: Option<&'a str>,
        current_match: usize,
        match_count: usize,
    ) -> Self {
        self.search_query = query;
        self.search_current_match = current_match;
        self.search_match_count = match_count;
        self
    }

    /// Public static method for App to build lines for search matching
    pub fn build_lines_static(finding: &InsightFinding, entity_index: usize) -> Vec<Line<'static>> {
        let selected = if finding.affected_entities.is_empty() {
            None
        } else {
            Some(entity_index)
        };
        Self::build_lines(finding, selected)
    }

    fn build_lines(finding: &InsightFinding, selected_entity: Option<usize>) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let sev_color = finding.severity.color();

        // Severity + Category header
        lines.push(Line::from(vec![
            Span::styled("Severity: ", Style::default().bold()),
            Span::styled(
                format!(" {} ", finding.severity.label()),
                Style::default()
                    .fg(Color::Black)
                    .bg(sev_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled("Category: ", Style::default().bold()),
            Span::raw(finding.category.label().to_string()),
        ]));

        lines.push(Line::from(""));

        // Finding title — bold with severity color
        let title_style = Style::default()
            .fg(sev_color)
            .add_modifier(Modifier::BOLD);
        let mut title_spans = vec![Span::styled(
            "Finding: ",
            Style::default().fg(Color::Cyan).bold(),
        )];
        title_spans.extend(highlight_trigger_terms(
            &finding.title,
            &finding.trigger_terms,
            title_style,
            Style::default()
                .fg(Color::Black)
                .bg(sev_color)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(title_spans));

        lines.push(Line::from(""));

        // Detail text with trigger term highlighting
        if !finding.detail.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Detail:",
                Style::default().fg(Color::Cyan).bold(),
            )]));

            let highlight_style = Style::default()
                .fg(Color::Black)
                .bg(sev_color)
                .add_modifier(Modifier::BOLD);
            let normal_style = Style::default().fg(Color::White);

            for detail_line in finding.detail.lines() {
                let spans = highlight_trigger_terms(
                    detail_line,
                    &finding.trigger_terms,
                    normal_style,
                    highlight_style,
                );
                lines.push(Line::from(spans));
            }
            lines.push(Line::from(""));
        }

        // Affected entities with severity-colored bullets
        if !finding.affected_entities.is_empty() {
            let count = finding.affected_entities.len();
            let header_suffix = if selected_entity.is_some() {
                " (n/p to navigate, Enter to view)"
            } else {
                ""
            };
            lines.push(Line::from(vec![
                Span::styled(
                    "Affected Entities ",
                    Style::default().fg(Color::Cyan).bold(),
                ),
                Span::styled(
                    format!("({})", count),
                    Style::default().fg(sev_color).bold(),
                ),
                Span::styled(":", Style::default().fg(Color::Cyan).bold()),
                Span::styled(
                    header_suffix.to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            let bullet = Span::styled(
                " \u{25CF} ",
                Style::default().fg(sev_color),
            );
            for (i, entity) in finding.affected_entities.iter().enumerate() {
                let is_selected = selected_entity == Some(i);
                if is_selected {
                    lines.push(Line::from(vec![
                        Span::styled(
                            " \u{25B6} ",
                            Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{}. {}", i + 1, entity),
                            Style::default()
                                .add_modifier(Modifier::REVERSED)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        bullet.clone(),
                        Span::raw(format!("{}. {}", i + 1, entity)),
                    ]));
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

        let mut title = format!(
            " {} | {} | {} ",
            self.finding.severity.label(),
            self.finding.category.label(),
            truncate_title(&self.finding.title, area.width.saturating_sub(30) as usize),
        );

        if let Some(query) = self.search_query {
            if self.search_match_count > 0 {
                title.push_str(&format!(
                    " [{}/{} \"{}\"] ",
                    self.search_current_match + 1,
                    self.search_match_count,
                    query
                ));
            } else {
                title.push_str(&format!(" [no matches for \"{}\"] ", query));
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, sev_style));

        let mut lines = Self::build_lines(self.finding, self.selected_entity);

        // Apply search highlighting if active
        if let Some(query) = self.search_query {
            if !query.is_empty() {
                let (highlighted, _) = highlight_search_matches(&lines, query);
                lines = highlighted;
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        paragraph.render(area, buf);
    }
}

/// Splits `text` into alternating normal/highlighted spans based on trigger term matches.
/// Case-insensitive matching. Non-empty trigger terms only.
fn highlight_trigger_terms(
    text: &str,
    trigger_terms: &[String],
    normal_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    if trigger_terms.is_empty() || text.is_empty() {
        return vec![Span::styled(text.to_string(), normal_style)];
    }

    // Filter out empty terms and very short terms that would highlight too aggressively
    let terms: Vec<&str> = trigger_terms
        .iter()
        .filter(|t| t.len() >= 2)
        .map(|t| t.as_str())
        .collect();

    if terms.is_empty() {
        return vec![Span::styled(text.to_string(), normal_style)];
    }

    let text_lower = text.to_lowercase();
    // Find all match positions: (start, end)
    let mut matches: Vec<(usize, usize)> = Vec::new();
    for term in &terms {
        let term_lower = term.to_lowercase();
        let mut search_start = 0;
        while let Some(pos) = text_lower[search_start..].find(&term_lower) {
            let abs_pos = search_start + pos;
            matches.push((abs_pos, abs_pos + term.len()));
            search_start = abs_pos + 1;
        }
    }

    if matches.is_empty() {
        return vec![Span::styled(text.to_string(), normal_style)];
    }

    // Sort by start position, then by length descending (prefer longer matches)
    matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));

    // Merge overlapping ranges
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for m in matches {
        if let Some(last) = merged.last_mut() {
            if m.0 <= last.1 {
                last.1 = last.1.max(m.1);
                continue;
            }
        }
        merged.push(m);
    }

    // Build spans
    let mut spans = Vec::new();
    let mut cursor = 0;
    for (start, end) in merged {
        if cursor < start {
            spans.push(Span::styled(text[cursor..start].to_string(), normal_style));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            highlight_style,
        ));
        cursor = end;
    }
    if cursor < text.len() {
        spans.push(Span::styled(text[cursor..].to_string(), normal_style));
    }

    spans
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;

    #[test]
    fn test_highlight_no_terms() {
        let spans = highlight_trigger_terms(
            "hello world",
            &[],
            Style::default(),
            Style::default().bold(),
        );
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello world");
    }

    #[test]
    fn test_highlight_single_match() {
        let spans = highlight_trigger_terms(
            "Payment has 75% failure rate",
            &["Payment".to_string(), "75%".to_string()],
            Style::default(),
            Style::default().bold(),
        );
        // Should have: "Payment" highlighted, " has " normal, "75%" highlighted, " failure rate" normal
        assert!(spans.len() >= 4);
        assert_eq!(spans[0].content, "Payment");
        assert_eq!(spans[2].content, "75%");
    }

    #[test]
    fn test_highlight_case_insensitive() {
        let spans = highlight_trigger_terms(
            "The PAYMENT type failed",
            &["payment".to_string()],
            Style::default(),
            Style::default().bold(),
        );
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "The ");
        assert_eq!(spans[1].content, "PAYMENT");
        assert_eq!(spans[2].content, " type failed");
    }

    #[test]
    fn test_highlight_short_terms_filtered() {
        let spans = highlight_trigger_terms(
            "a b c hello",
            &["a".to_string()],
            Style::default(),
            Style::default().bold(),
        );
        // Single char term should be filtered out
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "a b c hello");
    }
}
