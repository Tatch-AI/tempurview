//! CLI Worker for serialized Temporal CLI command execution
//!
//! This module provides a worker that processes all CLI requests sequentially,
//! avoiding concurrent CLI/TLS issues when the terminal is in raw mode.

use crate::action::{Action, DataPayload};
use crate::client::TemporalClient;
use crate::domain::{
    run_insights_scan, InsightsConfig, StatusCounts, TypeStat, WorkflowFilter, WorkflowStatus,
};
use futures::stream::{self, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Requests that can be sent to the CLI worker
#[derive(Debug, Clone)]
pub enum CliRequest {
    /// Load workflow status counts (one query per status, executed sequentially)
    LoadCounts {
        filter: WorkflowFilter,
    },
    /// Load workflows matching a filter
    LoadWorkflows {
        filter: WorkflowFilter,
        limit: u32,
        gen: u64,
    },
    /// Load detailed information about a specific workflow
    LoadDetail {
        workflow_id: String,
        run_id: Option<String>,
    },
    /// Cancel a running workflow
    CancelWorkflow {
        workflow_id: String,
        run_id: Option<String>,
    },
    /// Terminate a workflow
    TerminateWorkflow {
        workflow_id: String,
        run_id: Option<String>,
        reason: String,
    },
    /// Load type statistics (fetches workflows with filter, groups by type)
    LoadTypeStats {
        filter: WorkflowFilter,
        limit: u32,
    },
    /// Load workflow history events
    LoadHistory {
        workflow_id: String,
        run_id: Option<String>,
    },
    /// Run insights scan (list + sample histories + compute findings)
    LoadInsights {
        filter: WorkflowFilter,
        limit: u32,
        gen: u64,
    },
    /// Scan workflow histories for activity failures
    ScanActivityFails {
        workflows: Vec<(String, String)>,
        gen: u64,
    },
}

/// Worker that processes CLI requests sequentially
pub struct CliWorker {
    client: Arc<dyn TemporalClient>,
    request_rx: mpsc::UnboundedReceiver<CliRequest>,
    action_tx: mpsc::UnboundedSender<Action>,
    insights_config: InsightsConfig,
}

impl CliWorker {
    /// Create a new CLI worker
    pub fn new(
        client: Arc<dyn TemporalClient>,
        request_rx: mpsc::UnboundedReceiver<CliRequest>,
        action_tx: mpsc::UnboundedSender<Action>,
        insights_config: InsightsConfig,
    ) -> Self {
        Self {
            client,
            request_rx,
            action_tx,
            insights_config,
        }
    }

    /// Spawn the worker as a background task
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    /// Run the worker loop, processing requests sequentially
    async fn run(mut self) {
        debug!("CLI worker started");
        while let Some(request) = self.request_rx.recv().await {
            debug!("CLI worker processing request: {:?}", request);
            self.handle_request(request).await;
        }
        debug!("CLI worker shutting down");
    }

    /// Handle a single request
    async fn handle_request(&self, request: CliRequest) {
        match request {
            CliRequest::LoadCounts { filter } => self.load_counts(&filter).await,
            CliRequest::LoadWorkflows { filter, limit, gen } => {
                self.load_workflows(filter, limit, gen);
            }
            CliRequest::LoadDetail {
                workflow_id,
                run_id,
            } => self.load_detail(workflow_id, run_id).await,
            CliRequest::CancelWorkflow {
                workflow_id,
                run_id,
            } => self.cancel_workflow(workflow_id, run_id).await,
            CliRequest::TerminateWorkflow {
                workflow_id,
                run_id,
                reason,
            } => self.terminate_workflow(workflow_id, run_id, reason).await,
            CliRequest::LoadTypeStats { filter, limit } => {
                self.load_type_stats(&filter, limit).await
            }
            CliRequest::LoadHistory {
                workflow_id,
                run_id,
            } => self.load_history(workflow_id, run_id).await,
            CliRequest::LoadInsights { filter, limit, gen } => {
                self.load_insights(&filter, limit, gen);
            }
            CliRequest::ScanActivityFails { workflows, gen } => {
                self.scan_activity_fails(workflows, gen);
            }
        }
    }

    /// Load workflow counts for all statuses via count() API.
    /// Includes date range from filter so counts match the visible workflow list.
    async fn load_counts(&self, filter: &WorkflowFilter) {
        debug!("Loading workflow counts for all statuses");
        let mut counts = StatusCounts::new();

        // Build date conditions from filter
        let date_filter = WorkflowFilter {
            start_time_after: filter.start_time_after,
            start_time_before: filter.start_time_before,
            close_time_after: filter.close_time_after,
            close_time_before: filter.close_time_before,
            ..WorkflowFilter::new()
        };

        for status in WorkflowStatus::all() {
            let mut conditions = vec![
                format!("ExecutionStatus='{}'", status.as_query_value()),
            ];
            // Append date conditions from the date-only filter
            let date_query = date_filter.to_query();
            if let Some(dq) = &date_query {
                conditions.push(dq.clone());
            }
            let query = conditions.join(" AND ");

            match self.client.count(Some(&query)).await {
                Ok(count) => {
                    debug!("Status {:?}: {} workflows", status, count);
                    counts.set(*status, count);
                }
                Err(e) => {
                    error!("Failed to load count for status {:?}: {}", status, e);
                }
            }
        }

        let _ = self
            .action_tx
            .send(Action::DataLoaded(DataPayload::Counts(counts)));
    }

    /// Load workflows matching a filter using streaming pagination.
    /// Spawns the streaming+forwarding as a detached task so the worker
    /// can immediately process subsequent requests (e.g. LoadDetail).
    fn load_workflows(&self, filter: WorkflowFilter, limit: u32, gen: u64) {
        info!(
            "CliWorker: Loading workflows (streaming): filter={:?}, limit={}, gen={}",
            filter.description(),
            limit,
            gen
        );

        let client = self.client.clone();
        let action_tx = self.action_tx.clone();

        tokio::spawn(async move {
            let (page_tx, mut page_rx) = mpsc::unbounded_channel();

            // Spawn streaming list in background
            let filter_clone = filter.clone();
            let list_task = tokio::spawn(async move {
                client.list_streaming(&filter_clone, limit, page_tx).await
            });

            // Forward pages to app as they arrive
            let mut total_loaded = 0usize;
            while let Some(page) = page_rx.recv().await {
                total_loaded += page.len();
                info!(
                    "CliWorker: Forwarding page ({} workflows total)",
                    total_loaded
                );
                let _ = action_tx
                    .send(Action::DataLoaded(DataPayload::WorkflowsPage(page, gen)));
            }

            // Channel closed — streaming complete or errored
            match list_task.await {
                Ok(Ok(())) => {
                    info!(
                        "CliWorker: Streaming complete ({} workflows)",
                        total_loaded
                    );
                    let _ = action_tx
                        .send(Action::DataLoaded(DataPayload::WorkflowsDone(gen)));
                }
                Ok(Err(e)) => {
                    error!(
                        "CliWorker: Streaming failed after {} workflows: {}",
                        total_loaded, e
                    );
                    if total_loaded > 0 {
                        let _ = action_tx
                            .send(Action::DataLoaded(DataPayload::WorkflowsDone(gen)));
                    }
                    let _ = action_tx.send(Action::Error(format!(
                        "Loaded {} workflows before error: {}",
                        total_loaded, e
                    )));
                }
                Err(e) => {
                    error!("CliWorker: Streaming task panicked: {}", e);
                    let _ = action_tx.send(Action::Error(e.to_string()));
                }
            }
        });
    }

    /// Load detailed information about a workflow
    async fn load_detail(&self, workflow_id: String, run_id: Option<String>) {
        debug!("Loading workflow detail: {}", workflow_id);
        match self.client.describe(&workflow_id, run_id.as_deref()).await {
            Ok(detail) => {
                debug!("Loaded detail for workflow: {}", workflow_id);
                let _ = self
                    .action_tx
                    .send(Action::DataLoaded(DataPayload::Detail(Box::new(detail))));
            }
            Err(e) => {
                error!("Failed to load workflow detail for {}: {}", workflow_id, e);
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
    }

    /// Load type statistics by fetching workflows and grouping by type
    async fn load_type_stats(&self, filter: &WorkflowFilter, limit: u32) {
        debug!("Loading type stats with limit={}, filter={:?}", limit, filter);
        // Build a date-only filter for type stats (ignore status/type/id filters)
        let date_filter = WorkflowFilter {
            start_time_after: filter.start_time_after,
            start_time_before: filter.start_time_before,
            close_time_after: filter.close_time_after,
            close_time_before: filter.close_time_before,
            ..WorkflowFilter::new()
        };
        match self.client.list(&date_filter, limit).await {
            Ok(workflows) => {
                let stats = TypeStat::from_workflows(&workflows);
                debug!("Computed {} type stats from {} workflows", stats.len(), workflows.len());
                let _ = self
                    .action_tx
                    .send(Action::DataLoaded(DataPayload::TypeStats(stats)));
            }
            Err(e) => {
                error!("Failed to load type stats: {}", e);
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
    }

    /// Load workflow history events
    async fn load_history(&self, workflow_id: String, run_id: Option<String>) {
        debug!("Loading history for workflow: {}", workflow_id);
        match self
            .client
            .get_history(&workflow_id, run_id.as_deref())
            .await
        {
            Ok(events) => {
                debug!(
                    "Loaded {} history events for workflow: {}",
                    events.len(),
                    workflow_id
                );
                let _ = self
                    .action_tx
                    .send(Action::DataLoaded(DataPayload::History(events)));
            }
            Err(e) => {
                error!(
                    "Failed to load history for workflow {}: {}",
                    workflow_id, e
                );
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
    }

    /// Run insights scan as a detached task (same pattern as load_workflows).
    /// Returns immediately so the worker can process other requests concurrently.
    fn load_insights(&self, filter: &WorkflowFilter, limit: u32, gen: u64) {
        let client = self.client.clone();
        let action_tx = self.action_tx.clone();
        let config = self.insights_config.clone();
        let filter = filter.clone();

        tokio::spawn(async move {
            match run_insights_scan(
                client,
                &filter,
                limit,
                &config,
                Some(action_tx.clone()),
                gen,
            )
            .await
            {
                Ok(result) => {
                    let _ = action_tx
                        .send(Action::DataLoaded(DataPayload::Insights(result, gen)));
                }
                Err(e) => {
                    error!("CliWorker: Failed insights scan: {}", e);
                    let _ = action_tx.send(Action::Error(e.to_string()));
                }
            }
        });
    }

    /// Scan workflow histories for activity failures. Spawns detached like load_workflows.
    /// Sends progressive partial results every 50 workflows scanned.
    fn scan_activity_fails(&self, workflows: Vec<(String, String)>, gen: u64) {
        let client = self.client.clone();
        let action_tx = self.action_tx.clone();

        tokio::spawn(async move {
            let total = workflows.len();
            info!(
                "ScanActivityFails: scanning {} workflows (gen={})",
                total, gen
            );

            let mut fail_ids = HashSet::new();
            let mut scanned = 0usize;

            let mut results = stream::iter(workflows)
                .map(|(wf_id, run_id)| {
                    let client = client.clone();
                    async move {
                        let has_fail = match client.get_history(&wf_id, Some(&run_id)).await {
                            Ok(events) => events
                                .iter()
                                .any(|ev| ev.event_type.contains("ActivityTaskFailed")),
                            Err(e) => {
                                debug!(
                                    "ScanActivityFails: failed to get history for {}: {}",
                                    wf_id, e
                                );
                                false
                            }
                        };
                        (wf_id, has_fail)
                    }
                })
                .buffer_unordered(50);

            while let Some((wf_id, has_fail)) = results.next().await {
                if has_fail {
                    fail_ids.insert(wf_id);
                }
                scanned += 1;
                // Send partial update every 50 workflows
                if scanned % 50 == 0 {
                    debug!(
                        "ScanActivityFails: {}/{} scanned, {} found (gen={})",
                        scanned,
                        total,
                        fail_ids.len(),
                        gen
                    );
                    let _ = action_tx.send(Action::DataLoaded(
                        DataPayload::ActivityFailPartial(fail_ids.clone(), gen),
                    ));
                }
            }

            info!(
                "ScanActivityFails: complete — found {} in {} workflows (gen={})",
                fail_ids.len(),
                total,
                gen
            );
            let _ = action_tx.send(Action::DataLoaded(DataPayload::ActivityFailIds(
                fail_ids, gen,
            )));
        });
    }

    /// Cancel a running workflow
    async fn cancel_workflow(&self, workflow_id: String, run_id: Option<String>) {
        info!("Cancelling workflow: {}", workflow_id);
        match self.client.cancel(&workflow_id, run_id.as_deref()).await {
            Ok(()) => {
                info!("Successfully cancelled workflow: {}", workflow_id);
                let _ = self.action_tx.send(Action::Refresh);
            }
            Err(e) => {
                error!("Failed to cancel workflow {}: {}", workflow_id, e);
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
    }

    /// Terminate a workflow
    async fn terminate_workflow(
        &self,
        workflow_id: String,
        run_id: Option<String>,
        reason: String,
    ) {
        warn!("Terminating workflow: {}", workflow_id);
        match self
            .client
            .terminate(&workflow_id, run_id.as_deref(), &reason)
            .await
        {
            Ok(()) => {
                info!("Successfully terminated workflow: {}", workflow_id);
                let _ = self.action_tx.send(Action::Refresh);
            }
            Err(e) => {
                error!("Failed to terminate workflow {}: {}", workflow_id, e);
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
    }
}

/// Handle for sending requests to the CLI worker
#[derive(Clone)]
pub struct CliHandle {
    request_tx: mpsc::UnboundedSender<CliRequest>,
}

impl CliHandle {
    /// Create a new CLI handle
    pub fn new(request_tx: mpsc::UnboundedSender<CliRequest>) -> Self {
        Self { request_tx }
    }

    /// Send a request to the CLI worker
    pub fn send(&self, request: CliRequest) -> Result<(), mpsc::error::SendError<CliRequest>> {
        self.request_tx.send(request)
    }

    /// Load workflow counts
    pub fn load_counts(&self, filter: WorkflowFilter) {
        let _ = self.send(CliRequest::LoadCounts { filter });
    }

    /// Load workflows with a filter
    pub fn load_workflows(&self, filter: WorkflowFilter, limit: u32, gen: u64) {
        let _ = self.send(CliRequest::LoadWorkflows { filter, limit, gen });
    }

    /// Load workflow detail
    pub fn load_detail(&self, workflow_id: String, run_id: Option<String>) {
        let _ = self.send(CliRequest::LoadDetail {
            workflow_id,
            run_id,
        });
    }

    /// Load type statistics
    pub fn load_type_stats(&self, filter: WorkflowFilter, limit: u32) {
        let _ = self.send(CliRequest::LoadTypeStats { filter, limit });
    }

    /// Load workflow history
    pub fn load_history(&self, workflow_id: String, run_id: Option<String>) {
        let _ = self.send(CliRequest::LoadHistory {
            workflow_id,
            run_id,
        });
    }

    /// Run insights scan
    pub fn load_insights(&self, filter: WorkflowFilter, limit: u32, gen: u64) {
        let _ = self.send(CliRequest::LoadInsights { filter, limit, gen });
    }

    /// Scan workflow histories for activity failures
    pub fn scan_activity_fails(&self, workflows: Vec<(String, String)>, gen: u64) {
        let _ = self.send(CliRequest::ScanActivityFails { workflows, gen });
    }

    /// Cancel a workflow
    pub fn cancel_workflow(&self, workflow_id: String, run_id: Option<String>) {
        let _ = self.send(CliRequest::CancelWorkflow {
            workflow_id,
            run_id,
        });
    }

    /// Terminate a workflow
    pub fn terminate_workflow(&self, workflow_id: String, run_id: Option<String>, reason: String) {
        let _ = self.send(CliRequest::TerminateWorkflow {
            workflow_id,
            run_id,
            reason,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::MockTemporalClient;
    use crate::domain::InsightsConfig;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_cli_worker_load_workflows() {
        let client = Arc::new(MockTemporalClient::with_random_data(10));
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let worker = CliWorker::new(client, request_rx, action_tx, InsightsConfig::default());
        let _handle = worker.spawn();

        let cli_handle = CliHandle::new(request_tx);
        cli_handle.load_workflows(WorkflowFilter::new(), 50, 1);

        // Streaming sends WorkflowsPage(s) then WorkflowsDone
        let mut got_page = false;
        let mut got_done = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            match timeout(Duration::from_secs(2), action_rx.recv()).await {
                Ok(Some(Action::DataLoaded(DataPayload::WorkflowsPage(page, _gen)))) => {
                    assert!(!page.is_empty());
                    got_page = true;
                }
                Ok(Some(Action::DataLoaded(DataPayload::WorkflowsDone(_gen)))) => {
                    got_done = true;
                    break;
                }
                Ok(Some(_)) => {} // ignore other actions
                _ => break,
            }
        }
        assert!(got_page, "Expected at least one WorkflowsPage");
        assert!(got_done, "Expected WorkflowsDone");
    }

    #[tokio::test]
    async fn test_cli_worker_load_counts() {
        // LoadCounts queries count for each status and returns StatusCounts
        let client = Arc::new(MockTemporalClient::with_random_data(10));
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let worker = CliWorker::new(client, request_rx, action_tx, InsightsConfig::default());
        let _handle = worker.spawn();

        let cli_handle = CliHandle::new(request_tx);
        cli_handle.load_counts(WorkflowFilter::new());

        // Should receive counts for all statuses
        let result = timeout(Duration::from_secs(5), action_rx.recv()).await;
        assert!(result.is_ok());
        let action = result.unwrap();
        assert!(matches!(
            action,
            Some(Action::DataLoaded(DataPayload::Counts(_)))
        ));
    }

    #[tokio::test]
    async fn test_cli_handle_clone() {
        let (request_tx, _request_rx) = mpsc::unbounded_channel();
        let handle1 = CliHandle::new(request_tx);
        let handle2 = handle1.clone();

        // Both handles should be usable
        assert!(handle1.send(CliRequest::LoadCounts { filter: WorkflowFilter::new() }).is_ok());
        assert!(handle2.send(CliRequest::LoadCounts { filter: WorkflowFilter::new() }).is_ok());
    }
}
