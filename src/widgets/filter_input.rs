use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Renders the filter input field
pub struct FilterInput<'a> {
    input: &'a str,
    is_active: bool,
    is_date_mode: bool,
    is_search_mode: bool,
    date_label: Option<&'a str>,
    search_status: Option<&'a str>,
    match_count: Option<usize>,
}

impl<'a> FilterInput<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            is_active: false,
            is_date_mode: false,
            is_search_mode: false,
            date_label: None,
            search_status: None,
            match_count: None,
        }
    }

    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }

    pub fn date_mode(mut self, is_date_mode: bool) -> Self {
        self.is_date_mode = is_date_mode;
        self
    }

    pub fn search_mode(mut self, is_search_mode: bool) -> Self {
        self.is_search_mode = is_search_mode;
        self
    }

    pub fn date_label(mut self, label: Option<&'a str>) -> Self {
        self.date_label = label;
        self
    }

    pub fn search_status(mut self, status: Option<&'a str>) -> Self {
        self.search_status = status;
        self
    }

    pub fn match_count(mut self, count: Option<usize>) -> Self {
        self.match_count = count;
        self
    }
}

impl Widget for FilterInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = if self.is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let border_style = if self.is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Show cursor at the end when active
        let display_text = if self.is_active {
            format!("{}_", self.input)
        } else if let Some(status) = self.search_status {
            status.to_string()
        } else if self.input.is_empty() {
            if self.date_label.is_some() {
                "Press / to search, f to filter, d for date range...".to_string()
            } else {
                "Press / to search...".to_string()
            }
        } else {
            self.input.to_string()
        };

        let title_owned: String;
        let title: &str = if self.is_date_mode {
            "Date range (e.g. 2h, 3d, 1w, 2024-01-15) — Enter to apply, Esc to cancel"
        } else if self.is_search_mode {
            if let Some(count) = self.match_count {
                title_owned = format!("Search ({} matches) — Enter to close, Esc to cancel", count);
                &title_owned
            } else {
                "Search (Enter to apply, Esc to cancel)"
            }
        } else if self.is_active {
            "Filter (Enter to apply, Esc to cancel)"
        } else {
            "Filter"
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let paragraph = Paragraph::new(display_text).style(style).block(block);

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_filter_input_renders() {
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let widget = FilterInput::new("test query").active(true);
                frame.render_widget(widget, frame.area());
            })
            .unwrap();

        // Just verify it doesn't panic
    }
}
