//! CLI Worker for serialized Temporal CLI command execution
//!
//! This module provides a worker that processes all CLI requests sequentially,
//! avoiding concurrent CLI/TLS issues when the terminal is in raw mode.

use crate::action::{Action, DataPayload};
use crate::client::TemporalClient;
use crate::domain::{StatusCounts, TypeStat, WorkflowFilter, WorkflowStatus};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Requests that can be sent to the CLI worker
#[derive(Debug, Clone)]
pub enum CliRequest {
    /// Load workflow status counts (one query per status, executed sequentially)
    LoadCounts,
    /// Load workflows matching a filter
    LoadWorkflows {
        filter: WorkflowFilter,
        limit: u32,
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
}

/// Worker that processes CLI requests sequentially
pub struct CliWorker {
    client: Arc<dyn TemporalClient>,
    request_rx: mpsc::UnboundedReceiver<CliRequest>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl CliWorker {
    /// Create a new CLI worker
    pub fn new(
        client: Arc<dyn TemporalClient>,
        request_rx: mpsc::UnboundedReceiver<CliRequest>,
        action_tx: mpsc::UnboundedSender<Action>,
    ) -> Self {
        Self {
            client,
            request_rx,
            action_tx,
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
            CliRequest::LoadCounts => self.load_counts().await,
            CliRequest::LoadWorkflows { filter, limit } => {
                self.load_workflows(filter, limit).await
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
        }
    }

    /// Load workflow counts for all statuses via count() API
    async fn load_counts(&self) {
        debug!("Loading workflow counts for all statuses");
        let mut counts = StatusCounts::new();

        for status in WorkflowStatus::all() {
            let query = format!("ExecutionStatus='{}'", status.as_query_value());
            match self.client.count(Some(&query)).await {
                Ok(count) => {
                    debug!("Status {:?}: {} workflows", status, count);
                    counts.set(*status, count);
                }
                Err(e) => {
                    error!("Failed to load count for status {:?}: {}", status, e);
                    // Continue with other statuses, don't fail completely
                }
            }
        }

        let _ = self
            .action_tx
            .send(Action::DataLoaded(DataPayload::Counts(counts)));
    }

    /// Load workflows matching a filter
    async fn load_workflows(&self, filter: WorkflowFilter, limit: u32) {
        info!(
            "CliWorker: Loading workflows: filter={:?}, limit={}",
            filter.description(),
            limit
        );
        match self.client.list(&filter, limit).await {
            Ok(workflows) => {
                info!("CliWorker: Loaded {} workflows successfully", workflows.len());
                let _ = self
                    .action_tx
                    .send(Action::DataLoaded(DataPayload::Workflows(workflows)));
            }
            Err(e) => {
                error!("CliWorker: Failed to load workflows: {}", e);
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
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
    pub fn load_counts(&self) {
        let _ = self.send(CliRequest::LoadCounts);
    }

    /// Load workflows with a filter
    pub fn load_workflows(&self, filter: WorkflowFilter, limit: u32) {
        let _ = self.send(CliRequest::LoadWorkflows { filter, limit });
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
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_cli_worker_load_workflows() {
        let client = Arc::new(MockTemporalClient::with_random_data(10));
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let worker = CliWorker::new(client, request_rx, action_tx);
        let _handle = worker.spawn();

        let cli_handle = CliHandle::new(request_tx);
        cli_handle.load_workflows(WorkflowFilter::new(), 50);

        // Wait for the action
        let result = timeout(Duration::from_secs(5), action_rx.recv()).await;
        assert!(result.is_ok());
        let action = result.unwrap();
        assert!(matches!(
            action,
            Some(Action::DataLoaded(DataPayload::Workflows(_)))
        ));
    }

    #[tokio::test]
    async fn test_cli_worker_load_counts() {
        // LoadCounts queries count for each status and returns StatusCounts
        let client = Arc::new(MockTemporalClient::with_random_data(10));
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let worker = CliWorker::new(client, request_rx, action_tx);
        let _handle = worker.spawn();

        let cli_handle = CliHandle::new(request_tx);
        cli_handle.load_counts();

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
        assert!(handle1.send(CliRequest::LoadCounts).is_ok());
        assert!(handle2.send(CliRequest::LoadCounts).is_ok());
    }
}
