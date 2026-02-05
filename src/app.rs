use crate::action::{Action, DataPayload, TableColumn};
use crate::domain::{StatusCounts, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary};
use ratatui::widgets::TableState;
use std::collections::HashSet;
use std::time::Instant;

/// Which view is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    WorkflowList,
    WorkflowDetail,
}

/// Input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    FilterInput,
}

/// Loading state for async data
#[derive(Debug, Clone)]
pub enum LoadState<T> {
    NotLoaded,
    Loading,
    Loaded(T),
    Error(String),
}

impl<T> LoadState<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, LoadState::Loading)
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, LoadState::Loaded(_))
    }

    pub fn as_ref(&self) -> Option<&T> {
        match self {
            LoadState::Loaded(t) => Some(t),
            _ => None,
        }
    }
}

/// Main application state
pub struct App {
    // View state
    pub view: View,
    pub view_stack: Vec<View>,
    pub input_mode: InputMode,
    pub show_help: bool,

    // Data state
    pub status_counts: LoadState<StatusCounts>,
    pub workflows: LoadState<Vec<WorkflowSummary>>,
    pub selected_workflow: Option<LoadState<WorkflowDetail>>,

    // Filter state
    pub filter: WorkflowFilter,
    pub filter_input: String,

    // Table state (for scrolling/selection)
    pub table_state: TableState,

    // Column visibility
    pub visible_columns: HashSet<TableColumn>,

    // App control
    pub should_quit: bool,
    pub last_error: Option<String>,
    pub last_refresh: Option<Instant>,
    pub last_quit_attempt: Option<Instant>,

    // Page size for scrolling
    pub page_size: usize,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let table_state = TableState::default().with_selected(0);

        // All columns visible by default
        let visible_columns: HashSet<TableColumn> = TableColumn::all().iter().copied().collect();

