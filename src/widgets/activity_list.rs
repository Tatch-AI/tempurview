use crate::app::LoadState;
use crate::domain::{
    format_duration, format_elapsed_since, ActivityExecution, ActivityStatus, HistoryEvent,
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// Renders a table of activity executions extracted from workflow history
pub struct ActivityListWidget<'a> {
    events: &'a LoadState<Vec<HistoryEvent>>,
    activities: &'a [ActivityExecution],
    expanded: Option<usize>,
}

impl<'a> ActivityListWidget<'a> {
    pub fn new(
        events: &'a LoadState<Vec<HistoryEvent>>,
        activities: &'a [ActivityExecution],
    ) -> Self {
        Self {
            events,
            activities,
            expanded: None,
        }
    }

    pub fn expanded(mut self, expanded: Option<usize>) -> Self {
        self.expanded = expanded;
        self
    }

    fn build_title(&self) -> String {
        let total = self.activities.len();
        let failed = self
            .activities
            .iter()
            .filter(|a| matches!(a.status, ActivityStatus::Failed | ActivityStatus::TimedOut))
            .count();

        if failed > 0 {
            format!("Activities ({} total, {} failed)", total, failed)
        } else {
            format!("Activities ({} total)", total)
        }
    }

    fn build_header() -> Row<'static> {
        Row::new(vec![
            Cell::from("Activity Type").style(Style::default().bold()),
            Cell::from("ID").style(Style::default().bold()),
            Cell::from("Status").style(Style::default().bold()),
            Cell::from("Duration").style(Style::default().bold()),
            Cell::from("Att").style(Style::default().bold()),
            Cell::from("Queue Wait").style(Style::default().bold()),
        ])
        .style(Style::default().fg(Color::Cyan))
        .bottom_margin(0)
    }

    fn build_widths() -> Vec<Constraint> {
        vec![
            Constraint::Percentage(30), // Activity Type
            Constraint::Length(6),      // ID
            Constraint::Length(6),      // Status
            Constraint::Length(12),     // Duration
            Constraint::Length(5),      // Attempts
            Constraint::Length(12),     // Queue Wait
        ]
    }

    fn activity_to_row(activity: &ActivityExecution) -> Row<'static> {
        let status_style = Style::default().fg(activity.status.color());

        let duration_str = match activity.status {
            ActivityStatus::Completed | ActivityStatus::Failed | ActivityStatus::TimedOut | ActivityStatus::Canceled => {
                activity
                    .execution_time
                    .as_ref()
                    .map(format_duration)
                    .unwrap_or_else(|| "-".to_string())
            }
            ActivityStatus::Running => {
                activity
                    .started_time
                    .as_ref()
                    .map(format_elapsed_since)
                    .unwrap_or_else(|| "-".to_string())
            }
            ActivityStatus::Scheduled => "-".to_string(),
        };

        let attempt_str = if activity.attempt > 0 {
            format!("{}", activity.attempt)
        } else {
            "-".to_string()
        };

        let queue_wait_str = match activity.status {
            ActivityStatus::Scheduled => {
                // Show how long it's been waiting
                format_elapsed_since(&activity.scheduled_time)
            }
            _ => activity
                .queue_wait
                .as_ref()
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string()),
        };

        Row::new(vec![
            Cell::from(truncate_string(&activity.activity_type, 35)),
            Cell::from(activity.activity_id.clone()),
            Cell::from(activity.status.short_name().to_string()).style(status_style),
            Cell::from(duration_str),
            Cell::from(attempt_str),
            Cell::from(queue_wait_str),
        ])
    }

    fn render_expanded_detail(
        activity: &ActivityExecution,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let mut lines: Vec<Line> = Vec::new();

        // Input
        if let Some(ref input) = activity.input {
            let input_str = serde_json::to_string_pretty(input).unwrap_or_else(|_| format!("{}", input));
            lines.push(Line::from(vec![
                Span::styled("  Input: ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(truncate_string(&input_str, 200)),
            ]));
        }

        // Output
        if let Some(ref output) = activity.output {
            let output_str = serde_json::to_string_pretty(output).unwrap_or_else(|_| format!("{}", output));
            lines.push(Line::from(vec![
                Span::styled("  Output: ", Style::default().fg(Color::Green).bold()),
                Span::raw(truncate_string(&output_str, 200)),
            ]));
        }

        // Failure
        if let Some(ref failure) = activity.failure {
            lines.push(Line::from(vec![
                Span::styled("  Failure: ", Style::default().fg(Color::Red).bold()),
                Span::raw(format!("[{}] {}", failure.failure_type, failure.message)),
            ]));
            if let Some(ref trace) = failure.stack_trace {
                for trace_line in trace.lines().take(3) {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            trace_line.to_string(),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }
        }

        // Task queue
        if let Some(ref tq) = activity.task_queue {
            lines.push(Line::from(vec![
                Span::styled("  Queue: ", Style::default().fg(Color::Yellow).bold()),
                Span::raw(tq.clone()),
            ]));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (no additional details)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }

        let paragraph = Paragraph::new(lines).style(Style::default().bg(Color::Black));
        paragraph.render(area, buf);
    }
}

impl StatefulWidget for ActivityListWidget<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.events {
            LoadState::Loaded(_) => {
                let title = self.build_title();

                if self.activities.is_empty() {
                    let empty = Paragraph::new("No activities found in workflow history")
                        .style(Style::default().add_modifier(Modifier::DIM))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title),
                        );
                    empty.render(area, buf);
                    return;
                }

                // If we have an expanded activity, we render the table and detail manually
                if let Some(expanded_idx) = self.expanded {
                    self.render_with_expanded(area, buf, state, &title, expanded_idx);
                } else {
                    let header = Self::build_header();
                    let widths = Self::build_widths();
                    let rows: Vec<Row> = self
                        .activities
                        .iter()
                        .map(Self::activity_to_row)
                        .collect();

                    let table = Table::new(rows, widths)
                        .header(header)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(title),
                        )
                        .row_highlight_style(
                            Style::default()
                                .add_modifier(Modifier::REVERSED)
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol(">> ");

                    StatefulWidget::render(table, area, buf, state);
                }
            }
            LoadState::Loading => {
                let loading = Paragraph::new("Loading activity history...")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Activities"),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Activities"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'a' from Workflow Detail to view activities")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Activities"),
                    );
                empty.render(area, buf);
            }
        }
    }
}

