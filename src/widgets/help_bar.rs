use crate::app::{InputMode, View};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// Renders contextual keyboard shortcuts at the bottom
pub struct HelpBar {
    shortcuts: Vec<(&'static str, &'static str)>,
}

impl HelpBar {
    pub fn for_view(view: View, input_mode: InputMode) -> Self {
        let shortcuts = match (view, input_mode) {
            (_, InputMode::FilterInput) => vec![
                ("Enter", "Apply"),
                ("Esc", "Cancel"),
            ],
            (View::Dashboard, _) => vec![
                ("j/k", "Navigate"),
                ("1-5", "Filter status"),
                ("0", "Clear"),
                ("Enter", "View list"),
                ("r", "Refresh"),
                ("?", "Help"),
                ("q", "Quit"),
            ],
            (View::WorkflowList, _) => vec![
                ("j/k", "Navigate"),
                ("Enter", "Details"),
                ("/", "Filter"),
                ("1-5", "Status"),
                ("0", "Clear"),
                ("Esc", "Back"),
                ("r", "Refresh"),
                ("q", "Quit"),
            ],
            (View::WorkflowDetail, _) => vec![
                ("Esc", "Back"),
                ("c", "Cancel"),
                ("t", "Terminate"),
                ("r", "Refresh"),
                ("q", "Quit"),
            ],
        };

        Self { shortcuts }
    }
}

impl Widget for HelpBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spans: Vec<Span> = self
            .shortcuts
            .iter()
            .flat_map(|(key, action)| {
                vec![
                    Span::styled(
                        format!(" {} ", key),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {} ", action),
                        Style::default().fg(Color::White),
                    ),
                ]
            })
            .collect();

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

/// A more detailed help overlay
pub struct HelpOverlay;

impl Widget for HelpOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        // Clear the area first
        Clear.render(area, buf);

        let help_text = r#"Tempurview - Keyboard Shortcuts

Navigation:
  j / ↓       Move down
  k / ↑       Move up
  g / Home    Go to top
  G / End     Go to bottom
  PgUp/PgDn   Page up/down

Views:
  Enter       Select / View details
  Esc         Go back

Filtering:
  /           Open filter input
  1           Filter: Running
  2           Filter: Completed
  3           Filter: Failed
  4           Filter: Canceled
  5           Filter: Terminated
  0           Clear all filters

Actions:
  r           Refresh data
  c           Cancel workflow (detail view)
  t           Terminate workflow (detail view)

Other:
  ?           Toggle this help
  q           Quit
  Ctrl+C      Quit
"#;

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Help")
            .style(Style::default().bg(Color::DarkGray));

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));

        paragraph.render(area, buf);
    }
}