        Self {
            view: View::WorkflowList,
            view_stack: Vec::new(),
            input_mode: InputMode::Normal,
            show_help: false,

            status_counts: LoadState::NotLoaded,
            workflows: LoadState::NotLoaded,
            selected_workflow: None,

            filter: WorkflowFilter::new(),
            filter_input: String::new(),

            table_state,
            visible_columns,

            should_quit: false,
            last_error: None,
            last_refresh: None,
            last_quit_attempt: None,

            page_size: 10,
        }
    }

    /// Apply an action and return any side effects to perform
    /// This is a pure function - no I/O happens here
    pub fn update(&mut self, action: Action) -> Vec<Effect> {
        // Clear last error on any action (except Error, Tick, and Quit which sets its own message)
        if !matches!(action, Action::Error(_) | Action::Tick | Action::Quit) {
            self.last_error = None;
        }

        match action {
            Action::NavigateUp => {
                self.select_previous();
                vec![]
            }
            Action::NavigateDown => {
                self.select_next();
                vec![]
            }
            Action::NavigateTop => {
                self.select_first();
                vec![]
            }
            Action::NavigateBottom => {
                self.select_last();
                vec![]
            }
            Action::PageUp => {
                for _ in 0..self.page_size {
                    self.select_previous();
                }
                vec![]
            }
            Action::PageDown => {
                for _ in 0..self.page_size {
                    self.select_next();
                }
                vec![]
            }
            Action::ViewDetail => {
                if let Some(wf_id) = self.selected_workflow_id() {
                    let id = wf_id.to_string();
                    self.view_stack.push(self.view);
                    self.view = View::WorkflowDetail;
                    self.selected_workflow = Some(LoadState::Loading);
                    vec![Effect::LoadWorkflowDetail(id)]
                } else {
                    vec![]
                }
            }
            Action::GoBack => {
                if self.input_mode == InputMode::FilterInput {
                    self.input_mode = InputMode::Normal;
                    self.filter_input.clear();
                } else if let Some(prev_view) = self.view_stack.pop() {
                    self.view = prev_view;
                    self.selected_workflow = None;
                } else if self.view == View::WorkflowDetail {
                    // Go back to list from detail
                    self.view = View::WorkflowList;
                    self.selected_workflow = None;
                }
                vec![]
            }
            Action::SetStatusFilter(status) => {
                self.filter.status = status;
                self.table_state.select(Some(0));
                vec![Effect::LoadWorkflows]
            }
            Action::SetTypeFilter(workflow_type) => {
                self.filter.workflow_type = workflow_type;
                self.table_state.select(Some(0));
                vec![Effect::LoadWorkflows]
            }
            Action::NextStatusFilter => {
                let statuses = WorkflowStatus::all();
                let next_status = match self.filter.status {
                    None => Some(statuses[0]),
                    Some(current) => {
                        let current_idx = statuses.iter().position(|s| *s == current).unwrap_or(0);
                        let next_idx = (current_idx + 1) % statuses.len();
                        Some(statuses[next_idx])
                    }
                };
                self.filter.status = next_status;
                self.table_state.select(Some(0));
                vec![Effect::LoadWorkflows]
            }
            Action::PrevStatusFilter => {
                let statuses = WorkflowStatus::all();
                let prev_status = match self.filter.status {
                    None => Some(statuses[statuses.len() - 1]),
                    Some(current) => {
                        let current_idx = statuses.iter().position(|s| *s == current).unwrap_or(0);
                        let prev_idx = if current_idx == 0 {
                            statuses.len() - 1
                        } else {
                            current_idx - 1
                        };
                        Some(statuses[prev_idx])
                    }
                };
                self.filter.status = prev_status;
                self.table_state.select(Some(0));
                vec![Effect::LoadWorkflows]
            }
            Action::ToggleColumn(column) => {
                if self.visible_columns.contains(&column) {
                    // Don't allow hiding all columns - keep at least one
                    if self.visible_columns.len() > 1 {
                        self.visible_columns.remove(&column);
                    }
                } else {
                    self.visible_columns.insert(column);
                }
                vec![]
            }
            Action::ClearFilters => {
                self.filter = WorkflowFilter::new();
                self.table_state.select(Some(0));
                vec![Effect::LoadWorkflows]
            }
            Action::OpenFilterInput => {
                self.input_mode = InputMode::FilterInput;
                self.filter_input.clear();
                vec![]
            }
            Action::CloseFilterInput => {
                self.input_mode = InputMode::Normal;
                // Apply filter from input
                if !self.filter_input.is_empty() {
                    self.filter = WorkflowFilter::from_query(&self.filter_input);
                    self.filter_input.clear();
                    vec![Effect::LoadWorkflows]
                } else {
                    vec![]
                }
            }
            Action::AppendFilterChar(c) => {
                self.filter_input.push(c);
                vec![]
            }
            Action::DeleteFilterChar => {
                self.filter_input.pop();
                vec![]
            }
            Action::Refresh => {
                // Load both counts (from API) and workflows (filtered)
                self.status_counts = LoadState::Loading;
                self.workflows = LoadState::Loading;
                self.last_refresh = Some(Instant::now());
                vec![Effect::LoadCounts, Effect::LoadWorkflows]
            }
            Action::CancelWorkflow(id) => {
                let workflow_id = if id.is_empty() {
                    self.selected_workflow_id().map(|s| s.to_string())
                } else {
                    Some(id)
                };

                if let Some(wf_id) = workflow_id {
                    vec![Effect::CancelWorkflow(wf_id)]
                } else {
                    vec![]
                }
            }
            Action::TerminateWorkflow(id) => {
                let workflow_id = if id.is_empty() {
                    self.selected_workflow_id().map(|s| s.to_string())
                } else {
                    Some(id)
                };

                if let Some(wf_id) = workflow_id {
                    vec![Effect::TerminateWorkflow(wf_id)]
                } else {
                    vec![]
                }
            }
            Action::Quit => {
                let now = Instant::now();
                if let Some(last_attempt) = self.last_quit_attempt {
                    // If second Ctrl+C within 2 seconds, quit
                    if now.duration_since(last_attempt).as_secs() < 2 {
                        self.should_quit = true;
                    } else {
                        // Too much time passed, reset and show message
                        self.last_quit_attempt = Some(now);
                        self.last_error = Some("Press Ctrl+C again to quit".to_string());
                    }
                } else {
                    // First Ctrl+C, show message
                    self.last_quit_attempt = Some(now);
                    self.last_error = Some("Press Ctrl+C again to quit".to_string());
                }
                vec![]
            }
            Action::ToggleHelp => {
                self.show_help = !self.show_help;
                vec![]
            }
            Action::Tick => {
                // Could trigger auto-refresh here
                vec![]
            }
            Action::DataLoaded(payload) => {
                match payload {
                    DataPayload::Counts(counts) => {
                        self.status_counts = LoadState::Loaded(counts);
                    }
                    DataPayload::Workflows(wfs) => {
                        // Only compute counts locally if we don't have API-loaded counts
                        // API counts are more accurate as they include ALL workflows
                        if !self.status_counts.is_loaded() {
                            let counts = StatusCounts::from_workflows(&wfs);
                            self.status_counts = LoadState::Loaded(counts);
                        }

                        self.workflows = LoadState::Loaded(wfs);
                        // Reset selection if it's out of bounds
                        if let LoadState::Loaded(ref workflows) = self.workflows {
                            let selected = self.table_state.selected().unwrap_or(0);
                            if selected >= workflows.len() && !workflows.is_empty() {
                                self.table_state.select(Some(workflows.len() - 1));
                            }
                        }
                    }
                    DataPayload::Detail(detail) => {
                        self.selected_workflow = Some(LoadState::Loaded(*detail));
                    }
                }
                vec![]
            }
            Action::Error(msg) => {
                self.last_error = Some(msg.clone());
                // Update loading states to error
                if self.status_counts.is_loading() {
                    self.status_counts = LoadState::Error(msg.clone());
                }
                if self.workflows.is_loading() {
                    self.workflows = LoadState::Error(msg.clone());
                }
                if let Some(LoadState::Loading) = self.selected_workflow {
                    self.selected_workflow = Some(LoadState::Error(msg));
                }
                vec![]
            }
        }
    }

    fn select_next(&mut self) {
        let len = self.current_list_len();

        if len == 0 {
            return;
        }

        let table_state = self.current_table_state_mut();
        let current = table_state.selected().unwrap_or(0);
        let next = if current >= len - 1 {
            current
        } else {
            current + 1
        };
        table_state.select(Some(next));
    }

    fn select_previous(&mut self) {
        let table_state = self.current_table_state_mut();
        let current = table_state.selected().unwrap_or(0);
        let prev = current.saturating_sub(1);
        table_state.select(Some(prev));
    }

    fn select_first(&mut self) {
        let table_state = self.current_table_state_mut();
        table_state.select(Some(0));
    }

    fn select_last(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            let table_state = self.current_table_state_mut();
            table_state.select(Some(len - 1));
        }
    }

    fn current_table_state_mut(&mut self) -> &mut TableState {
        // Always navigate the workflow table with j/k
        &mut self.table_state
    }

    fn current_list_len(&self) -> usize {
        // Always use workflow list length for navigation
        if let LoadState::Loaded(ref workflows) = self.workflows {
            workflows.len()
        } else {
            0
        }
    }

    pub fn selected_workflow_id(&self) -> Option<&str> {
        if let LoadState::Loaded(ref workflows) = self.workflows {
            let selected = self.table_state.selected().unwrap_or(0);
            workflows.get(selected).map(|wf| wf.workflow_id.as_str())
        } else {
            None
        }
    }

    pub fn selected_workflow_run_id(&self) -> Option<&str> {
        if let LoadState::Loaded(ref workflows) = self.workflows {
            let selected = self.table_state.selected().unwrap_or(0);
            workflows.get(selected).map(|wf| wf.run_id.as_str())
        } else {
            None
        }
    }
}

