use crate::domain::{HistoryEvent, WorkflowDetail, WorkflowFilter, WorkflowSummary};
use async_trait::async_trait;
use thiserror::Error;

/// Results from async operations
pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Timeout")]
    Timeout,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Workflow not found: {0}")]
    NotFound(String),
}

/// Abstraction over Temporal operations
/// This trait enables mocking for tests
#[async_trait]
pub trait TemporalClient: Send + Sync {
    /// Get count of workflows matching a query
    async fn count(&self, query: Option<&str>) -> ClientResult<u64>;

    /// List workflows matching a filter
    async fn list(&self, filter: &WorkflowFilter, limit: u32) -> ClientResult<Vec<WorkflowSummary>>;

    /// Get detailed information about a workflow
    async fn describe(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<WorkflowDetail>;

    /// Get workflow history events
    async fn get_history(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<Vec<HistoryEvent>>;

    /// Cancel a running workflow
    async fn cancel(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<()>;

    /// Terminate a workflow
    async fn terminate(&self, workflow_id: &str, run_id: Option<&str>, reason: &str) -> ClientResult<()>;
}
