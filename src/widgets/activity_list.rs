use crate::app::LoadState;
use crate::domain::{
    format_duration, format_elapsed_since, ActivityExecution, ActivityStatus,
    ChildWorkflowExecution, ChildWorkflowStatus, HistoryEvent,
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

/// A unified timeline item that can be either an activity or a child workflow
enum TimelineItem<'a> {
    Activity(&'a ActivityExecution),
    ChildWorkflow(&'a ChildWorkflowExecution),
}

impl<'a> TimelineItem<'a> {
    fn sort_key(&self) -> i64 {
        match self {
            TimelineItem::Activity(a) => a.scheduled_event_id,
            TimelineItem::ChildWorkflow(cw) => cw.initiated_event_id,
        }
    }
}

/// Renders a table of activity executions and child workflows interleaved chronologically
pub struct ActivityListWidget<'a> {
    events: &'a LoadState<Vec<HistoryEvent>>,
    activities: &'a [ActivityExecution],
    child_workflows: &'a [ChildWorkflowExecution],
    expanded: Option<usize>,
}

impl<'a> ActivityListWidget<'a> {
    pub fn new(
        events: &'a LoadState<Vec<HistoryEvent>>,
        activities: &'a [ActivityExecution],
        child_workflows: &'a [ChildWorkflowExecution],
    ) -> Self {
        Self {
            events,
            activities,
            child_workflows,
            expanded: None,
        }
    }

    pub fn expanded(mut self, expanded: Option<usize>) -> Self {
        self.expanded = expanded;
        self
    }

    fn build_timeline(&self) -> Vec<TimelineItem<'a>> {
        let mut items: Vec<TimelineItem<'a>> = Vec::with_capacity(
            self.activities.len() + self.child_workflows.len(),
        );
        for a in self.activities {
            items.push(TimelineItem::Activity(a));
        }
        for cw in self.child_workflows {
            items.push(TimelineItem::ChildWorkflow(cw));
        }
        items.sort_by_key(|item| item.sort_key());
        items
    }

    fn build_title(&self) -> String {
        let act_count = self.activities.len();
        let cw_count = self.child_workflows.len();
        let failed_acts = self
            .activities
            .iter()
            .filter(|a| matches!(a.status, ActivityStatus::Failed | ActivityStatus::TimedOut))
            .count();
        let failed_cws = self
            .child_workflows
            .iter()
            .filter(|cw| {
                matches!(
                    cw.status,
                    ChildWorkflowStatus::Failed
                        | ChildWorkflowStatus::TimedOut
                        | ChildWorkflowStatus::StartFailed
                )
            })
            .count();

        let total_failed = failed_acts + failed_cws;

        let mut parts = vec![format!("{} activities", act_count)];
        if cw_count > 0 {
            parts.push(format!("{} child workflows", cw_count));
        }
        if total_failed > 0 {
            parts.push(format!("{} failed", total_failed));
        }

        format!("Timeline ({})", parts.join(", "))
    }

    fn build_header() -> Row<'static> {
        Row::new(vec![
            Cell::from("Type").style(Style::default().bold()),
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
            Constraint::Percentage(30), // Type
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
            ActivityStatus::Completed
            | ActivityStatus::Failed
            | ActivityStatus::TimedOut
            | ActivityStatus::Canceled => activity
                .execution_time
                .as_ref()
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string()),
            ActivityStatus::Running => activity
                .started_time
                .as_ref()
                .map(format_elapsed_since)
                .unwrap_or_else(|| "-".to_string()),
            ActivityStatus::Scheduled => "-".to_string(),
        };

        let attempt_str = if activity.attempt > 0 {
            format!("{}", activity.attempt)
        } else {
            "-".to_string()
        };

        let queue_wait_str = match activity.status {
            ActivityStatus::Scheduled => format_elapsed_since(&activity.scheduled_time),
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

    fn child_workflow_to_row(cw: &ChildWorkflowExecution) -> Row<'static> {
        let status_style = Style::default().fg(cw.status.color());

        // Dimmed [cw] prefix on the type
        let type_str = format!("[cw] {}", truncate_string(&cw.workflow_type, 30));

        // Truncated workflow_id for ID column
        let id_str = truncate_string(&cw.workflow_id, 6);

        let duration_str = cw
            .execution_time
            .as_ref()
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string());

        let queue_wait_str = cw
            .start_latency
            .as_ref()
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string());

        Row::new(vec![
            Cell::from(type_str).style(Style::default().add_modifier(Modifier::DIM)),
            Cell::from(id_str),
            Cell::from(cw.status.short_name().to_string()).style(status_style),
            Cell::from(duration_str),
            Cell::from("-".to_string()), // No attempt for child workflows
            Cell::from(queue_wait_str),
        ])
    }

    fn timeline_item_to_row(item: &TimelineItem) -> Row<'static> {
        match item {
            TimelineItem::Activity(a) => Self::activity_to_row(a),
            TimelineItem::ChildWorkflow(cw) => Self::child_workflow_to_row(cw),
        }
    }

    fn render_expanded_detail(activity: &ActivityExecution, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();

        // Input
        if let Some(ref input) = activity.input {
            let input_str =
                serde_json::to_string_pretty(input).unwrap_or_else(|_| format!("{}", input));
            lines.push(Line::from(vec![
                Span::styled("  Input: ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(truncate_string(&input_str, 200)),
            ]));
        }

        // Output
        if let Some(ref output) = activity.output {
            let output_str =
                serde_json::to_string_pretty(output).unwrap_or_else(|_| format!("{}", output));
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

    fn render_expanded_cw_detail(cw: &ChildWorkflowExecution, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("  Workflow ID: ", Style::default().fg(Color::Cyan).bold()),
            Span::raw(cw.workflow_id.clone()),
        ]));

        if let Some(ref run_id) = cw.run_id {
            lines.push(Line::from(vec![
                Span::styled("  Run ID: ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(run_id.clone()),
            ]));
        }

        if let Some(ref ns) = cw.namespace {
            lines.push(Line::from(vec![
                Span::styled("  Namespace: ", Style::default().fg(Color::Yellow).bold()),
                Span::raw(ns.clone()),
            ]));
        }

        if let Some(ref failure) = cw.failure {
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

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (no additional details)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }

        let paragraph = Paragraph::new(lines).style(Style::default().bg(Color::Black));
        paragraph.render(area, buf);
    }

    fn detail_height_for_item(item: &TimelineItem) -> u16 {
        match item {
            TimelineItem::Activity(activity) => {
                let mut lines = 0u16;
                if activity.input.is_some() {
                    lines += 1;
                }
                if activity.output.is_some() {
                    lines += 1;
                }
                if let Some(ref f) = activity.failure {
                    lines += 1;
                    if let Some(ref trace) = f.stack_trace {
                        lines += trace.lines().count().min(3) as u16;
                    }
                }
                if activity.task_queue.is_some() {
                    lines += 1;
                }
                if lines == 0 {
                    lines = 1;
                }
                lines + 1
            }
            TimelineItem::ChildWorkflow(cw) => {
                let mut lines = 1u16; // workflow_id always shown
                if cw.run_id.is_some() {
                    lines += 1;
                }
                if cw.namespace.is_some() {
                    lines += 1;
                }
                if let Some(ref f) = cw.failure {
                    lines += 1;
                    if let Some(ref trace) = f.stack_trace {
                        lines += trace.lines().count().min(3) as u16;
                    }
                }
                lines + 1
            }
        }
    }
}

