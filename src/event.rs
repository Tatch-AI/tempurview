use crate::action::{Action, TableColumn};
use crate::app::{InputMode, View};
use crate::domain::DateRangePreset;
use crossterm::event::{
    Event as CrosstermEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Events that can occur
#[derive(Debug, Clone)]
pub enum Event {
    /// Keyboard input
    Key(KeyEvent),
    /// Periodic tick for animations/refresh
    Tick,
    /// Terminal resize
    Resize(u16, u16),
}

/// Event handler that runs in background
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    _task: JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut interval = tokio::time::interval(tick_rate);

            loop {
                let tick = interval.tick();
                let event = reader.next();

                tokio::select! {
                    _ = tick => {
                        if tx.send(Event::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(evt)) = event => {
                        match evt {
                            CrosstermEvent::Key(key) if key.kind == KeyEventKind::Press => {
                                if tx.send(Event::Key(key)).is_err() {
                                    break;
                                }
                            }
                            CrosstermEvent::Resize(w, h) => {
                                if tx.send(Event::Resize(w, h)).is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        Self { rx, _task: task }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

/// Pure function: map keyboard event to action
pub fn key_to_action(key: KeyEvent, view: View, input_mode: InputMode) -> Option<Action> {
    match input_mode {
        InputMode::FilterInput => match key.code {
            KeyCode::Enter => Some(Action::CloseFilterInput),
            KeyCode::Esc => Some(Action::CloseFilterInput),
            KeyCode::Char(c) => Some(Action::AppendFilterChar(c)),
            KeyCode::Backspace => Some(Action::DeleteFilterChar),
            _ => None,
        },
        InputMode::SortSelect => match key.code {
            KeyCode::Esc => Some(Action::CloseSort),
            KeyCode::Char(c) => Some(Action::SortBy(c as u8)),
            _ => None,
        },
        InputMode::DateRangeSelect => match key.code {
            KeyCode::Char('1') => Some(Action::SelectDateRangePreset(DateRangePreset::LastHour)),
            KeyCode::Char('2') => Some(Action::SelectDateRangePreset(DateRangePreset::Last6Hours)),
            KeyCode::Char('3') => {
                Some(Action::SelectDateRangePreset(DateRangePreset::Last24Hours))
            }
            KeyCode::Char('4') => Some(Action::SelectDateRangePreset(DateRangePreset::Last3Days)),
            KeyCode::Char('5') => Some(Action::SelectDateRangePreset(DateRangePreset::Last7Days)),
            KeyCode::Char('6') => Some(Action::SelectDateRangePreset(DateRangePreset::Last30Days)),
            KeyCode::Char('0') => Some(Action::ClearDateRange),
            KeyCode::Char('c') => Some(Action::EnterCustomDateInput),
            KeyCode::Esc => Some(Action::CloseDateRangeMode),
            _ => None,
        },
        InputMode::DateRangeCustom => match key.code {
            KeyCode::Enter => Some(Action::ApplyCustomDateRange),
            KeyCode::Esc => Some(Action::CancelCustomDateRange),
            KeyCode::Char(c) => Some(Action::AppendDateRangeChar(c)),
            KeyCode::Backspace => Some(Action::DeleteDateRangeChar),
            _ => None,
        },
        InputMode::PendingG => match key.code {
            KeyCode::Char('g') => Some(Action::NavigateTop),
            KeyCode::Char('x') if view == View::WorkflowDetail => Some(Action::OpenWorkflowUrl),
            KeyCode::Esc => Some(Action::CancelPendingG),
            _ => Some(Action::CancelPendingG),
        },
        InputMode::Normal => {
            // Check for Ctrl+C first
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                if let KeyCode::Char('c') = key.code {
                    return Some(Action::Quit);
                }
            }

            match key.code {
                KeyCode::Char('q') => Some(Action::Quit),
                KeyCode::Char('j') | KeyCode::Down => Some(Action::NavigateDown),
                KeyCode::Char('k') | KeyCode::Up => Some(Action::NavigateUp),
                KeyCode::Char('g') => Some(Action::EnterPendingG),
                KeyCode::Char('G') => Some(Action::NavigateBottom),
                KeyCode::Char('/') => Some(Action::OpenFilterInput),
                KeyCode::Char('r') => Some(Action::Refresh),
                KeyCode::Char('?') => Some(Action::ToggleHelp),
                KeyCode::PageUp => Some(Action::PageUp),
                KeyCode::PageDown => Some(Action::PageDown),
                KeyCode::Home => Some(Action::NavigateTop),
                KeyCode::End => Some(Action::NavigateBottom),
                KeyCode::Char('d')
                    if view == View::WorkflowList || view == View::TypeList =>
                {
                    Some(Action::EnterDateRangeMode)
                }
                KeyCode::Char('s') if view == View::WorkflowList || view == View::TypeList => {
                    Some(Action::EnterSortMode)
                }
                KeyCode::Char('T') if view == View::WorkflowList => {
                    Some(Action::ViewTypeList)
                }
                KeyCode::Char('I') if view == View::WorkflowList => {
                    Some(Action::ViewInsights)
                }
                KeyCode::Enter => match view {
                    View::WorkflowList => Some(Action::ViewDetail),
                    View::TypeList => Some(Action::ViewDetail),
                    View::ActivityList => Some(Action::ToggleActivityDetail),
                    View::Insights => Some(Action::ToggleInsightDetail),
                    View::WorkflowDetail => None,
                },
                KeyCode::Esc => Some(Action::GoBack),
                // Status filter shortcuts (number keys)
                KeyCode::Char('1') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::Running,
                ))),
                KeyCode::Char('2') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::Completed,
                ))),
                KeyCode::Char('3') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::Failed,
                ))),
                KeyCode::Char('4') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::Canceled,
                ))),
                KeyCode::Char('5') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::Terminated,
                ))),
                KeyCode::Char('6') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::TimedOut,
                ))),
                KeyCode::Char('7') => Some(Action::SetStatusFilter(Some(
                    crate::domain::WorkflowStatus::ContinuedAsNew,
                ))),
                KeyCode::Char('0') => Some(Action::ClearFilters),
                // Cycle through status filters
                KeyCode::Char(']') => Some(Action::NextStatusFilter),
                KeyCode::Char('[') => Some(Action::PrevStatusFilter),
                // Column visibility toggles (F1-F4)
                KeyCode::F(1) => Some(Action::ToggleColumn(TableColumn::Status)),
                KeyCode::F(2) => Some(Action::ToggleColumn(TableColumn::Type)),
                KeyCode::F(3) => Some(Action::ToggleColumn(TableColumn::WorkflowId)),
                KeyCode::F(4) => Some(Action::ToggleColumn(TableColumn::Started)),
                // View-specific shortcuts
                KeyCode::Char('c') if view == View::WorkflowDetail => {
                    Some(Action::CancelWorkflow(String::new())) // ID filled in by app
                }
                KeyCode::Char('t') if view == View::WorkflowDetail => {
                    Some(Action::TerminateWorkflow(String::new())) // ID filled in by app
                }
                KeyCode::Char('a') if view == View::WorkflowDetail => {
                    Some(Action::ViewActivities)
                }
                KeyCode::Char('x') if view == View::WorkflowDetail => {
                    Some(Action::CopyWorkflowUrl)
                }
                _ => None,
            }
        }
    }
}

