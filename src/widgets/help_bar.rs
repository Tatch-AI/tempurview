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
            (_, InputMode::FilterInput) => vec![("Enter", "Apply"), ("Esc", "Cancel")],
            (_, InputMode::DateRangeSelect) => vec![
                ("1", "1h"),
                ("2", "6h"),
                ("3", "24h"),
                ("4", "3d"),
                ("5", "7d"),
                ("6", "30d"),
                ("0", "Clear"),
                ("c", "Custom"),
                ("Esc", "Cancel"),
            ],
            (_, InputMode::DateRangeCustom) => {
                vec![("Enter", "Apply"), ("Esc", "Cancel")]
            }
            (View::WorkflowList, InputMode::SortSelect) => vec![
                ("s", "Status"),
                ("t", "Type"),
                ("w", "Workflow ID"),
                ("d", "Date"),
                ("Esc", "Cancel"),
            ],
            (View::TypeList, InputMode::SortSelect) => vec![
                ("t", "Type"),
                ("n", "Total"),
                ("1-7", "Status col"),
                ("Esc", "Cancel"),
            ],
            (View::WorkflowList, _) => vec![
                ("j/k", "Navigate"),
                ("Enter", "Details"),
                ("d", "Date"),
                ("s", "Sort"),
                ("T", "Types"),
                ("/", "Filter"),
                ("1-7", "Status"),
                ("r", "Refresh"),
                ("?", "Help"),
            ],
            (View::TypeList, _) => vec![
                ("j/k", "Navigate"),
                ("Enter", "Select"),
                ("d", "Date"),
                ("s", "Sort"),
                ("/", "Search"),
                ("Esc", "Back"),
                ("?", "Help"),
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
                    Span::styled(format!(" {} ", action), Style::default().fg(Color::White)),
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
  T           Workflow Types view

Sorting:
  s           Enter sort mode
    then: s/t/w/d (WorkflowList)
    then: t/n/1-7 (TypeList)

Filtering:
  /           Open filter input
  1           Filter: Running
  2           Filter: Completed
  3           Filter: Failed
  4           Filter: Canceled
  5           Filter: Terminated
  6           Filter: TimedOut
  7           Filter: ContinuedAsNew
  0           Clear all filters
  ]           Next status filter
  [           Previous status filter

Date Range:
  d           Enter date range mode
    then: 1-6  Preset (1h/6h/24h/3d/7d/30d)
    then: 0    Clear date range
    then: c    Custom (e.g. 2h, 3d, 1w)

Column Visibility:
  F1          Toggle Status column
  F2          Toggle Type column
  F3          Toggle Workflow ID column
  F4          Toggle Started column

Actions:
  r           Refresh data
  c           Cancel workflow (detail view)
  t           Terminate workflow (detail view)

Other:
  ?           Toggle this help
  q           Quit (press twice)
  Ctrl+C      Quit (press twice)
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
