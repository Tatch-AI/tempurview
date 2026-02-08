use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::domain::{correlate_activities, parse_date_input, run_insights_scan, WorkflowFilter, WorkflowStatus};

use super::AppState;

/// Serve the embedded single-page frontend.
pub async fn index() -> Response {
    let html = include_str!("../../static/index.html");
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

#[derive(Deserialize)]
pub struct WorkflowListParams {
    pub status: Option<String>,
    pub workflow_type: Option<String>,
    pub since: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
}

/// GET /api/workflows — list workflows with optional filters.
pub async fn list_workflows(
    State(state): State<AppState>,
    Query(params): Query<WorkflowListParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut filter = WorkflowFilter::new();

    if let Some(ref s) = params.status {
        let ws: WorkflowStatus = s
            .parse()
            .map_err(|_| (StatusCode::BAD_REQUEST, format!("Invalid status: {s}")))?;
        filter = filter.with_status(ws);
    }
    if let Some(ref t) = params.workflow_type {
        filter = filter.with_type(t.clone());
    }
    if let Some(ref s) = params.since {
        let dt = parse_date_input(s)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Invalid since value: {s}")))?;
        filter = filter.with_start_time_after(dt);
    }
    if let Some(ref b) = params.before {
        let dt = parse_date_input(b)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Invalid before value: {b}")))?;
        filter = filter.with_start_time_before(dt);
    }

    let limit = params.limit.unwrap_or(state.config.default_limit);
    let workflows = state
        .client
        .list(&filter, limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "workflows": workflows, "count": workflows.len() })))
}

/// GET /api/workflows/:id — get workflow detail.
pub async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<RunIdParam>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let detail = state
        .client
        .describe(&id, params.run_id.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::to_value(&detail).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct RunIdParam {
    pub run_id: Option<String>,
}

/// GET /api/workflows/:id/activities — list correlated activities.
pub async fn list_activities(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<RunIdParam>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let events = state
        .client
        .get_history(&id, params.run_id.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let activities = correlate_activities(&events);
    Ok(Json(json!({ "activities": activities, "count": activities.len() })))
}

/// GET /api/workflows/:id/events — list raw history events.
pub async fn list_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<RunIdParam>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let events = state
        .client
        .get_history(&id, params.run_id.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "events": events, "count": events.len() })))
}

#[derive(Deserialize)]
pub struct InsightsParams {
    pub since: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u32>,
}

/// GET /api/insights — run an insights scan.
pub async fn get_insights(
    State(state): State<AppState>,
    Query(params): Query<InsightsParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut filter = WorkflowFilter::new();

    if let Some(ref s) = params.since {
        let dt = parse_date_input(s)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Invalid since value: {s}")))?;
        filter = filter.with_start_time_after(dt);
    }
    if let Some(ref b) = params.before {
        let dt = parse_date_input(b)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Invalid before value: {b}")))?;
        filter = filter.with_start_time_before(dt);
    }

    let limit = params.limit.unwrap_or(state.config.default_limit);
    let result = run_insights_scan(
        state.client.clone(),
        &filter,
        limit,
        &state.config.insights,
        None,
        0,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::to_value(&result).unwrap_or_default()))
}

/// POST /api/workflows/:id/cancel — stub (501 Not Implemented).
pub async fn cancel_workflow(Path(_id): Path<String>) -> (StatusCode, String) {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Cancel not yet implemented in web UI".to_string(),
    )
}

/// POST /api/workflows/:id/terminate — stub (501 Not Implemented).
pub async fn terminate_workflow(Path(_id): Path<String>) -> (StatusCode, String) {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Terminate not yet implemented in web UI".to_string(),
    )
}
