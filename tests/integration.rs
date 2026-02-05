//! Integration tests for Tempurview
//!
//! These tests verify that the various components work together correctly.
//! They use the mock client and don't require external services.

mod common;

use tempurview::action::{Action, DataPayload};
use tempurview::app::{App, LoadState, View};
use tempurview::client::{MockTemporalClient, TemporalClient};
use tempurview::domain::{StatusCounts, WorkflowFilter, WorkflowStatus};

// ============================================================================
// Mock Client Integration Tests
// ============================================================================

#[tokio::test]
async fn test_mock_client_count_all() {
    let client = MockTemporalClient::with_random_data(100);

    let count = client.count(None).await.unwrap();
    assert_eq!(count, 100);
}

#[tokio::test]
async fn test_mock_client_count_by_status() {
    let client = MockTemporalClient::with_random_data(100);

    let mut total = 0u64;
    for status in WorkflowStatus::all() {
        let query = format!("ExecutionStatus='{}'", status.as_query_value());
        let count = client.count(Some(&query)).await.unwrap();
        total += count;
    }

    // Total of all statuses should equal total workflows
    assert_eq!(total, 100);
}

#[tokio::test]
async fn test_mock_client_list_respects_limit() {
    let client = MockTemporalClient::with_random_data(100);
    let filter = WorkflowFilter::new();

    let workflows = client.list(&filter, 10).await.unwrap();
    assert_eq!(workflows.len(), 10);

    let workflows = client.list(&filter, 50).await.unwrap();
    assert_eq!(workflows.len(), 50);
}

#[tokio::test]
async fn test_mock_client_list_with_status_filter() {
    let client = MockTemporalClient::with_random_data(100);

    let mut filter = WorkflowFilter::new();
    filter.status = Some(WorkflowStatus::Running);

    let workflows = client.list(&filter, 100).await.unwrap();

    // All returned workflows should be running
    for wf in &workflows {
        assert_eq!(wf.status, WorkflowStatus::Running);
    }
}

#[tokio::test]
async fn test_mock_client_describe() {
    let client = MockTemporalClient::with_random_data(10);

    // Get a workflow ID from the list
    let filter = WorkflowFilter::new();
    let workflows = client.list(&filter, 1).await.unwrap();
    let workflow_id = &workflows[0].workflow_id;

    // Describe should return details
    let detail = client.describe(workflow_id, None).await.unwrap();
    assert_eq!(detail.summary.workflow_id, *workflow_id);
}

#[tokio::test]
async fn test_mock_client_describe_not_found() {
    let client = MockTemporalClient::with_random_data(10);

    let result = client.describe("non-existent-workflow", None).await;
    assert!(result.is_err());
}

// ============================================================================
// App State Integration Tests
// ============================================================================

#[test]
fn test_app_initial_state() {
    let app = App::new();

    assert_eq!(app.view, View::WorkflowList);
    assert!(matches!(app.status_counts, LoadState::NotLoaded));
    assert!(matches!(app.workflows, LoadState::NotLoaded));
    assert!(!app.should_quit);
}

#[test]
fn test_app_refresh_triggers_loading() {
    let mut app = App::new();

    let effects = app.update(Action::Refresh);

    assert!(matches!(app.status_counts, LoadState::Loading));
    assert!(matches!(app.workflows, LoadState::Loading));
    // LoadCounts queries count for all statuses from API
    assert!(effects
        .iter()
        .any(|e| matches!(e, tempurview::Effect::LoadCounts)));
    assert!(effects
        .iter()
        .any(|e| matches!(e, tempurview::Effect::LoadWorkflows)));
}

#[test]
fn test_app_data_loaded_updates_state() {
    let mut app = App::new();
    app.status_counts = LoadState::Loading;

    let mut counts = StatusCounts::new();
    counts.set(WorkflowStatus::Running, 42);

    app.update(Action::DataLoaded(DataPayload::Counts(counts)));

    match &app.status_counts {
        LoadState::Loaded(c) => {
            assert_eq!(c.get(WorkflowStatus::Running), 42);
        }
        _ => panic!("Expected Loaded state"),
    }
}

