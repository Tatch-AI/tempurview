//! gRPC-based Temporal client implementation
//!
//! This client connects directly to Temporal using gRPC, providing a persistent
//! connection that's much more efficient than spawning CLI processes.

use super::{ClientError, ClientResult, TemporalClient};
use crate::domain::{
    HistoryEvent, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary,
};
use crate::proto::{
    self, CountWorkflowExecutionsRequest, DescribeWorkflowExecutionRequest,
    ListWorkflowExecutionsRequest, RequestCancelWorkflowExecutionRequest,
    TerminateWorkflowExecutionRequest, WorkflowServiceClient,
};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use tonic::metadata::AsciiMetadataValue;
use tonic::service::Interceptor;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{Request, Status};

/// Interceptor that adds API key authorization header and namespace
#[derive(Clone)]
struct ApiKeyInterceptor {
    api_key: Option<AsciiMetadataValue>,
    namespace: Option<AsciiMetadataValue>,
}

impl Interceptor for ApiKeyInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if let Some(ref token) = self.api_key {
            request.metadata_mut().insert("authorization", token.clone());
        }
        if let Some(ref ns) = self.namespace {
            request
                .metadata_mut()
                .insert("temporal-namespace", ns.clone());
        }
        Ok(request)
    }
}

/// gRPC-based Temporal client with persistent connection
pub struct GrpcTemporalClient {
    client: WorkflowServiceClient<tonic::service::interceptor::InterceptedService<Channel, ApiKeyInterceptor>>,
    namespace: String,
}

impl GrpcTemporalClient {
    /// Create a new gRPC client from environment variables
    pub async fn from_env() -> ClientResult<Self> {
        let address = std::env::var("TEMPORAL_ADDRESS")
            .map_err(|_| ClientError::ConfigError("TEMPORAL_ADDRESS not set".into()))?;
        let namespace = std::env::var("TEMPORAL_NAMESPACE")
            .map_err(|_| ClientError::ConfigError("TEMPORAL_NAMESPACE not set".into()))?;
        let api_key = std::env::var("TEMPORAL_API_KEY").ok();

        Self::connect(&address, namespace, api_key).await
    }

    /// Connect to Temporal server
    pub async fn connect(
        address: &str,
        namespace: String,
        api_key: Option<String>,
    ) -> ClientResult<Self> {
        tracing::info!("Connecting to Temporal at {}", address);
        tracing::debug!("Namespace: {}", namespace);
        tracing::debug!("API key provided: {}", api_key.is_some());

        // Build endpoint with TLS
        let endpoint = format!("https://{}", address);

        let channel = Endpoint::from_shared(endpoint.clone())
            .map_err(|e| ClientError::ConnectionError(format!("Invalid endpoint: {}", e)))?
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .map_err(|e| ClientError::ConnectionError(format!("TLS config error: {}", e)))?
            .connect()
            .await
            .map_err(|e| {
                tracing::error!("Connection failed to {}: {}", endpoint, e);
                ClientError::ConnectionError(format!("Failed to connect: {}", e))
            })?;

        tracing::info!("Connected to Temporal successfully");

        // Create interceptor with API key and namespace
        let interceptor = ApiKeyInterceptor {
            api_key: api_key.as_ref().and_then(|key| {
                let auth_value = format!("Bearer {}", key);
                auth_value.parse::<AsciiMetadataValue>().ok()
            }),
            namespace: namespace.parse::<AsciiMetadataValue>().ok(),
        };

        let client = WorkflowServiceClient::with_interceptor(channel, interceptor);

        Ok(Self { client, namespace })
    }

    /// Create a tonic request (interceptor will add auth headers)
    fn make_request<T>(&self, inner: T) -> Request<T> {
        Request::new(inner)
    }
}

#[async_trait]
impl TemporalClient for GrpcTemporalClient {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64> {
        let inner = CountWorkflowExecutionsRequest {
            namespace: self.namespace.clone(),
            query: query.unwrap_or("").to_string(),
        };

        let response = self
            .client
            .clone()
            .count_workflow_executions(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        Ok(response.into_inner().count as u64)
    }

