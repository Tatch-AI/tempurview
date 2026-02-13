use crate::app::LoadState;
use crate::domain::{StatusCounts, WorkflowStatus};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Renders a dashboard showing workflow counts by status
pub struct StatusDashboard<'a> {
    counts: &'a LoadState<StatusCounts>,
    selected_status: Option<WorkflowStatus>,
    activity_fail_count: Option<u64>,
    activity_fail_selected: bool,
    activity_fail_scanning: bool,
}

impl<'a> StatusDashboard<'a> {
    pub fn new(counts: &'a LoadState<StatusCounts>) -> Self {
        Self {
            counts,
            selected_status: None,
            activity_fail_count: None,
            activity_fail_selected: false,
            activity_fail_scanning: false,
        }
    }

    pub fn selected(mut self, status: Option<WorkflowStatus>) -> Self {
        self.selected_status = status;
        self
    }

    pub fn activity_fail(mut self, count: Option<u64>, selected: bool, scanning: bool) -> Self {
        self.activity_fail_count = count;
        self.activity_fail_selected = selected;
        self.activity_fail_scanning = scanning;
        self
    }
}

impl Widget for StatusDashboard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.counts {
            LoadState::Loading => {
                let loading = Paragraph::new("Loading...")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Status Dashboard"),
                    );
                loading.render(area, buf);
            }
            LoadState::Loaded(counts) => {
                let block = Block::default().borders(Borders::ALL).title(format!(
                    "Status Dashboard (Total: {})",
                    format_count(counts.total())
                ));

                let inner = block.inner(area);
                block.render(area, buf);

                // Create layout: RUN, OK, W-FAIL, A-FAIL, CANC, TERM, TIME, CONT
                let statuses = WorkflowStatus::all();
                let box_count = statuses.len() as u32 + 1; // +1 for A-FAIL
                let chunks = Layout::horizontal(
                    (0..box_count)
                        .map(|_| Constraint::Ratio(1, box_count))
                        .collect::<Vec<_>>(),
                )
                .split(inner);

                // A-FAIL goes right after Failed (index 2), so at chunk index 3
                let afail_chunk = 3;
                let mut chunk_idx = 0;
                for status in statuses.iter() {
                    if chunk_idx == afail_chunk {
                        // Render A-FAIL box first at this position
                        render_afail_box(
                            self.activity_fail_count,
                            self.activity_fail_selected,
                            self.activity_fail_scanning,
                            chunks[chunk_idx],
                            buf,
                        );
                        chunk_idx += 1;
                    }
                    let count = counts.get(*status);
                    let is_selected = self.selected_status == Some(*status);
                    render_status_box(*status, count, is_selected, chunks[chunk_idx], buf);
                    chunk_idx += 1;
                }
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Status Dashboard"),
                    );
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'r' to refresh")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Status Dashboard"),
                    );
                empty.render(area, buf);
            }
        }
    }
}

fn render_status_box(
    status: WorkflowStatus,
    count: u64,
    is_selected: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    let mut style = Style::default().fg(status.color());

    if is_selected {
        style = style.add_modifier(Modifier::REVERSED);
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .title(Span::styled(status.short_name(), style));

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height > 0 && inner.width > 0 {
        let count_str = format_count(count);
        let count_line = Line::from(Span::styled(
            count_str,
            Style::default()
                .fg(status.color())
                .add_modifier(Modifier::BOLD),
        ));

        let paragraph = Paragraph::new(count_line);
        paragraph.render(inner, buf);
    }
}

fn render_afail_box(
    count: Option<u64>,
    is_selected: bool,
    scanning: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    let color = Color::LightRed;
    let mut style = Style::default().fg(color);

    if is_selected {
        style = style.add_modifier(Modifier::REVERSED);
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .title(Span::styled("A-FAIL", style));

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height > 0 && inner.width > 0 {
        let count_str = match count {
            Some(c) if scanning && c == 0 => "...".to_string(),
            Some(c) if scanning => format!("~{}", format_count(c)),
            Some(c) => format_count(c),
            None => "—".to_string(),
        };
        let count_line = Line::from(Span::styled(
            count_str,
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD),
        ));

        let paragraph = Paragraph::new(count_line);
        paragraph.render(inner, buf);
    }
}

/// Pure function to format a count for display
pub fn format_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_count() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(42), "42");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1.0K");
        assert_eq!(format_count(1500), "1.5K");
        assert_eq!(format_count(10000), "10.0K");
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(2_500_000), "2.5M");
    }
}
