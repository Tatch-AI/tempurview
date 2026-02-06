use crate::app::LoadState;
use crate::domain::HistoryEvent;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// Table view of all history events, selectable with j/k and Enter for drill-down
pub struct EventLogWidget<'a> {
    events: &'a LoadState<Vec<HistoryEvent>>,
}

impl<'a> EventLogWidget<'a> {
    pub fn new(events: &'a LoadState<Vec<HistoryEvent>>) -> Self {
        Self { events }
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

    /// Build a summary string from the first few non-null, non-event-id detail fields
    fn summary(event: &HistoryEvent) -> String {
        if let Some(obj) = event.details.as_object() {
            let kvs: Vec<String> = obj
                .iter()
                .filter(|(k, _)| !k.ends_with("_event_id") && *k != "event_id")
                .filter(|(_, v)| !v.is_null())
                .take(3)
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => truncate(s, 40),
                        other => truncate(&other.to_string(), 40),
                    };
                    format!("{}={}", k, val_str)
                })
                .collect();
            kvs.join("  ")
        } else {
            String::new()
        }
    }
}

impl StatefulWidget for EventLogWidget<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        match self.events {
            LoadState::Loaded(events) => {
                let title = format!(" Event Log ({} events) ", events.len());
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(
                        title,
                        Style::default().fg(Color::Cyan).bold(),
                    ));

                let header = Row::new(vec!["#", "Time", "Event Type", "Summary"])
                    .style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(0);

                let rows: Vec<Row> = events
                    .iter()
                    .map(|event| {
                        let ts = event.timestamp.format("%H:%M:%S%.3f").to_string();
                        let color = Self::event_color(&event.event_type);
                        let summary = Self::summary(event);

                        Row::new(vec![
                            format!("#{}", event.event_id),
                            ts,
                            event.event_type.clone(),
                            summary,
                        ])
                        .style(Style::default().fg(color))
                    })
                    .collect();

                let widths = [
                    Constraint::Length(6),
                    Constraint::Length(14),
                    Constraint::Length(35),
                    Constraint::Fill(1),
                ];

                let table = Table::new(rows, widths)
                    .header(header)
                    .block(block)
                    .row_highlight_style(
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");

                StatefulWidget::render(table, area, buf, state);
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
