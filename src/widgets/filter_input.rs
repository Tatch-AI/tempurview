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
}

impl<'a> FilterInput<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            is_active: false,
        }
    }

    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
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
        } else if self.input.is_empty() {
            "Press / to filter...".to_string()
        } else {
            self.input.to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(if self.is_active { "Filter (Enter to apply, Esc to cancel)" } else { "Filter" });

        let paragraph = Paragraph::new(display_text)
            .style(style)
            .block(block);

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