/// Side effects to be performed after state update
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadCounts,
    LoadWorkflows,
    LoadWorkflowDetail(String),
    CancelWorkflow(String),
    TerminateWorkflow(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::WorkflowStatus;
    use chrono::Utc;

    fn make_test_workflows() -> Vec<WorkflowSummary> {
        vec![
            WorkflowSummary {
                workflow_id: "wf-1".to_string(),
                run_id: "run-1".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Running,
                start_time: Utc::now(),
                close_time: None,
                task_queue: "default".to_string(),
            },
            WorkflowSummary {
                workflow_id: "wf-2".to_string(),
                run_id: "run-2".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Completed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: "default".to_string(),
            },
            WorkflowSummary {
                workflow_id: "wf-3".to_string(),
                run_id: "run-3".to_string(),
                workflow_type: "Test".to_string(),
                status: WorkflowStatus::Failed,
                start_time: Utc::now(),
                close_time: Some(Utc::now()),
                task_queue: "default".to_string(),
            },
        ]
    }

    #[test]
    fn test_navigation() {
        let mut app = App::new();
        app.view = View::WorkflowList;
        app.workflows = LoadState::Loaded(make_test_workflows());

        assert_eq!(app.table_state.selected(), Some(0));

        app.update(Action::NavigateDown);
        assert_eq!(app.table_state.selected(), Some(1));

        app.update(Action::NavigateDown);
        assert_eq!(app.table_state.selected(), Some(2));

        // Should not go past the end
        app.update(Action::NavigateDown);
        assert_eq!(app.table_state.selected(), Some(2));

        app.update(Action::NavigateUp);
        assert_eq!(app.table_state.selected(), Some(1));

        app.update(Action::NavigateTop);
        assert_eq!(app.table_state.selected(), Some(0));

        app.update(Action::NavigateBottom);
        assert_eq!(app.table_state.selected(), Some(2));
    }

    #[test]
    fn test_refresh_triggers_load_effects() {
        let mut app = App::new();
        let effects = app.update(Action::Refresh);

        assert!(matches!(app.status_counts, LoadState::Loading));
        assert!(matches!(app.workflows, LoadState::Loading));
        // LoadCounts loads counts for all statuses from API
        // LoadWorkflows loads the filtered workflow list
        assert!(effects.contains(&Effect::LoadCounts));
        assert!(effects.contains(&Effect::LoadWorkflows));
    }

    #[test]
    fn test_quit() {
        let mut app = App::new();
        assert!(!app.should_quit);

        // First quit shows warning
        app.update(Action::Quit);
        assert!(!app.should_quit);
        assert_eq!(app.last_error, Some("Press Ctrl+C again to quit".to_string()));

        // Second quit actually quits
        app.update(Action::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_view_switching() {
        let mut app = App::new();
        app.workflows = LoadState::Loaded(make_test_workflows());

        assert_eq!(app.view, View::WorkflowList);

        app.update(Action::ViewDetail);
        assert_eq!(app.view, View::WorkflowDetail);
        assert_eq!(app.view_stack, vec![View::WorkflowList]);

        app.update(Action::GoBack);
        assert_eq!(app.view, View::WorkflowList);
    }

    #[test]
    fn test_filter_input_mode() {
        let mut app = App::new();

        app.update(Action::OpenFilterInput);
        assert_eq!(app.input_mode, InputMode::FilterInput);

        app.update(Action::AppendFilterChar('t'));
        app.update(Action::AppendFilterChar('e'));
        app.update(Action::AppendFilterChar('s'));
        app.update(Action::AppendFilterChar('t'));
        assert_eq!(app.filter_input, "test");

        app.update(Action::DeleteFilterChar);
        assert_eq!(app.filter_input, "tes");

        app.update(Action::CloseFilterInput);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_set_status_filter() {
        let mut app = App::new();
        let effects = app.update(Action::SetStatusFilter(Some(WorkflowStatus::Failed)));

        assert_eq!(app.filter.status, Some(WorkflowStatus::Failed));
        assert!(effects.contains(&Effect::LoadWorkflows));
    }

    #[test]
    fn test_clear_filters() {
        let mut app = App::new();
        app.filter.status = Some(WorkflowStatus::Failed);
        app.filter.workflow_type = Some("Test".to_string());

        let effects = app.update(Action::ClearFilters);

        assert!(app.filter.is_empty());
        assert!(effects.contains(&Effect::LoadWorkflows));
    }

    #[test]
    fn test_data_loaded() {
        let mut app = App::new();

        let mut counts = StatusCounts::new();
        counts.set(WorkflowStatus::Running, 10);
        app.update(Action::DataLoaded(DataPayload::Counts(counts.clone())));

        assert!(matches!(app.status_counts, LoadState::Loaded(_)));
        if let LoadState::Loaded(c) = &app.status_counts {
            assert_eq!(c.get(WorkflowStatus::Running), 10);
        }
    }

    #[test]
    fn test_error_handling() {
        let mut app = App::new();
        app.status_counts = LoadState::Loading;
        app.workflows = LoadState::Loading;

        app.update(Action::Error("Test error".to_string()));

        assert_eq!(app.last_error, Some("Test error".to_string()));
        assert!(matches!(app.status_counts, LoadState::Error(_)));
        assert!(matches!(app.workflows, LoadState::Error(_)));
    }

    #[test]
    fn test_selected_workflow_id() {
        let mut app = App::new();
        app.workflows = LoadState::Loaded(make_test_workflows());
        app.table_state.select(Some(1));

        assert_eq!(app.selected_workflow_id(), Some("wf-2"));
    }
}
