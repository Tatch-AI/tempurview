use crate::domain::{
    DateRangePreset, HistoryEvent, InsightsResult, InsightsScanPhase, StatusCounts, TypeStat,
    WorkflowDetail, WorkflowStatus, WorkflowSummary,
};
use std::collections::HashSet;

/// Table columns that can be toggled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableColumn {
    Status,
    Type,
    WorkflowId,
    Started,
}

impl TableColumn {
    pub fn all() -> &'static [TableColumn] {
        &[
            TableColumn::Status,
            TableColumn::Type,
            TableColumn::WorkflowId,
            TableColumn::Started,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            TableColumn::Status => "Status",
            TableColumn::Type => "Type",
            TableColumn::WorkflowId => "Workflow ID",
            TableColumn::Started => "Started",
        }
    }
}

/// All possible user actions
#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    NavigateUp,
    NavigateDown,
    NavigateTop,
    NavigateBottom,
    PageUp,
    PageDown,

    // View switching
    ViewDetail,
    ViewTypeList,
    ViewActivities,
    GoBack,

    // Activity view
    ToggleActivityDetail,
    ViewActivityDetail,

    // Insights view
    ViewInsights,
    ViewInsightDetail,
    NextAffectedEntity,
    PrevAffectedEntity,

    // Event log view
    ViewEventLog,
    ViewEventDetail,

    // Search (in detail views)
    OpenSearchInput,
    CloseSearchInput,
    AppendSearchChar(char),
    DeleteSearchChar,
    NextSearchMatch,
    PrevSearchMatch,
    ClearSearch,

    // Filtering
    SetStatusFilter(Option<WorkflowStatus>),
    SetTypeFilter(Option<String>),
    SetActivityFailFilter,
    NextStatusFilter,
    PrevStatusFilter,
    ClearFilters,
    OpenFilterInput,
    CloseFilterInput,
    AppendFilterChar(char),
    DeleteFilterChar,

    // Column visibility
    ToggleColumn(TableColumn),

    // Sorting / Picker
    EnterSortMode,
    SortBy(u8),
    ClosePicker,
    PickerUp,
    PickerDown,
    PickerSelect,

    // Date range
    EnterDateRangeMode,
    SelectDateRangePreset(DateRangePreset),
    ClearDateRange,
    EnterCustomDateInput,
    CloseDateRangeMode,
    AppendDateRangeChar(char),
    DeleteDateRangeChar,
    ApplyCustomDateRange,
    CancelCustomDateRange,

    // URL actions
    CopyWorkflowUrl,
    OpenWorkflowUrl,

    // Multi-key chord
    EnterPendingG,
    CancelPendingG,

    // Data operations
    Refresh,
    CancelWorkflow(String),
    TerminateWorkflow(String),

    // App control
    Quit,
    ToggleHelp,

    // Internal
    Tick,
    DataLoaded(DataPayload),
    Error(String),
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Action::NavigateUp, Action::NavigateUp) => true,
            (Action::NavigateDown, Action::NavigateDown) => true,
            (Action::NavigateTop, Action::NavigateTop) => true,
            (Action::NavigateBottom, Action::NavigateBottom) => true,
            (Action::PageUp, Action::PageUp) => true,
            (Action::PageDown, Action::PageDown) => true,
            (Action::ViewDetail, Action::ViewDetail) => true,
            (Action::ViewTypeList, Action::ViewTypeList) => true,
            (Action::ViewActivities, Action::ViewActivities) => true,
            (Action::GoBack, Action::GoBack) => true,
            (Action::ToggleActivityDetail, Action::ToggleActivityDetail) => true,
            (Action::ViewActivityDetail, Action::ViewActivityDetail) => true,
            (Action::ViewInsights, Action::ViewInsights) => true,
            (Action::ViewInsightDetail, Action::ViewInsightDetail) => true,
            (Action::NextAffectedEntity, Action::NextAffectedEntity) => true,
            (Action::PrevAffectedEntity, Action::PrevAffectedEntity) => true,
            (Action::ViewEventLog, Action::ViewEventLog) => true,
            (Action::ViewEventDetail, Action::ViewEventDetail) => true,
            (Action::OpenSearchInput, Action::OpenSearchInput) => true,
            (Action::CloseSearchInput, Action::CloseSearchInput) => true,
            (Action::AppendSearchChar(a), Action::AppendSearchChar(b)) => a == b,
            (Action::DeleteSearchChar, Action::DeleteSearchChar) => true,
            (Action::NextSearchMatch, Action::NextSearchMatch) => true,
            (Action::PrevSearchMatch, Action::PrevSearchMatch) => true,
            (Action::ClearSearch, Action::ClearSearch) => true,
            (Action::SetStatusFilter(a), Action::SetStatusFilter(b)) => a == b,
            (Action::SetTypeFilter(a), Action::SetTypeFilter(b)) => a == b,
            (Action::SetActivityFailFilter, Action::SetActivityFailFilter) => true,
            (Action::NextStatusFilter, Action::NextStatusFilter) => true,
            (Action::PrevStatusFilter, Action::PrevStatusFilter) => true,
            (Action::ClearFilters, Action::ClearFilters) => true,
            (Action::ToggleColumn(a), Action::ToggleColumn(b)) => a == b,
            (Action::EnterSortMode, Action::EnterSortMode) => true,
            (Action::SortBy(a), Action::SortBy(b)) => a == b,
            (Action::ClosePicker, Action::ClosePicker) => true,
            (Action::PickerUp, Action::PickerUp) => true,
            (Action::PickerDown, Action::PickerDown) => true,
            (Action::PickerSelect, Action::PickerSelect) => true,
            (Action::OpenFilterInput, Action::OpenFilterInput) => true,
            (Action::CloseFilterInput, Action::CloseFilterInput) => true,
            (Action::AppendFilterChar(a), Action::AppendFilterChar(b)) => a == b,
            (Action::DeleteFilterChar, Action::DeleteFilterChar) => true,
            (Action::EnterDateRangeMode, Action::EnterDateRangeMode) => true,
            (Action::SelectDateRangePreset(a), Action::SelectDateRangePreset(b)) => a == b,
            (Action::ClearDateRange, Action::ClearDateRange) => true,
            (Action::EnterCustomDateInput, Action::EnterCustomDateInput) => true,
            (Action::CloseDateRangeMode, Action::CloseDateRangeMode) => true,
            (Action::AppendDateRangeChar(a), Action::AppendDateRangeChar(b)) => a == b,
            (Action::DeleteDateRangeChar, Action::DeleteDateRangeChar) => true,
            (Action::ApplyCustomDateRange, Action::ApplyCustomDateRange) => true,
            (Action::CancelCustomDateRange, Action::CancelCustomDateRange) => true,
            (Action::CopyWorkflowUrl, Action::CopyWorkflowUrl) => true,
            (Action::OpenWorkflowUrl, Action::OpenWorkflowUrl) => true,
            (Action::EnterPendingG, Action::EnterPendingG) => true,
            (Action::CancelPendingG, Action::CancelPendingG) => true,
            (Action::Refresh, Action::Refresh) => true,
            (Action::CancelWorkflow(a), Action::CancelWorkflow(b)) => a == b,
            (Action::TerminateWorkflow(a), Action::TerminateWorkflow(b)) => a == b,
            (Action::Quit, Action::Quit) => true,
            (Action::ToggleHelp, Action::ToggleHelp) => true,
            (Action::Tick, Action::Tick) => true,
            (Action::Error(a), Action::Error(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Action {}

/// Payload for loaded data
#[derive(Debug, Clone)]
pub enum DataPayload {
    Counts(StatusCounts),
    Workflows(Vec<WorkflowSummary>, u64),
    WorkflowsPage(Vec<WorkflowSummary>, u64),
    WorkflowsDone(u64),
    Detail(Box<WorkflowDetail>),
    TypeStats(Vec<TypeStat>),
    History(Vec<HistoryEvent>),
    Insights(InsightsResult, u64),
    InsightsPartial(InsightsResult, u64),
    InsightsProgress(InsightsScanPhase, u64),
    ActivityFailPartial(HashSet<String>, u64),
    ActivityFailIds(HashSet<String>, u64),
}
