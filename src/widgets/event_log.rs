use crate::app::LoadState;
use crate::domain::HistoryEvent;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Full-screen scrollable pager showing every history event chronologically
pub struct EventLogWidget<'a> {
    events: &'a LoadState<Vec<HistoryEvent>>,
    scroll: u16,
}

impl<'a> EventLogWidget<'a> {
    pub fn new(events: &'a LoadState<Vec<HistoryEvent>>, scroll: u16) -> Self {
        Self { events, scroll }
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

    fn build_lines(events: &[HistoryEvent]) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        for event in events {
            let ts = event.timestamp.format("%H:%M:%S%.3f").to_string();
            let color = Self::event_color(&event.event_type);

            // Main line: [HH:MM:SS.mmm] #id EventTypeName
            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", ts),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("#{:<4} ", event.event_id),
                    Style::default().fg(Color::White).bold(),
                ),
                Span::styled(event.event_type.clone(), Style::default().fg(color)),
            ]));

            // Detail line with key-value pairs from details (skip internal *_event_id keys)
            if let Some(obj) = event.details.as_object() {
                let kvs: Vec<String> = obj
                    .iter()
                    .filter(|(k, _)| !k.ends_with("_event_id"))
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, v)| {
                        let val_str = match v {
                            serde_json::Value::String(s) => truncate(s, 60),
                            other => truncate(&other.to_string(), 60),
                        };
                        format!("{}={}", k, val_str)
                    })
                    .collect();

                if !kvs.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("       {}", kvs.join("  ")),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }

        lines
    }
}

impl Widget for EventLogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.events {
            LoadState::Loaded(events) => {
                let title = format!(" Event Log ({} events) ", events.len());
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(
                        title,
                        Style::default().fg(Color::Cyan).bold(),
                    ));

                let lines = Self::build_lines(events);

                let paragraph = Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0));

                paragraph.render(area, buf);
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Loading event history...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Event Log"),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Event Log"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'l' from Workflow Detail to view event log")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Event Log"),
                    );
                empty.render(area, buf);
            }
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