impl ActivityListWidget<'_> {
    fn render_with_expanded(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut TableState,
        title: &str,
        expanded_idx: usize,
    ) {
        // Calculate detail height based on the expanded activity
        let detail_height = if expanded_idx < self.activities.len() {
            let activity = &self.activities[expanded_idx];
            let mut lines = 0u16;
            if activity.input.is_some() { lines += 1; }
            if activity.output.is_some() { lines += 1; }
            if let Some(ref f) = activity.failure {
                lines += 1;
                if let Some(ref trace) = f.stack_trace {
                    lines += trace.lines().count().min(3) as u16;
                }
            }
            if activity.task_queue.is_some() { lines += 1; }
            if lines == 0 { lines = 1; } // "(no additional details)"
            lines + 1 // padding
        } else {
            0
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.to_string());
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 + detail_height {
            // Not enough space, just render the table without expansion
            let header = Self::build_header();
            let widths = Self::build_widths();
            let rows: Vec<Row> = self
                .activities
                .iter()
                .map(Self::activity_to_row)
                .collect();

            let table = Table::new(rows, widths)
                .header(header)
                .row_highlight_style(
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            StatefulWidget::render(table, inner, buf, state);
            return;
        }

        // Split: table area and detail area
        let table_height = inner.height.saturating_sub(detail_height);
        let table_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: table_height,
        };
        let detail_area = Rect {
            x: inner.x,
            y: inner.y + table_height,
            width: inner.width,
            height: detail_height,
        };

        // Render table
        let header = Self::build_header();
        let widths = Self::build_widths();
        let rows: Vec<Row> = self
            .activities
            .iter()
            .map(Self::activity_to_row)
            .collect();

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        StatefulWidget::render(table, table_area, buf, state);

        // Render expanded detail
        if expanded_idx < self.activities.len() {
            Self::render_expanded_detail(&self.activities[expanded_idx], detail_area, buf);
        }
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    // Collapse to single line for display
    let single_line: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if single_line.len() > max_len {
        format!("{}...", &single_line[..max_len.saturating_sub(3)])
    } else {
        single_line
    }
}