/// Convert an event to an action
pub fn event_to_action(event: Event, view: View, input_mode: InputMode) -> Option<Action> {
    match event {
        Event::Key(key) => key_to_action(key, view, input_mode),
        Event::Tick => Some(Action::Tick),
        Event::Resize(_, _) => None, // Handled by ratatui
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_quit_mapping() {
        let key = make_key(KeyCode::Char('q'));
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn test_navigation_mapping() {
        let key = make_key(KeyCode::Char('j'));
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::NavigateDown));

        let key = make_key(KeyCode::Char('k'));
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::NavigateUp));
    }

    #[test]
    fn test_arrow_keys() {
        let key = make_key(KeyCode::Down);
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::NavigateDown));

        let key = make_key(KeyCode::Up);
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::NavigateUp));
    }

    #[test]
    fn test_filter_input_mode() {
        let key = make_key(KeyCode::Char('q'));
        // In filter input mode, 'q' should type 'q', not quit
        let action = key_to_action(key, View::WorkflowList, InputMode::FilterInput);
        assert_eq!(action, Some(Action::AppendFilterChar('q')));
    }

    #[test]
    fn test_enter_view_specific() {
        let key = make_key(KeyCode::Enter);

        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::ViewDetail));

        let action = key_to_action(key, View::WorkflowDetail, InputMode::Normal);
        assert_eq!(action, None);
    }

    #[test]
    fn test_escape_in_filter_mode() {
        let key = make_key(KeyCode::Esc);
        let action = key_to_action(key, View::WorkflowList, InputMode::FilterInput);
        assert_eq!(action, Some(Action::CloseFilterInput));
    }

    #[test]
    fn test_status_filter_shortcuts() {
        let key = make_key(KeyCode::Char('1'));
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(
            action,
            Some(Action::SetStatusFilter(Some(
                crate::domain::WorkflowStatus::Running
            )))
        );

        let key = make_key(KeyCode::Char('3'));
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(
            action,
            Some(Action::SetStatusFilter(Some(
                crate::domain::WorkflowStatus::Failed
            )))
        );

        let key = make_key(KeyCode::Char('0'));
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::ClearFilters));
    }

    #[test]
    fn test_ctrl_c_quits() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let action = key_to_action(key, View::WorkflowList, InputMode::Normal);
        assert_eq!(action, Some(Action::Quit));
    }
}