#[test]
fn test_app_navigation_with_workflows() {
    let mut app = App::new();
    app.view = View::WorkflowList;
    app.workflows = LoadState::Loaded(vec![
        common::make_test_workflow(1, WorkflowStatus::Running),
        common::make_test_workflow(2, WorkflowStatus::Completed),
        common::make_test_workflow(3, WorkflowStatus::Failed),
    ]);

    // Start at first item
    assert_eq!(app.table_state.selected(), Some(0));

    // Navigate down
    app.update(Action::NavigateDown);
    assert_eq!(app.table_state.selected(), Some(1));

    // Navigate down again
    app.update(Action::NavigateDown);
    assert_eq!(app.table_state.selected(), Some(2));

    // Navigate down at bottom (should stay at 2)
    app.update(Action::NavigateDown);
    assert_eq!(app.table_state.selected(), Some(2));

    // Navigate up
    app.update(Action::NavigateUp);
    assert_eq!(app.table_state.selected(), Some(1));

    // Jump to bottom
    app.update(Action::NavigateBottom);
    assert_eq!(app.table_state.selected(), Some(2));

    // Jump to top
    app.update(Action::NavigateTop);
    assert_eq!(app.table_state.selected(), Some(0));
}

#[test]
fn test_app_view_navigation() {
    let mut app = App::new();
    app.workflows = LoadState::Loaded(vec![common::make_test_workflow(1, WorkflowStatus::Running)]);

    assert_eq!(app.view, View::WorkflowList);

    // Go to detail
    app.update(Action::ViewDetail);
    assert_eq!(app.view, View::WorkflowDetail);

    // Go back to list
    app.update(Action::GoBack);
    assert_eq!(app.view, View::WorkflowList);
}

#[test]
fn test_app_status_filter_triggers_reload() {
    let mut app = App::new();

    let effects = app.update(Action::SetStatusFilter(Some(WorkflowStatus::Failed)));

    assert_eq!(app.filter.status, Some(WorkflowStatus::Failed));
    assert!(effects
        .iter()
        .any(|e| matches!(e, tempurview::Effect::LoadWorkflows)));
}

#[test]
fn test_app_clear_filters() {
    let mut app = App::new();
    app.filter.status = Some(WorkflowStatus::Failed);
    app.filter.workflow_type = Some("TestWorkflow".to_string());

    let effects = app.update(Action::ClearFilters);

    assert!(app.filter.is_empty());
    assert!(effects
        .iter()
        .any(|e| matches!(e, tempurview::Effect::LoadWorkflows)));
}

#[test]
fn test_app_error_handling() {
    let mut app = App::new();
    app.status_counts = LoadState::Loading;
    app.workflows = LoadState::Loading;

    app.update(Action::Error("Connection failed".to_string()));

    assert_eq!(app.last_error, Some("Connection failed".to_string()));
    assert!(matches!(app.status_counts, LoadState::Error(_)));
    assert!(matches!(app.workflows, LoadState::Error(_)));
}

#[test]
fn test_app_quit() {
    let mut app = App::new();

    assert!(!app.should_quit);

    // First quit shows warning
    app.update(Action::Quit);
    assert!(!app.should_quit);

    // Second quit actually quits
    app.update(Action::Quit);
    assert!(app.should_quit);
}

// ============================================================================
// Full Flow Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_refresh_cycle() {
    let client = MockTemporalClient::with_random_data(50);
    let mut app = App::new();

    // Trigger refresh
    let _effects = app.update(Action::Refresh);
    assert!(matches!(app.status_counts, LoadState::Loading));
    assert!(matches!(app.workflows, LoadState::Loading));

    // Simulate loading counts
    let mut counts = StatusCounts::new();
    for status in WorkflowStatus::all() {
        let query = format!("ExecutionStatus='{}'", status.as_query_value());
        let count = client.count(Some(&query)).await.unwrap();
        counts.set(*status, count);
    }
    app.update(Action::DataLoaded(DataPayload::Counts(counts)));

    // Simulate loading workflows
    let workflows = client.list(&app.filter, 50).await.unwrap();
    app.update(Action::DataLoaded(DataPayload::Workflows(workflows)));

    // Verify state
    assert!(matches!(app.status_counts, LoadState::Loaded(_)));
    assert!(matches!(app.workflows, LoadState::Loaded(_)));

    if let LoadState::Loaded(wfs) = &app.workflows {
        assert_eq!(wfs.len(), 50);
    }
}

#[tokio::test]
async fn test_filter_and_list_workflow() {
    let client = MockTemporalClient::with_random_data(100);
    let mut app = App::new();

    // Set filter for running workflows
    app.update(Action::SetStatusFilter(Some(WorkflowStatus::Running)));

    // Load workflows with filter
    let workflows = client.list(&app.filter, 100).await.unwrap();
    app.update(Action::DataLoaded(DataPayload::Workflows(workflows)));

    // Verify all loaded workflows match filter
    if let LoadState::Loaded(wfs) = &app.workflows {
        for wf in wfs {
            assert_eq!(wf.status, WorkflowStatus::Running);
        }
    }
}
