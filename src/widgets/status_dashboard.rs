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
}

impl<'a> StatusDashboard<'a> {
    pub fn new(counts: &'a LoadState<StatusCounts>) -> Self {
        Self {
            counts,
            selected_status: None,
        }
    }

    pub fn selected(mut self, status: Option<WorkflowStatus>) -> Self {
        self.selected_status = status;
        self
    }
}

impl Widget for StatusDashboard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.counts {
            LoadState::Loading => {
                let loading = Paragraph::new("Loading...")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(Block::default().borders(Borders::ALL).title("Status Dashboard"));
                loading.render(area, buf);
            }
            LoadState::Loaded(counts) => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Status Dashboard (Total: {})", format_count(counts.total())));

                let inner = block.inner(area);
                block.render(area, buf);

                // Create layout for status boxes
                let statuses = WorkflowStatus::all();
                let chunks = Layout::horizontal(
                    statuses.iter().map(|_| Constraint::Ratio(1, statuses.len() as u32)).collect::<Vec<_>>()
                ).split(inner);

                for (i, status) in statuses.iter().enumerate() {
                    let count = counts.get(*status);
                    let is_selected = self.selected_status == Some(*status);

                    render_status_box(*status, count, is_selected, chunks[i], buf);
                }
            }
            LoadState::Error(e) => {
                let error = Paragraph::new(format!("Error: {}", e))
                    .style(Style::default().fg(Color::Red))
                    .block(Block::default().borders(Borders::ALL).title("Status Dashboard"));
                error.render(area, buf);
            }
            LoadState::NotLoaded => {
                let empty = Paragraph::new("Press 'r' to refresh")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(Block::default().borders(Borders::ALL).title("Status Dashboard"));
                empty.render(area, buf);
            }
        }
    }
}

fn render_status_box(status: WorkflowStatus, count: u64, is_selected: bool, area: Rect, buf: &mut Buffer) {
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
            Style::default().fg(status.color()).add_modifier(Modifier::BOLD),
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