    async fn list(
        &self,
        filter: &WorkflowFilter,
        limit: u32,
    ) -> ClientResult<Vec<WorkflowSummary>> {
        let inner = ListWorkflowExecutionsRequest {
            namespace: self.namespace.clone(),
            page_size: limit as i32,
            next_page_token: vec![],
            query: filter.to_query().unwrap_or_default(),
        };

        let response = self
            .client
            .clone()
            .list_workflow_executions(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        let workflows = response
            .into_inner()
            .executions
            .into_iter()
            .map(workflow_info_to_summary)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(workflows)
    }

    async fn describe(
        &self,
        workflow_id: &str,
        run_id: Option<&str>,
    ) -> ClientResult<WorkflowDetail> {
        let inner = DescribeWorkflowExecutionRequest {
            namespace: self.namespace.clone(),
            execution: Some(proto::temporal::api::common::v1::WorkflowExecution {
                workflow_id: workflow_id.to_string(),
                run_id: run_id.unwrap_or("").to_string(),
            }),
        };

        let response = self
            .client
            .clone()
            .describe_workflow_execution(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        let inner = response.into_inner();

        let info = inner
            .workflow_execution_info
            .clone()
            .ok_or_else(|| ClientError::ParseError("Missing workflow execution info".into()))?;

        let summary = workflow_info_to_summary(info.clone())?;

        Ok(WorkflowDetail {
            summary,
            input: None,
            output: None,
            failure: None,
            history_length: info.history_length as u64,
            memo: std::collections::HashMap::new(),
            search_attributes: std::collections::HashMap::new(),
        })
    }

    async fn get_history(
        &self,
        _workflow_id: &str,
        _run_id: Option<&str>,
    ) -> ClientResult<Vec<HistoryEvent>> {
        // TODO: Implement GetWorkflowExecutionHistory
        Ok(vec![])
    }

    async fn cancel(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<()> {
        let inner = RequestCancelWorkflowExecutionRequest {
            namespace: self.namespace.clone(),
            workflow_execution: Some(proto::temporal::api::common::v1::WorkflowExecution {
                workflow_id: workflow_id.to_string(),
                run_id: run_id.unwrap_or("").to_string(),
            }),
            identity: "tempurview".to_string(),
            request_id: uuid_v4(),
            first_execution_run_id: String::new(),
            reason: String::new(),
            links: vec![],
        };

        self.client
            .clone()
            .request_cancel_workflow_execution(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        Ok(())
    }

    async fn terminate(
        &self,
        workflow_id: &str,
        run_id: Option<&str>,
        reason: &str,
    ) -> ClientResult<()> {
        let inner = TerminateWorkflowExecutionRequest {
            namespace: self.namespace.clone(),
            workflow_execution: Some(proto::temporal::api::common::v1::WorkflowExecution {
                workflow_id: workflow_id.to_string(),
                run_id: run_id.unwrap_or("").to_string(),
            }),
            reason: reason.to_string(),
            identity: "tempurview".to_string(),
            details: None,
            first_execution_run_id: String::new(),
            links: vec![],
        };

        self.client
            .clone()
            .terminate_workflow_execution(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        Ok(())
    }
}

/// Convert gRPC status to our ClientError
fn grpc_error_to_client_error(status: Status) -> ClientError {
    match status.code() {
        tonic::Code::NotFound => ClientError::NotFound(status.message().to_string()),
        tonic::Code::DeadlineExceeded => ClientError::Timeout,
        tonic::Code::Unavailable => {
            ClientError::ConnectionError(status.message().to_string())
        }
        _ => ClientError::CommandFailed(format!("{}: {}", status.code(), status.message())),
    }
}

/// Convert proto WorkflowExecutionInfo to our WorkflowSummary
fn workflow_info_to_summary(
    info: proto::temporal::api::workflow::v1::WorkflowExecutionInfo,
) -> ClientResult<WorkflowSummary> {
    let execution = info
        .execution
        .ok_or_else(|| ClientError::ParseError("Missing execution".into()))?;

    let workflow_type = info
        .r#type
        .map(|t| t.name)
        .unwrap_or_else(|| "Unknown".to_string());

    let status = proto_status_to_domain(info.status);

    let start_time = info
        .start_time
        .map(|t| timestamp_to_datetime(&t))
        .unwrap_or_else(Utc::now);

    let close_time = info.close_time.map(|t| timestamp_to_datetime(&t));

    Ok(WorkflowSummary {
        workflow_id: execution.workflow_id,
        run_id: execution.run_id,
        workflow_type,
        status,
        start_time,
        close_time,
        task_queue: info.task_queue,
    })
}

/// Convert proto timestamp to chrono DateTime
fn timestamp_to_datetime(ts: &prost_types::Timestamp) -> DateTime<Utc> {
    Utc.timestamp_opt(ts.seconds, ts.nanos as u32)
        .single()
        .unwrap_or_else(Utc::now)
}

/// Convert proto WorkflowExecutionStatus to our domain status
fn proto_status_to_domain(status: i32) -> WorkflowStatus {
    use proto::temporal::api::enums::v1::WorkflowExecutionStatus;

    match WorkflowExecutionStatus::try_from(status) {
        Ok(WorkflowExecutionStatus::Running) => WorkflowStatus::Running,
        Ok(WorkflowExecutionStatus::Completed) => WorkflowStatus::Completed,
        Ok(WorkflowExecutionStatus::Failed) => WorkflowStatus::Failed,
        Ok(WorkflowExecutionStatus::Canceled) => WorkflowStatus::Canceled,
        Ok(WorkflowExecutionStatus::Terminated) => WorkflowStatus::Terminated,
        Ok(WorkflowExecutionStatus::ContinuedAsNew) => WorkflowStatus::ContinuedAsNew,
        Ok(WorkflowExecutionStatus::TimedOut) => WorkflowStatus::TimedOut,
        _ => WorkflowStatus::Running, // Default to running for unknown
    }
}

/// Generate a simple UUID v4-like string
fn uuid_v4() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rng.gen::<u32>(),
        rng.gen::<u16>(),
        rng.gen::<u16>(),
        rng.gen::<u16>(),
        rng.gen::<u64>() & 0xffffffffffff
    )
}
