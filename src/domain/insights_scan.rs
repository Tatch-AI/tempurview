use crate::action::{Action, DataPayload};
use crate::client::{ClientError, ClientResult, TemporalClient};
use crate::domain::{
    compute_activity_findings, compute_child_workflow_findings, compute_list_findings,
    correlate_activities, correlate_child_workflows,
    finalize_decision_latency_findings, finalize_scheduling_overhead_findings,
    finalize_signal_findings, rank_findings, select_workflows_for_sampling, InsightsConfig,
    InsightsResult, InsightsScanPhase, WorkflowEventStats, WorkflowFilter,
};
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Run a full insights scan: list workflows, compute list-level findings,
/// sample histories concurrently, compute activity-level findings.
///
/// Shared between the TUI (via CliWorker) and CLI commands.
pub async fn run_insights_scan(
    client: Arc<dyn TemporalClient>,
    filter: &WorkflowFilter,
    limit: u32,
    config: &InsightsConfig,
    progress_tx: Option<mpsc::UnboundedSender<Action>>,
    scan_gen: u64,
) -> ClientResult<InsightsResult> {
    info!(
        "Starting insights scan (limit={}, has_date_range={}, concurrency={})",
        limit,
        filter.has_date_range(),
        config.concurrency,
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

    // Step 1: Fetch workflows (streaming when progress channel available)
    let workflows = if progress_tx.is_some() {
        let (page_tx, mut page_rx) = mpsc::unbounded_channel();
        let client_clone = client.clone();
        let df = date_filter.clone();
        let list_task = tokio::spawn(async move {
            client_clone.list_streaming(&df, limit, page_tx).await
        });
        let mut all = Vec::new();
        while let Some(page) = page_rx.recv().await {
            all.extend(page);
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(Action::DataLoaded(DataPayload::InsightsProgress(
                    InsightsScanPhase::FetchingWorkflows { fetched: all.len() },
                    scan_gen,
                )));
            }
        }
        match list_task.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(ClientError::CommandFailed(e.to_string())),
        }
        all
    } else {
        client.list(&date_filter, limit).await?
    };
    let workflows_scanned = workflows.len();
    info!("Insights - fetched {} workflows", workflows_scanned);

    // Step 2: Compute list-level findings
    let mut all_findings = compute_list_findings(&workflows);

    // Send partial result with list-level findings so UI can render them immediately
    if let Some(ref tx) = progress_tx {
        let partial = InsightsResult {
            findings: rank_findings(all_findings.clone()),
            workflows_scanned,
            histories_fetched: 0,
            computed_at: Utc::now(),
            scan_duration: Utc::now() - scan_start,
        };
        let _ = tx.send(Action::DataLoaded(DataPayload::InsightsPartial(partial, scan_gen)));
    }

    // Step 3: Select workflows for history sampling (all workflows, priority-ordered)
    let samples_to_fetch = select_workflows_for_sampling(&workflows, workflows.len());
    let total_to_fetch = samples_to_fetch.len();

    // Step 4: Fetch histories concurrently, streaming results
    let mut activity_samples: Vec<(String, Vec<crate::domain::ActivityExecution>)> = Vec::new();
    let mut child_wf_samples: Vec<(String, Vec<crate::domain::ChildWorkflowExecution>)> =
        Vec::new();
    let mut event_stats: Vec<WorkflowEventStats> = Vec::new();
    let mut scanned_count: usize = 0;

    // Create a stream of concurrent futures with bounded concurrency
    let concurrency = config.concurrency.max(1);

    type HistoryFuture = std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = (String, ClientResult<Vec<crate::domain::HistoryEvent>>),
                > + Send,
        >,
    >;

    let spawn_fetch = |client: Arc<dyn TemporalClient>, wf_id: String, run_id: String| -> HistoryFuture {
        Box::pin(async move {
            let result = client.get_history(&wf_id, Some(&run_id)).await;
            (wf_id, result)
        })
    };

    let mut futures: FuturesUnordered<HistoryFuture> = FuturesUnordered::new();
    let mut pending_iter = samples_to_fetch.into_iter();

    // Seed the initial batch
    for _ in 0..concurrency {
        if let Some(wf) = pending_iter.next() {
            futures.push(spawn_fetch(client.clone(), wf.workflow_id.clone(), wf.run_id.clone()));
        }
    }

    while let Some((wf_id, result)) = futures.next().await {
        // Enqueue the next workflow as soon as one completes
        if let Some(wf) = pending_iter.next() {
            futures.push(spawn_fetch(client.clone(), wf.workflow_id.clone(), wf.run_id.clone()));
        }

        match result {
            Ok(events) => {
                // Correlate activities and child workflows
                let activities = correlate_activities(&events);
                activity_samples.push((wf_id.clone(), activities));

                let child_workflows = correlate_child_workflows(&events);
                if !child_workflows.is_empty() {
                    child_wf_samples.push((wf_id.clone(), child_workflows));
                }

                // Accumulate event-level stats (streaming — events dropped after this)
                let mut stats = WorkflowEventStats::new(wf_id);
                for event in &events {
                    stats.accumulate(event);
                }
                event_stats.push(stats);
            }
            Err(e) => {
                debug!(
                    "Failed to fetch history for {}: {} (skipping)",
                    wf_id, e
                );
            }
        }

        scanned_count += 1;

        // Send progress update every 5 workflows or on last one
        if let Some(ref tx) = progress_tx {
            if scanned_count % 5 == 0 || scanned_count == total_to_fetch {
                let _ = tx.send(Action::DataLoaded(DataPayload::InsightsProgress(
                    InsightsScanPhase::SamplingHistories {
                        scanned: scanned_count,
                        total: total_to_fetch,
                    },
                    scan_gen,
                )));
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

    // Step 5c: Compute event-level findings from accumulated stats (streaming)
    all_findings.extend(finalize_signal_findings(&event_stats));
    all_findings.extend(finalize_decision_latency_findings(&event_stats));
    all_findings.extend(finalize_scheduling_overhead_findings(&event_stats));

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
