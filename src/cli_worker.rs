//! CLI Worker for serialized Temporal CLI command execution
//!
//! This module provides a worker that processes all CLI requests sequentially,
//! avoiding concurrent CLI/TLS issues when the terminal is in raw mode.

use crate::action::{Action, DataPayload};
use crate::client::TemporalClient;
use crate::domain::WorkflowFilter;
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
        }
    }

    /// Load workflow counts - now just fetches total count (1 API call instead of 7)
    /// Individual status counts are computed locally from the workflow list
    async fn load_counts(&self) {
        debug!("Loading total workflow count");
        match self.client.count(None).await {
            Ok(total) => {
                debug!("Total workflows: {}", total);
                // We don't send counts here anymore - they're computed from workflows
                // This is just a connectivity check / total count fetch
                // The actual counts by status come from the workflow list
            }
            Err(e) => {
                error!("Failed to load workflow count: {}", e);
                let _ = self.action_tx.send(Action::Error(e.to_string()));
            }
        }
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
        // LoadCounts now just does a connectivity check - counts are computed locally
        // from the workflow list, so this test verifies no error is sent
        let client = Arc::new(MockTemporalClient::with_random_data(10));
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let worker = CliWorker::new(client, request_rx, action_tx);
        let _handle = worker.spawn();

        let cli_handle = CliHandle::new(request_tx);
        cli_handle.load_counts();

        // LoadCounts no longer sends a result (counts computed from workflows)
        // Just verify it doesn't error - use a short timeout
        let result = timeout(Duration::from_millis(100), action_rx.recv()).await;
        // Should timeout (no action sent) or receive no error
        match result {
            Ok(Some(Action::Error(_))) => panic!("Should not error"),
            _ => {} // Either timeout or no message is fine
        }
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
