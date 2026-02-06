use crate::action::{Action, DataPayload, TableColumn};
use crate::domain::{
    correlate_activities, correlate_child_workflows, parse_date_input, ActivityExecution,
    ChildWorkflowExecution, HistoryEvent, InsightsResult, SortDirection, StatusCounts,
    TypeListColumn, TypeStat, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary,
};
use ratatui::widgets::TableState;
use std::collections::HashSet;
use std::time::Instant;

/// Which view is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    WorkflowList,
    WorkflowDetail,
    TypeList,
    ActivityList,
    EventLog,
    Insights,
    InsightDetail,
}

/// Input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    FilterInput,
    SortSelect,
    DateRangeSelect,
    DateRangeCustom,
    /// Waiting for second key after 'g' press (vim-style chord)
    PendingG,
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
    pub date_range_input: String,
    pub active_date_range_label: Option<String>,
    pub type_name_filter: Option<String>,

    // Table state (for scrolling/selection)
    pub table_state: TableState,

    // Column visibility
    pub visible_columns: HashSet<TableColumn>,

    // Sort state
    pub workflow_sort: Option<(TableColumn, SortDirection)>,
    pub type_sort: Option<(TypeListColumn, SortDirection)>,

    // TypeList state
    pub type_stats: LoadState<Vec<TypeStat>>,
    pub type_table_state: TableState,

    // ActivityList state
    pub activity_events: LoadState<Vec<HistoryEvent>>,
    pub activities: Vec<ActivityExecution>,
    pub child_workflows: Vec<ChildWorkflowExecution>,
    pub activity_table_state: TableState,
    pub expanded_activity: Option<usize>,

    // EventLog state
    pub event_log_scroll: u16,

    // Insights state
    pub insights: LoadState<InsightsResult>,
    pub insights_table_state: TableState,
    pub insight_detail_scroll: u16,
    pub insight_entity_index: usize,

    // Config
    pub temporal_namespace: String,

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
            date_range_input: String::new(),
            active_date_range_label: None,
            type_name_filter: None,

            table_state,
            visible_columns,

            workflow_sort: None,
            type_sort: None,

            type_stats: LoadState::NotLoaded,
            type_table_state: TableState::default().with_selected(0),

            activity_events: LoadState::NotLoaded,
            activities: Vec::new(),
            child_workflows: Vec::new(),
            activity_table_state: TableState::default().with_selected(0),
            expanded_activity: None,

            event_log_scroll: 0,

            insights: LoadState::NotLoaded,
            insights_table_state: TableState::default().with_selected(0),
            insight_detail_scroll: 0,
            insight_entity_index: 0,

            temporal_namespace: String::new(),

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

        // Reset PendingG mode on any action dispatched from it (except EnterPendingG itself)
        if self.input_mode == InputMode::PendingG
            && !matches!(action, Action::EnterPendingG | Action::Tick)
        {
            self.input_mode = InputMode::Normal;
        }

        match action {
            Action::NavigateUp => {
                if let Some(scroll) = self.current_scroll_mut() {
                    *scroll = scroll.saturating_sub(1);
                } else {
                    self.select_previous();
                }
                vec![]
            }
            Action::NavigateDown => {
                if let Some(scroll) = self.current_scroll_mut() {
                    *scroll = scroll.saturating_add(1);
                } else {
                    self.select_next();
                }
                vec![]
            }
            Action::NavigateTop => {
                if let Some(scroll) = self.current_scroll_mut() {
                    *scroll = 0;
                } else {
                    self.select_first();
                }
                vec![]
            }
            Action::NavigateBottom => {
                if let Some(scroll) = self.current_scroll_mut() {
                    *scroll = u16::MAX;
                } else {
                    self.select_last();
                }
                vec![]
            }
            Action::PageUp => {
                let ps = self.page_size as u16;
                if let Some(scroll) = self.current_scroll_mut() {
                    *scroll = scroll.saturating_sub(ps);
                } else {
                    for _ in 0..self.page_size {
                        self.select_previous();
                    }
                }
                vec![]
            }
            Action::PageDown => {
                let ps = self.page_size as u16;
                if let Some(scroll) = self.current_scroll_mut() {
                    *scroll = scroll.saturating_add(ps);
                } else {
                    for _ in 0..self.page_size {
                        self.select_next();
                    }
                }
                vec![]
            }
            Action::ViewDetail => {
                match self.view {
                    View::TypeList => {
                        // Select type → filter WorkflowList by that type
                        if let Some(type_name) = self.selected_type_name() {
                            let name = type_name.to_string();
                            // Pop back to wherever we came from (should be WorkflowList)
                            if let Some(prev_view) = self.view_stack.pop() {
                                self.view = prev_view;
                            } else {
                                self.view = View::WorkflowList;
                            }
                            self.filter.workflow_type = Some(name);
                            self.table_state.select(Some(0));
                            vec![Effect::LoadWorkflows]
                        } else {
                            vec![]
                        }
                    }
                    View::InsightDetail => {
                        // Drill down from insight detail into a workflow
                        if let LoadState::Loaded(ref result) = self.insights {
                            if let Some(selected_finding) = self
                                .insights_table_state
                                .selected()
                                .and_then(|i| result.findings.get(i))
                            {
                                if let Some(entity) = selected_finding
                                    .affected_entities
                                    .get(self.insight_entity_index)
                                {
                                    let id = entity.clone();
                                    self.view_stack.push(self.view);
                                    self.view = View::WorkflowDetail;
                                    self.selected_workflow = Some(LoadState::Loading);
                                    return vec![Effect::LoadWorkflowDetail(id, None)];
                                }
                            }
                        }
                        vec![]
                    }
                    _ => {
                        if let Some(wf_id) = self.selected_workflow_id() {
                            let id = wf_id.to_string();
                            let run_id =
                                self.selected_workflow_run_id().map(|s| s.to_string());
                            self.view_stack.push(self.view);
                            self.view = View::WorkflowDetail;
                            self.selected_workflow = Some(LoadState::Loading);
                            vec![Effect::LoadWorkflowDetail(id, run_id)]
                        } else {
                            vec![]
                        }
                    }
                }
            }
            Action::GoBack => {
                if self.input_mode == InputMode::PendingG {
                    self.input_mode = InputMode::Normal;
                } else if self.input_mode == InputMode::DateRangeSelect {
                    self.input_mode = InputMode::Normal;
                } else if self.input_mode == InputMode::DateRangeCustom {
                    self.input_mode = InputMode::Normal;
                    self.date_range_input.clear();
                } else if self.input_mode == InputMode::SortSelect {
                    self.input_mode = InputMode::Normal;
                } else if self.input_mode == InputMode::FilterInput {
                    self.input_mode = InputMode::Normal;
                    self.filter_input.clear();
                } else if let Some(prev_view) = self.view_stack.pop() {
                    if self.view == View::ActivityList {
                        self.activity_events = LoadState::NotLoaded;
                        self.activities.clear();
                        self.child_workflows.clear();
                        self.expanded_activity = None;
                    } else if self.view == View::EventLog {
                        self.event_log_scroll = 0;
                        // Keep activity_events cached for quick re-entry
                    } else if self.view == View::InsightDetail {
                        self.insight_detail_scroll = 0;
                    } else if self.view == View::Insights {
                        // Keep cached insights data for quick re-entry
                    } else {
                        self.selected_workflow = None;
                    }
                    self.view = prev_view;
                } else if self.view == View::WorkflowDetail {
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
                self.active_date_range_label = None;
                self.type_name_filter = None;
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
                if self.view == View::TypeList {
                    // In TypeList, apply as client-side name filter
                    if self.filter_input.is_empty() {
                        self.type_name_filter = None;
                    } else {
                        self.type_name_filter = Some(self.filter_input.clone());
                    }
                    self.type_table_state.select(Some(0));
                    self.filter_input.clear();
                    vec![]
                } else {
                    // Apply filter from input as Temporal query
                    if !self.filter_input.is_empty() {
                        self.filter = WorkflowFilter::from_query(&self.filter_input);
                        self.filter_input.clear();
                        vec![Effect::LoadWorkflows]
                    } else {
                        vec![]
                    }
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
            Action::EnterPendingG => {
                self.input_mode = InputMode::PendingG;
                vec![]
            }
            Action::CancelPendingG => {
                self.input_mode = InputMode::Normal;
                vec![]
            }
            Action::CopyWorkflowUrl => {
                if let Some(url) = self.workflow_url() {
                    vec![Effect::CopyToClipboard(url)]
                } else {
                    self.last_error = Some("No workflow loaded".to_string());
                    vec![]
                }
            }
            Action::OpenWorkflowUrl => {
                if let Some(url) = self.workflow_url() {
                    vec![Effect::OpenInBrowser(url)]
                } else {
                    self.last_error = Some("No workflow loaded".to_string());
                    vec![]
                }
            }
            Action::Refresh => {
                self.last_refresh = Some(Instant::now());
                if self.view == View::ActivityList {
                    // In activity view, reload history for the current workflow
                    if let Some(LoadState::Loaded(ref detail)) = self.selected_workflow {
                        let wf_id = detail.summary.workflow_id.clone();
                        let run_id = Some(detail.summary.run_id.clone());
                        self.activity_events = LoadState::Loading;
                        self.activities.clear();
                        self.child_workflows.clear();
                        self.expanded_activity = None;
                        return vec![Effect::LoadHistory(wf_id, run_id)];
                    }
                    return vec![];
                }
                if self.view == View::EventLog {
                    // Re-load history for event log
                    if let Some(LoadState::Loaded(ref detail)) = self.selected_workflow {
                        let wf_id = detail.summary.workflow_id.clone();
                        let run_id = Some(detail.summary.run_id.clone());
                        self.activity_events = LoadState::Loading;
                        self.event_log_scroll = 0;
                        return vec![Effect::LoadHistory(wf_id, run_id)];
                    }
                    return vec![];
                }
                if self.view == View::Insights || self.view == View::InsightDetail {
                    // Re-run the insights scan
                    self.insights = LoadState::Loading;
                    self.insights_table_state.select(Some(0));
                    self.insight_detail_scroll = 0;
                    // If in detail view, go back to list
                    if self.view == View::InsightDetail {
                        if let Some(prev) = self.view_stack.pop() {
                            self.view = prev;
                        } else {
                            self.view = View::Insights;
                        }
                    }
                    return vec![self.load_insights_effect()];
                }
                let mut effects = vec![Effect::LoadCounts, Effect::LoadWorkflows];
                self.status_counts = LoadState::Loading;
                self.workflows = LoadState::Loading;
                if self.view == View::TypeList {
                    self.type_stats = LoadState::Loading;
                    effects.push(Effect::LoadTypeStats);
                }
                effects
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
            Action::ViewActivities => {
                // Get workflow_id and run_id from the currently loaded detail
                if let Some(LoadState::Loaded(ref detail)) = self.selected_workflow {
                    let wf_id = detail.summary.workflow_id.clone();
                    let run_id = Some(detail.summary.run_id.clone());
                    self.view_stack.push(self.view);
                    self.view = View::ActivityList;
                    self.activity_events = LoadState::Loading;
                    self.activities.clear();
                    self.child_workflows.clear();
                    self.activity_table_state.select(Some(0));
                    self.expanded_activity = None;
                    vec![Effect::LoadHistory(wf_id, run_id)]
                } else {
                    self.last_error = Some("No workflow loaded".to_string());
                    vec![]
                }
            }
            Action::ToggleActivityDetail => {
                let combined_len = self.activities.len() + self.child_workflows.len();
                if let Some(selected) = self.activity_table_state.selected() {
                    if selected < combined_len {
                        if self.expanded_activity == Some(selected) {
                            self.expanded_activity = None;
                        } else {
                            self.expanded_activity = Some(selected);
                        }
                    }
                }
                vec![]
            }
            Action::ViewInsights => {
                self.view_stack.push(self.view);
                self.view = View::Insights;
                self.insights = LoadState::Loading;
                self.insights_table_state.select(Some(0));
                vec![self.load_insights_effect()]
            }
            Action::ViewInsightDetail => {
                if let LoadState::Loaded(ref result) = self.insights {
                    if let Some(selected) = self.insights_table_state.selected() {
                        if selected < result.findings.len() {
                            self.view_stack.push(self.view);
                            self.view = View::InsightDetail;
                            self.insight_detail_scroll = 0;
                            self.insight_entity_index = 0;
                        }
                    }
                }
                vec![]
            }
            Action::NextAffectedEntity => {
                if let LoadState::Loaded(ref result) = self.insights {
                    if let Some(finding) = self
                        .insights_table_state
                        .selected()
                        .and_then(|i| result.findings.get(i))
                    {
                        let len = finding.affected_entities.len();
                        if len > 0 {
                            self.insight_entity_index =
                                (self.insight_entity_index + 1) % len;
                        }
                    }
                }
                vec![]
            }
            Action::PrevAffectedEntity => {
                if let LoadState::Loaded(ref result) = self.insights {
                    if let Some(finding) = self
                        .insights_table_state
                        .selected()
                        .and_then(|i| result.findings.get(i))
                    {
                        let len = finding.affected_entities.len();
                        if len > 0 {
                            self.insight_entity_index = if self.insight_entity_index == 0 {
                                len - 1
                            } else {
                                self.insight_entity_index - 1
                            };
                        }
                    }
                }
                vec![]
            }
            Action::ViewEventLog => {
                // Enter event log view reusing already-loaded history or loading fresh
                self.view_stack.push(self.view);
                self.view = View::EventLog;
                self.event_log_scroll = 0;
                if self.activity_events.is_loaded() {
                    vec![]
                } else if let Some(LoadState::Loaded(ref detail)) = self.selected_workflow {
                    let wf_id = detail.summary.workflow_id.clone();
                    let run_id = Some(detail.summary.run_id.clone());
                    self.activity_events = LoadState::Loading;
                    vec![Effect::LoadHistory(wf_id, run_id)]
                } else {
                    self.last_error = Some("No workflow loaded".to_string());
                    vec![]
                }
            }
            Action::ViewTypeList => {
                self.view_stack.push(self.view);
                self.view = View::TypeList;
                self.type_stats = LoadState::Loading;
                self.type_table_state.select(Some(0));
                vec![Effect::LoadTypeStats]
            }
            Action::EnterSortMode => {
                self.input_mode = InputMode::SortSelect;
                vec![]
            }
            Action::CloseSort => {
                self.input_mode = InputMode::Normal;
                vec![]
            }
            Action::EnterDateRangeMode => {
                self.input_mode = InputMode::DateRangeSelect;
                vec![]
            }
            Action::SelectDateRangePreset(preset) => {
                self.filter.start_time_after = Some(chrono::Utc::now() - preset.duration());
                self.filter.start_time_before = None;
                self.active_date_range_label = Some(format!("{} ago", preset.short_label()));
                self.input_mode = InputMode::Normal;
                self.table_state.select(Some(0));
                self.date_range_reload_effects()
            }
            Action::ClearDateRange => {
                self.filter.start_time_after = None;
                self.filter.start_time_before = None;
                self.filter.close_time_after = None;
                self.filter.close_time_before = None;
                self.active_date_range_label = None;
                self.input_mode = InputMode::Normal;
                self.table_state.select(Some(0));
                self.date_range_reload_effects()
            }
            Action::EnterCustomDateInput => {
                self.input_mode = InputMode::DateRangeCustom;
                self.date_range_input.clear();
                vec![]
            }
            Action::CloseDateRangeMode => {
                self.input_mode = InputMode::Normal;
                vec![]
            }
            Action::AppendDateRangeChar(c) => {
                self.date_range_input.push(c);
                vec![]
            }
            Action::DeleteDateRangeChar => {
                self.date_range_input.pop();
                vec![]
            }
            Action::ApplyCustomDateRange => {
                let input = self.date_range_input.clone();
                if let Some(dt) = parse_date_input(&input) {
                    self.filter.start_time_after = Some(dt);
                    self.filter.start_time_before = None;
                    self.active_date_range_label = Some(format!("{} ago", input.trim()));
                    self.input_mode = InputMode::Normal;
                    self.date_range_input.clear();
                    self.table_state.select(Some(0));
                    self.date_range_reload_effects()
                } else {
                    self.last_error = Some(format!("Invalid date input: '{}'. Use e.g. 2h, 3d, 1w, or 2024-01-15", input));
                    self.input_mode = InputMode::Normal;
                    self.date_range_input.clear();
                    vec![]
                }
            }
            Action::CancelCustomDateRange => {
                self.input_mode = InputMode::Normal;
                self.date_range_input.clear();
                vec![]
            }
            Action::SortBy(key) => {
                self.input_mode = InputMode::Normal;
                match self.view {
                    View::WorkflowList => {
                        let column = match key {
                            b's' => Some(TableColumn::Status),
                            b't' => Some(TableColumn::Type),
                            b'w' => Some(TableColumn::WorkflowId),
                            b'd' => Some(TableColumn::Started),
                            _ => None,
                        };
                        if let Some(col) = column {
                            let direction = if let Some((ref current_col, ref dir)) =
                                self.workflow_sort
                            {
                                if *current_col == col {
                                    dir.toggle()
                                } else {
                                    SortDirection::Ascending
                                }
                            } else {
                                SortDirection::Ascending
                            };
                            self.workflow_sort = Some((col, direction));
                            self.sort_workflows();
                        }
                    }
                    View::TypeList => {
                        let column = match key {
                            b't' => Some(TypeListColumn::TypeName),
                            b'n' => Some(TypeListColumn::Total),
                            b'1' => Some(TypeListColumn::StatusCount(WorkflowStatus::Running)),
                            b'2' => Some(TypeListColumn::StatusCount(WorkflowStatus::Completed)),
                            b'3' => Some(TypeListColumn::StatusCount(WorkflowStatus::Failed)),
                            b'4' => Some(TypeListColumn::StatusCount(WorkflowStatus::Canceled)),
                            b'5' => Some(TypeListColumn::StatusCount(WorkflowStatus::Terminated)),
                            b'6' => Some(TypeListColumn::StatusCount(WorkflowStatus::TimedOut)),
                            b'7' => {
                                Some(TypeListColumn::StatusCount(WorkflowStatus::ContinuedAsNew))
                            }
                            _ => None,
                        };
                        if let Some(col) = column {
                            let direction =
                                if let Some((ref current_col, ref dir)) = self.type_sort {
                                    if *current_col == col {
                                        dir.toggle()
                                    } else {
                                        SortDirection::Descending
                                    }
                                } else {
                                    SortDirection::Descending
                                };
                            self.type_sort = Some((col, direction));
                            self.sort_type_stats();
                        }
                    }
                    _ => {}
                }
                vec![]
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
                        self.sort_workflows();
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
                    DataPayload::TypeStats(stats) => {
                        self.type_stats = LoadState::Loaded(stats);
                        self.sort_type_stats();
                        // Reset selection if out of bounds
                        if let LoadState::Loaded(ref ts) = self.type_stats {
                            let selected = self.type_table_state.selected().unwrap_or(0);
                            if selected >= ts.len() && !ts.is_empty() {
                                self.type_table_state.select(Some(ts.len() - 1));
                            }
                        }
                    }
                    DataPayload::History(events) => {
                        self.activities = correlate_activities(&events);
                        self.child_workflows = correlate_child_workflows(&events);
                        self.activity_events = LoadState::Loaded(events);
                        // Reset selection if out of bounds (combined length for timeline)
                        let combined_len =
                            self.activities.len() + self.child_workflows.len();
                        let selected = self.activity_table_state.selected().unwrap_or(0);
                        if selected >= combined_len && combined_len > 0 {
                            self.activity_table_state
                                .select(Some(combined_len - 1));
                        }
                    }
                    DataPayload::Insights(result) => {
                        self.insights = LoadState::Loaded(result);
                        // Reset selection if out of bounds
                        if let LoadState::Loaded(ref r) = self.insights {
                            let selected = self.insights_table_state.selected().unwrap_or(0);
                            if selected >= r.findings.len() && !r.findings.is_empty() {
                                self.insights_table_state
                                    .select(Some(r.findings.len() - 1));
                            }
                        }
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
                    self.selected_workflow = Some(LoadState::Error(msg.clone()));
                }
                if self.activity_events.is_loading() {
                    self.activity_events = LoadState::Error(msg.clone());
                }
                if self.insights.is_loading() {
                    self.insights = LoadState::Error(msg);
                }
                vec![]
            }
        }
    }

    /// Build a Temporal Cloud URL for the currently viewed workflow
    pub fn workflow_url(&self) -> Option<String> {
        if let Some(LoadState::Loaded(ref detail)) = self.selected_workflow {
            Some(format!(
                "https://cloud.temporal.io/namespaces/{}/workflows/{}/{}/history",
                self.temporal_namespace,
                detail.summary.workflow_id,
                detail.summary.run_id,
            ))
        } else {
            None
        }
    }

    /// Build a LoadInsights effect using the current filter state
    fn load_insights_effect(&self) -> Effect {
        let limit = if self.filter.has_date_range() {
            1000
        } else {
            500
        };
        Effect::LoadInsights {
            filter: self.filter.clone(),
            limit,
        }
    }

    /// Return the appropriate reload effects based on the current view
    fn date_range_reload_effects(&self) -> Vec<Effect> {
        if self.view == View::TypeList {
            vec![Effect::LoadTypeStats]
        } else {
            vec![Effect::LoadWorkflows]
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
        match self.view {
            View::TypeList => &mut self.type_table_state,
            View::ActivityList => &mut self.activity_table_state,
            View::Insights | View::InsightDetail => &mut self.insights_table_state,
            _ => &mut self.table_state,
        }
    }

    fn current_scroll_mut(&mut self) -> Option<&mut u16> {
        match self.view {
            View::InsightDetail => Some(&mut self.insight_detail_scroll),
            View::EventLog => Some(&mut self.event_log_scroll),
            _ => None,
        }
    }

    fn current_list_len(&self) -> usize {
        match self.view {
            View::TypeList => {
                if let LoadState::Loaded(ref stats) = self.type_stats {
                    stats.len()
                } else {
                    0
                }
            }
            View::ActivityList => self.activities.len() + self.child_workflows.len(),
            View::InsightDetail | View::EventLog => 0,
            View::Insights => {
                if let LoadState::Loaded(ref result) = self.insights {
                    result.findings.len()
                } else {
                    0
                }
            }
            _ => {
                if let LoadState::Loaded(ref workflows) = self.workflows {
                    workflows.len()
                } else {
                    0
                }
            }
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

    pub fn selected_type_name(&self) -> Option<&str> {
        if let LoadState::Loaded(ref stats) = self.type_stats {
            let selected = self.type_table_state.selected().unwrap_or(0);
            stats.get(selected).map(|ts| ts.workflow_type.as_str())
        } else {
            None
        }
    }

    fn sort_workflows(&mut self) {
        if let (Some((col, dir)), LoadState::Loaded(ref mut workflows)) =
            (&self.workflow_sort, &mut self.workflows)
        {
            let dir = *dir;
            match col {
                TableColumn::Status => workflows.sort_by(|a, b| {
                    let cmp = a.status.short_name().cmp(&b.status.short_name());
                    if dir == SortDirection::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                }),
                TableColumn::Type => workflows.sort_by(|a, b| {
                    let cmp = a.workflow_type.cmp(&b.workflow_type);
                    if dir == SortDirection::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                }),
                TableColumn::WorkflowId => workflows.sort_by(|a, b| {
                    let cmp = a.workflow_id.cmp(&b.workflow_id);
                    if dir == SortDirection::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                }),
                TableColumn::Started => workflows.sort_by(|a, b| {
                    let cmp = a.start_time.cmp(&b.start_time);
                    if dir == SortDirection::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                }),
            }
        }
    }

    fn sort_type_stats(&mut self) {
        if let (Some((col, dir)), LoadState::Loaded(ref mut stats)) =
            (&self.type_sort, &mut self.type_stats)
        {
            let dir = *dir;
            match col {
                TypeListColumn::TypeName => stats.sort_by(|a, b| {
                    let cmp = a.workflow_type.cmp(&b.workflow_type);
                    if dir == SortDirection::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                }),
                TypeListColumn::Total => stats.sort_by(|a, b| {
                    let cmp = a.total.cmp(&b.total);
                    if dir == SortDirection::Descending {
                        cmp.reverse()
                    } else {
                        cmp
                    }
                }),
                TypeListColumn::StatusCount(status) => {
                    let status = *status;
                    stats.sort_by(|a, b| {
                        let cmp = a.get_status_count(status).cmp(&b.get_status_count(status));
                        if dir == SortDirection::Descending {
                            cmp.reverse()
                        } else {
                            cmp
                        }
                    });
                }
            }
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
    LoadWorkflowDetail(String, Option<String>),
    LoadTypeStats,
    LoadHistory(String, Option<String>),
    LoadInsights {
        filter: WorkflowFilter,
        limit: u32,
    },
    CancelWorkflow(String),
    TerminateWorkflow(String),
    CopyToClipboard(String),
    OpenInBrowser(String),
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
