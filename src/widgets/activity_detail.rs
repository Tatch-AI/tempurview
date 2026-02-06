use crate::domain::{
    format_duration, highlight_search_matches, json_to_lines, ActivityExecution,
    ChildWorkflowExecution,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Full-screen scrollable detail view for a single activity or child workflow,
/// with JSON syntax highlighting and optional search highlighting.
pub struct ActivityDetailWidget<'a> {
    lines: Vec<Line<'static>>,
    scroll: u16,
    search_query: Option<&'a str>,
    search_current_match: usize,
    search_match_count: usize,
    title: String,
    title_color: Color,
}

impl<'a> ActivityDetailWidget<'a> {
    pub fn from_activity(activity: &ActivityExecution, scroll: u16) -> Self {
        let title = format!(
            " {} [{}] {} ",
            activity.activity_type,
            activity.activity_id,
            activity.status.short_name()
        );
        let title_color = activity.status.color();
        let lines = Self::build_activity_lines_internal(activity);

        Self {
            lines,
            scroll,
            search_query: None,
            search_current_match: 0,
            search_match_count: 0,
            title,
            title_color,
        }
    }

    pub fn from_child_workflow(cw: &ChildWorkflowExecution, scroll: u16) -> Self {
        let title = format!(
            " [cw] {} {} ",
            cw.workflow_type,
            cw.status.short_name()
        );
        let title_color = cw.status.color();
        let lines = Self::build_child_wf_lines_internal(cw);

        Self {
            lines,
            scroll,
            search_query: None,
            search_current_match: 0,
            search_match_count: 0,
            title,
            title_color,
        }
    }

    pub fn search(
        mut self,
        query: Option<&'a str>,
        current_match: usize,
        match_count: usize,
    ) -> Self {
        self.search_query = query;
        self.search_current_match = current_match;
        self.search_match_count = match_count;
        self
    }

