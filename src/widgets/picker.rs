use crate::picker::PickerState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Renders a centered picker overlay modal
pub struct PickerWidget<'a> {
    state: &'a PickerState,
}

impl<'a> PickerWidget<'a> {
    pub fn new(state: &'a PickerState) -> Self {
        Self { state }
    }
}

impl Widget for PickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items = &self.state.items;
        if items.is_empty() {
            return;
        }

        // Calculate popup size
        let max_label_len = items
            .iter()
            .map(|item| item.label.len() + 6) // "[k]  label"
            .max()
            .unwrap_or(10);
        let title_len = self.state.title.len() + 4; // borders + padding
        let content_width = max_label_len.max(title_len) as u16 + 4; // padding
        let content_height = items.len() as u16 + 2; // borders

        let popup_width = content_width.min(area.width.saturating_sub(4));
        let popup_height = content_height.min(area.height.saturating_sub(4));

        // Center the popup
        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };

        // Clear background
        Clear.render(popup_area, buf);

        // Build lines
        let lines: Vec<Line> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == self.state.selected;
                let key_style = Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD);
                let label_style = if is_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };

                Line::from(vec![
                    Span::styled(format!(" [{}]", item.key), key_style),
                    Span::styled(format!("  {} ", item.label), label_style),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.state.title.as_str())
            .style(Style::default().bg(Color::DarkGray));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));

        paragraph.render(popup_area, buf);
    }
}
