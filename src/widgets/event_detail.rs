use crate::domain::{highlight_search_matches, json_to_lines, HistoryEvent};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Full-screen scrollable detail view for a single HistoryEvent with JSON syntax
/// highlighting and optional search highlighting.
pub struct EventDetailWidget<'a> {
    event: &'a HistoryEvent,
    scroll: u16,
    search_query: Option<&'a str>,
    search_current_match: usize,
    search_match_count: usize,
}

impl<'a> EventDetailWidget<'a> {
    pub fn new(event: &'a HistoryEvent, scroll: u16) -> Self {
        Self {
            event,
            scroll,
            search_query: None,
            search_current_match: 0,
            search_match_count: 0,
        }
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

    fn event_color(event_type: &str) -> Color {
        if event_type.contains("Activity") {
            Color::Cyan
        } else if event_type.contains("ChildWorkflow") {
            Color::Magenta
        } else if event_type.contains("Timer") || event_type.contains("Signal") {
            Color::Yellow
        } else if event_type.contains("WorkflowExecution") {
            Color::Green
        } else if event_type.contains("WorkflowTask") {
            Color::DarkGray
        } else {
            Color::White
        }
    }

    fn build_lines(event: &HistoryEvent) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        let color = Self::event_color(&event.event_type);

        // Header: Event ID + Type
        lines.push(Line::from(vec![
            Span::styled("Event: ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                format!("#{}", event.event_id),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw("  "),
            Span::styled(event.event_type.clone(), Style::default().fg(color).bold()),
        ]));

        // Timestamp
        lines.push(Line::from(vec![
            Span::styled("Time:  ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                event.timestamp.to_rfc3339(),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Blank line + Details label
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Details:",
            Style::default().fg(Color::Cyan).bold(),
        )));

        // JSON syntax-highlighted details
        let json_lines = json_to_lines(&event.details);
        lines.extend(json_lines);

        lines
    }
}

impl Widget for EventDetailWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let color = Self::event_color(&self.event.event_type);

        let mut title = format!(" #{} {} ", self.event.event_id, self.event.event_type);
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
            .title(Span::styled(
                title,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));

        let mut lines = Self::build_lines(self.event);

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
