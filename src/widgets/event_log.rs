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
    filtered_indices: Option<&'a [usize]>,
}

impl<'a> EventLogWidget<'a> {
    pub fn new(events: &'a LoadState<Vec<HistoryEvent>>) -> Self {
        Self {
            events,
            filtered_indices: None,
        }
    }

    pub fn filtered_indices(mut self, indices: Option<&'a [usize]>) -> Self {
        self.filtered_indices = indices;
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

    fn event_to_row(event: &HistoryEvent) -> Row<'static> {
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
    }
}

impl StatefulWidget for EventLogWidget<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut TableState) {
        match self.events {
            LoadState::Loaded(events) => {
                let total_len = match self.filtered_indices {
                    Some(indices) => indices.len(),
                    None => events.len(),
                };

                let title = if self.filtered_indices.is_some() {
                    format!(
                        " Event Log ({}/{} events) ",
                        total_len,
                        events.len()
                    )
                } else {
                    format!(" Event Log ({} events) ", events.len())
                };
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(
                        title,
                        Style::default().fg(Color::Cyan).bold(),
                    ));

                if total_len == 0 {
                    let empty = Paragraph::new("No matching events")
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(block);
                    empty.render(area, buf);
                    return;
                }

                let header = Row::new(vec!["#", "Time", "Event Type", "Summary"])
                    .style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(0);

                let widths = [
                    Constraint::Length(6),
                    Constraint::Length(14),
                    Constraint::Length(35),
                    Constraint::Fill(1),
                ];

                // Virtual viewport rendering
                // borders(2) + header(1) = 3
                let viewport_height = area.height.saturating_sub(3) as usize;
                let offset = state.offset();
                let selected = state.selected().unwrap_or(0);

                let adjusted_offset = if selected < offset {
                    selected
                } else if viewport_height > 0 && selected >= offset + viewport_height {
                    selected.saturating_sub(viewport_height - 1)
                } else {
                    offset
                };
                *state.offset_mut() = adjusted_offset;

                let end = (adjusted_offset + viewport_height).min(total_len);

                let rows: Vec<Row> = (adjusted_offset..end)
                    .map(|visible_idx| {
                        let data_idx = match self.filtered_indices {
                            Some(indices) => indices[visible_idx],
                            None => visible_idx,
                        };
                        Self::event_to_row(&events[data_idx])
                    })
                    .collect();

                let table = Table::new(rows, widths)
                    .header(header)
                    .block(block)
                    .row_highlight_style(
                        Style::default()
                            .add_modifier(Modifier::REVERSED)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");

                let mut local_state = TableState::default();
                if selected >= adjusted_offset && selected < end {
                    local_state.select(Some(selected - adjusted_offset));
                }
                StatefulWidget::render(table, area, buf, &mut local_state);
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
