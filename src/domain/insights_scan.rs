use crate::client::{ClientResult, TemporalClient};
use crate::domain::{
    compute_activity_findings, compute_child_workflow_findings, compute_list_findings,
    correlate_activities, correlate_child_workflows, rank_findings, select_workflows_for_sampling,
    InsightThresholds, InsightsConfig, InsightsResult, WorkflowFilter,
};
use chrono::Utc;
use tracing::{debug, info};

/// Run a full insights scan: list workflows, compute list-level findings,
/// sample histories, compute activity-level findings.
///
/// Shared between the TUI (via CliWorker) and CLI commands.
pub async fn run_insights_scan(
    client: &dyn TemporalClient,
    filter: &WorkflowFilter,
    limit: u32,
    config: &InsightsConfig,
) -> ClientResult<InsightsResult> {
    info!(
        "Starting insights scan (limit={}, has_date_range={})",
        limit,
        filter.has_date_range()
    );
    let scan_start = Utc::now();

    // Build a date-only filter (ignore status/type/id filters)
    let date_filter = WorkflowFilter {
        start_time_after: filter.start_time_after,
        start_time_before: filter.start_time_before,
        close_time_after: filter.close_time_after,
        close_time_before: filter.close_time_before,
        ..WorkflowFilter::new()
    };

    // Step 1: Fetch workflows
    let workflows = client.list(&date_filter, limit).await?;
    let workflows_scanned = workflows.len();
    info!("Insights - fetched {} workflows", workflows_scanned);

    // Step 2: Compute list-level findings
    let mut all_findings = compute_list_findings(&workflows);

    // Step 3: Select workflows for history sampling
    let samples_to_fetch =
        select_workflows_for_sampling(&workflows, InsightThresholds::MAX_HISTORY_SAMPLES);

    // Step 4: Fetch histories and correlate activities + child workflows
    let mut activity_samples: Vec<(String, Vec<crate::domain::ActivityExecution>)> = Vec::new();
    let mut child_wf_samples: Vec<(String, Vec<crate::domain::ChildWorkflowExecution>)> =
        Vec::new();
    for wf in &samples_to_fetch {
        match client
            .get_history(&wf.workflow_id, Some(&wf.run_id))
            .await
        {
            Ok(events) => {
                let activities = correlate_activities(&events);
                activity_samples.push((wf.workflow_id.clone(), activities));

                let child_workflows = correlate_child_workflows(&events);
                if !child_workflows.is_empty() {
                    child_wf_samples.push((wf.workflow_id.clone(), child_workflows));
                }
            }
            Err(e) => {
                debug!(
                    "Failed to fetch history for {}: {} (skipping)",
                    wf.workflow_id, e
                );
            }
        }
    }
    let histories_fetched = activity_samples.len();
    info!(
        "Insights - fetched {} histories ({} with child workflows)",
        histories_fetched,
        child_wf_samples.len()
    );

    // Step 5: Compute activity-level findings
    let activity_findings = compute_activity_findings(&activity_samples, config);
    all_findings.extend(activity_findings);

    // Step 5b: Compute child workflow findings
    let child_wf_findings = compute_child_workflow_findings(&child_wf_samples);
    all_findings.extend(child_wf_findings);

    // Step 6: Rank all findings
    let findings = rank_findings(all_findings);

    let scan_end = Utc::now();
    let scan_duration = scan_end - scan_start;

    let result = InsightsResult {
        findings,
        workflows_scanned,
        histories_fetched,
        computed_at: scan_end,
        scan_duration,
    };

    info!(
        "Insights scan complete - {} findings, {:.1}s",
        result.findings.len(),
        scan_duration.num_milliseconds() as f64 / 1000.0
    );

    Ok(result)
}