impl StatefulWidget for ActivityListWidget<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.events {
            LoadState::Loaded(_) => {
                let title = self.build_title();
                let timeline = self.build_timeline();

                if timeline.is_empty() {
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

                // If we have an expanded item, we render the table and detail manually
                if let Some(expanded_idx) = self.expanded {
                    self.render_with_expanded(area, buf, state, &title, expanded_idx, &timeline);
                } else {
                    let header = Self::build_header();
                    let widths = Self::build_widths();
                    let rows: Vec<Row> = timeline
                        .iter()
                        .map(Self::timeline_item_to_row)
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
                            .title("Timeline"),
                    );
                loading.render(area, buf);
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Timeline"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'a' from Workflow Detail to view activities")
                    .style(Style::default().add_modifier(Modifier::DIM))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Timeline"),
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
        timeline: &[TimelineItem],
    ) {
        // Calculate detail height based on the expanded item
        let detail_height = if expanded_idx < timeline.len() {
            Self::detail_height_for_item(&timeline[expanded_idx])
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
            let rows: Vec<Row> = timeline
                .iter()
                .map(Self::timeline_item_to_row)
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
        let rows: Vec<Row> = timeline
            .iter()
            .map(Self::timeline_item_to_row)
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
        if expanded_idx < timeline.len() {
            match &timeline[expanded_idx] {
                TimelineItem::Activity(a) => {
                    Self::render_expanded_detail(a, detail_area, buf);
                }
                TimelineItem::ChildWorkflow(cw) => {
                    Self::render_expanded_cw_detail(cw, detail_area, buf);
                }
            }
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