    /// Build lines for an activity — public so App can reuse for search matching.
    pub fn build_activity_lines(activity: &ActivityExecution) -> Vec<Line<'static>> {
        Self::build_activity_lines_internal(activity)
    }

    /// Build lines for a child workflow — public so App can reuse for search matching.
    pub fn build_child_wf_lines(cw: &ChildWorkflowExecution) -> Vec<Line<'static>> {
        Self::build_child_wf_lines_internal(cw)
    }

    fn build_activity_lines_internal(activity: &ActivityExecution) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Header: Type + ID + Status
        lines.push(Line::from(vec![
            Span::styled("Type:   ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                activity.activity_type.clone(),
                Style::default().fg(Color::White).bold(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("ID:     ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                activity.activity_id.clone(),
                Style::default().fg(Color::White),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                activity.status.short_name().to_string(),
                Style::default().fg(activity.status.color()).bold(),
            ),
        ]));

        // Attempt
        if activity.attempt > 0 {
            lines.push(Line::from(vec![
                Span::styled("Attempt:", Style::default().fg(Color::Cyan).bold()),
                Span::raw(format!(" {}", activity.attempt)),
            ]));
        }

        // Task Queue
        if let Some(ref tq) = activity.task_queue {
            lines.push(Line::from(vec![
                Span::styled("Queue:  ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(tq.clone()),
            ]));
        }

        // Timestamps section
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Timestamps:",
            Style::default().fg(Color::Cyan).bold(),
        )));
        lines.push(Line::from(vec![
            Span::styled("  Scheduled: ", Style::default().fg(Color::DarkGray)),
            Span::raw(activity.scheduled_time.to_rfc3339()),
        ]));
        if let Some(ref t) = activity.started_time {
            lines.push(Line::from(vec![
                Span::styled("  Started:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(t.to_rfc3339()),
            ]));
        }
        if let Some(ref t) = activity.closed_time {
            lines.push(Line::from(vec![
                Span::styled("  Closed:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(t.to_rfc3339()),
            ]));
        }

        // Durations section
        if activity.queue_wait.is_some()
            || activity.execution_time.is_some()
            || activity.total_time.is_some()
        {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Durations:",
                Style::default().fg(Color::Cyan).bold(),
            )));
            if let Some(ref d) = activity.queue_wait {
                lines.push(Line::from(vec![
                    Span::styled("  Queue Wait: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format_duration(d)),
                ]));
            }
            if let Some(ref d) = activity.execution_time {
                lines.push(Line::from(vec![
                    Span::styled("  Execution:  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format_duration(d)),
                ]));
            }
            if let Some(ref d) = activity.total_time {
                lines.push(Line::from(vec![
                    Span::styled("  Total:      ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format_duration(d)),
                ]));
            }
        }

        // Input
        if let Some(ref input) = activity.input {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Input:",
                Style::default().fg(Color::Cyan).bold(),
            )));
            let json_lines = json_to_lines(input);
            lines.extend(json_lines);
        }

        // Output
        if let Some(ref output) = activity.output {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Output:",
                Style::default().fg(Color::Green).bold(),
            )));
            let json_lines = json_to_lines(output);
            lines.extend(json_lines);
        }

        // Failure
        if let Some(ref failure) = activity.failure {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Failure:",
                Style::default().fg(Color::Red).bold(),
            )));
            lines.push(Line::from(vec![
                Span::styled("  Type:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    failure.failure_type.clone(),
                    Style::default().fg(Color::Red),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Message: ", Style::default().fg(Color::DarkGray)),
                Span::raw(failure.message.clone()),
            ]));
            if let Some(ref trace) = failure.stack_trace {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Stack Trace:",
                    Style::default().fg(Color::Red).bold(),
                )));
                for trace_line in trace.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", trace_line),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }

        lines
    }

    fn build_child_wf_lines_internal(cw: &ChildWorkflowExecution) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        // Header
        lines.push(Line::from(vec![
            Span::styled("Type:        ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                cw.workflow_type.clone(),
                Style::default().fg(Color::White).bold(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Workflow ID: ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                cw.workflow_id.clone(),
                Style::default().fg(Color::White),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Status:      ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                cw.status.short_name().to_string(),
                Style::default().fg(cw.status.color()).bold(),
            ),
        ]));

        if let Some(ref run_id) = cw.run_id {
            lines.push(Line::from(vec![
                Span::styled("Run ID:      ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(run_id.clone()),
            ]));
        }
        if let Some(ref ns) = cw.namespace {
            lines.push(Line::from(vec![
                Span::styled("Namespace:   ", Style::default().fg(Color::Cyan).bold()),
                Span::raw(ns.clone()),
            ]));
        }

        // Timestamps
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Timestamps:",
            Style::default().fg(Color::Cyan).bold(),
        )));
        lines.push(Line::from(vec![
            Span::styled("  Initiated: ", Style::default().fg(Color::DarkGray)),
            Span::raw(cw.initiated_time.to_rfc3339()),
        ]));
        if let Some(ref t) = cw.started_time {
            lines.push(Line::from(vec![
                Span::styled("  Started:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(t.to_rfc3339()),
            ]));
        }
        if let Some(ref t) = cw.closed_time {
            lines.push(Line::from(vec![
                Span::styled("  Closed:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(t.to_rfc3339()),
            ]));
        }

        // Durations
        if cw.start_latency.is_some()
            || cw.execution_time.is_some()
            || cw.total_time.is_some()
        {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Durations:",
                Style::default().fg(Color::Cyan).bold(),
            )));
            if let Some(ref d) = cw.start_latency {
                lines.push(Line::from(vec![
                    Span::styled("  Start Latency: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format_duration(d)),
                ]));
            }
            if let Some(ref d) = cw.execution_time {
                lines.push(Line::from(vec![
                    Span::styled("  Execution:     ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format_duration(d)),
                ]));
            }
            if let Some(ref d) = cw.total_time {
                lines.push(Line::from(vec![
                    Span::styled("  Total:         ", Style::default().fg(Color::DarkGray)),
                    Span::raw(format_duration(d)),
                ]));
            }
        }

        // Failure
        if let Some(ref failure) = cw.failure {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Failure:",
                Style::default().fg(Color::Red).bold(),
            )));
            lines.push(Line::from(vec![
                Span::styled("  Type:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    failure.failure_type.clone(),
                    Style::default().fg(Color::Red),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Message: ", Style::default().fg(Color::DarkGray)),
                Span::raw(failure.message.clone()),
            ]));
            if let Some(ref trace) = failure.stack_trace {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Stack Trace:",
                    Style::default().fg(Color::Red).bold(),
                )));
                for trace_line in trace.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", trace_line),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }

        lines
    }
}

impl Widget for ActivityDetailWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut title = self.title.clone();
        if let Some(query) = self.search_query {
            if self.search_match_count > 0 {
                title.push_str(&format!(
                    " [{}/{} \"{}\"] ",
                    self.search_current_match + 1,
                    self.search_match_count,
                    query
                ));
            } else {
                title.push_str(&format!(" [no matches for \"{}\"] ", query));
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                title,
                Style::default()
                    .fg(self.title_color)
                    .add_modifier(Modifier::BOLD),
            ));

        let mut lines = self.lines;

        // Apply search highlighting if active
        if let Some(query) = self.search_query {
            if !query.is_empty() {
                let (highlighted, _) = highlight_search_matches(&lines, query);
                lines = highlighted;
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        paragraph.render(area, buf);
    }
}
