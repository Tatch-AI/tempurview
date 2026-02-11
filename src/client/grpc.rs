//! gRPC-based Temporal client implementation
//!
//! This client connects directly to Temporal using gRPC, providing a persistent
//! connection that's much more efficient than spawning CLI processes.

use super::{ClientError, ClientResult, TemporalClient};
use crate::domain::{
    HistoryEvent, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary,
};
use tokio::sync::mpsc;
use crate::proto::{
    self, CountWorkflowExecutionsRequest, DescribeWorkflowExecutionRequest,
    GetWorkflowExecutionHistoryRequest, GetWorkflowExecutionHistoryReverseRequest,
    ListWorkflowExecutionsRequest, RequestCancelWorkflowExecutionRequest,
    TerminateWorkflowExecutionRequest, WorkflowServiceClient,
};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
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

    /// Get workflow input from first history event
    async fn get_workflow_input(
        &self,
        workflow_id: &str,
        run_id: Option<&str>,
    ) -> ClientResult<serde_json::Value> {
        let inner = GetWorkflowExecutionHistoryRequest {
            namespace: self.namespace.clone(),
            execution: Some(proto::temporal::api::common::v1::WorkflowExecution {
                workflow_id: workflow_id.to_string(),
                run_id: run_id.unwrap_or("").to_string(),
            }),
            maximum_page_size: 1, // Only need first event
            next_page_token: vec![],
            wait_new_event: false,
            history_event_filter_type: 0,
            skip_archival: false,
        };

        let response = self
            .client
            .clone()
            .get_workflow_execution_history(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        let history = response.into_inner().history;
        if let Some(history) = history {
            if let Some(first_event) = history.events.into_iter().next() {
                if let Some(proto::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionStartedEventAttributes(attrs)) = first_event.attributes {
                    if let Some(input) = attrs.input {
                        return Ok(payloads_to_json(&input));
                    }
                }
            }
        }

        Err(ClientError::ParseError("No input found in workflow history".into()))
    }

    /// Get workflow result (output or failure) from last history event
    async fn get_workflow_result(
        &self,
        workflow_id: &str,
        run_id: Option<&str>,
    ) -> ClientResult<(Option<serde_json::Value>, Option<crate::domain::FailureInfo>)> {
        let inner = GetWorkflowExecutionHistoryReverseRequest {
            namespace: self.namespace.clone(),
            execution: Some(proto::temporal::api::common::v1::WorkflowExecution {
                workflow_id: workflow_id.to_string(),
                run_id: run_id.unwrap_or("").to_string(),
            }),
            maximum_page_size: 10, // Get last few events to find completion/failure
            next_page_token: vec![],
        };

        let response = self
            .client
            .clone()
            .get_workflow_execution_history_reverse(self.make_request(inner))
            .await
            .map_err(grpc_error_to_client_error)?;

        let history = response.into_inner().history;
        if let Some(history) = history {
            for event in history.events {
                match event.attributes {
                    Some(proto::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionCompletedEventAttributes(attrs)) => {
                        let output = attrs.result.map(|r| payloads_to_json(&r));
                        return Ok((output, None));
                    }
                    Some(proto::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionFailedEventAttributes(attrs)) => {
                        let failure = attrs.failure.map(failure_to_domain);
                        return Ok((None, failure));
                    }
                    Some(proto::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionTimedOutEventAttributes(_)) => {
                        return Ok((None, Some(crate::domain::FailureInfo {
                            message: "Workflow timed out".to_string(),
                            failure_type: "Timeout".to_string(),
                            stack_trace: None,
                            cause: None,
                        })));
                    }
                    Some(proto::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionCanceledEventAttributes(_)) => {
                        return Ok((None, Some(crate::domain::FailureInfo {
                            message: "Workflow was canceled".to_string(),
                            failure_type: "Canceled".to_string(),
                            stack_trace: None,
                            cause: None,
                        })));
                    }
                    Some(proto::temporal::api::history::v1::history_event::Attributes::WorkflowExecutionTerminatedEventAttributes(attrs)) => {
                        return Ok((None, Some(crate::domain::FailureInfo {
                            message: attrs.reason,
                            failure_type: "Terminated".to_string(),
                            stack_trace: None,
                            cause: None,
                        })));
                    }
                    _ => continue,
                }
            }
        }

        Ok((None, None))
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
        let mut all_workflows = Vec::new();
        let mut next_page_token = vec![];
        let query = filter.to_query().unwrap_or_default();
        let page_size = 1000.min(limit) as i32;
        let mut type_intern: HashMap<Arc<str>, Arc<str>> = HashMap::new();
        let mut queue_intern: HashMap<Arc<str>, Arc<str>> = HashMap::new();

        loop {
            let inner = ListWorkflowExecutionsRequest {
                namespace: self.namespace.clone(),
                page_size,
                next_page_token: next_page_token.clone(),
                query: query.clone(),
            };

            let response = self
                .client
                .clone()
                .list_workflow_executions(self.make_request(inner))
                .await
                .map_err(grpc_error_to_client_error)?;

            let resp = response.into_inner();
            for info in resp.executions {
                let mut summary = workflow_info_to_summary(info)?;
                summary.workflow_type = type_intern
                    .entry(summary.workflow_type.clone())
                    .or_insert_with(|| summary.workflow_type.clone())
                    .clone();
                summary.task_queue = queue_intern
                    .entry(summary.task_queue.clone())
                    .or_insert_with(|| summary.task_queue.clone())
                    .clone();
                all_workflows.push(summary);
                if all_workflows.len() >= limit as usize {
                    return Ok(all_workflows);
                }
            }

            if resp.next_page_token.is_empty() {
                break;
            }
            next_page_token = resp.next_page_token;
        }

        Ok(all_workflows)
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

        // Fetch input from first history event
        let input = self.get_workflow_input(workflow_id, run_id).await.ok();

        // Fetch output/failure from last history event (only if workflow is closed)
        let (output, failure) = if summary.close_time.is_some() {
            self.get_workflow_result(workflow_id, run_id).await.ok().unwrap_or((None, None))
        } else {
            (None, None)
        };

        Ok(WorkflowDetail {
            summary,
            input,
            output,
            failure,
            history_length: info.history_length as u64,
            memo: std::collections::HashMap::new(),
            search_attributes: std::collections::HashMap::new(),
        })
    }

    async fn get_history(
        &self,
        workflow_id: &str,
        run_id: Option<&str>,
    ) -> ClientResult<Vec<HistoryEvent>> {
        let mut all_events = Vec::new();
        let mut next_page_token = vec![];

        loop {
            let inner = GetWorkflowExecutionHistoryRequest {
                namespace: self.namespace.clone(),
                execution: Some(proto::temporal::api::common::v1::WorkflowExecution {
                    workflow_id: workflow_id.to_string(),
                    run_id: run_id.unwrap_or("").to_string(),
                }),
                maximum_page_size: 1000,
                next_page_token: next_page_token.clone(),
                wait_new_event: false,
                history_event_filter_type: 0, // HISTORY_EVENT_FILTER_TYPE_ALL_EVENT
                skip_archival: false,
            };

            let response = self
                .client
                .clone()
                .get_workflow_execution_history(self.make_request(inner))
                .await
                .map_err(grpc_error_to_client_error)?;

            let resp = response.into_inner();
            if let Some(history) = resp.history {
                for e in history.events {
                    let details = extract_event_details(&e);
                    all_events.push(HistoryEvent {
                        event_id: e.event_id,
                        event_type: event_type_name(e.event_type),
                        timestamp: e
                            .event_time
                            .map(|t| timestamp_to_datetime(&t))
                            .unwrap_or_else(Utc::now),
                        details,
                    });
                }
            }

            if resp.next_page_token.is_empty() {
                break;
            }
            next_page_token = resp.next_page_token;
        }

        Ok(all_events)
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

    async fn list_streaming(
        &self,
        filter: &WorkflowFilter,
        limit: u32,
        page_tx: mpsc::UnboundedSender<Vec<WorkflowSummary>>,
    ) -> ClientResult<()> {
        let mut next_page_token = vec![];
        let query = filter.to_query().unwrap_or_default();
        let page_size = 1000.min(limit) as i32;
        let mut type_intern: HashMap<Arc<str>, Arc<str>> = HashMap::new();
        let mut queue_intern: HashMap<Arc<str>, Arc<str>> = HashMap::new();
        let mut total_count = 0usize;

        loop {
            let inner = ListWorkflowExecutionsRequest {
                namespace: self.namespace.clone(),
                page_size,
                next_page_token: next_page_token.clone(),
                query: query.clone(),
            };

            let response = self
                .client
                .clone()
                .list_workflow_executions(self.make_request(inner))
                .await
                .map_err(grpc_error_to_client_error)?;

            let resp = response.into_inner();
            let mut page_batch = Vec::with_capacity(resp.executions.len());

            for info in resp.executions {
                let mut summary = workflow_info_to_summary(info)?;
                summary.workflow_type = type_intern
                    .entry(summary.workflow_type.clone())
                    .or_insert_with(|| summary.workflow_type.clone())
                    .clone();
                summary.task_queue = queue_intern
                    .entry(summary.task_queue.clone())
                    .or_insert_with(|| summary.task_queue.clone())
                    .clone();
                page_batch.push(summary);
                total_count += 1;
                if total_count >= limit as usize {
                    let _ = page_tx.send(page_batch);
                    return Ok(());
                }
            }

            if !page_batch.is_empty() {
                let _ = page_tx.send(page_batch);
            }

            if resp.next_page_token.is_empty() {
                break;
            }
            next_page_token = resp.next_page_token;
        }
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

    let workflow_type: Arc<str> = Arc::from(
        info.r#type
            .map(|t| t.name)
            .unwrap_or_else(|| "Unknown".to_string())
            .as_str(),
    );

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
        task_queue: Arc::from(info.task_queue.as_str()),
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

/// Convert Payloads to JSON value
fn payloads_to_json(payloads: &proto::temporal::api::common::v1::Payloads) -> serde_json::Value {
    let values: Vec<serde_json::Value> = payloads
        .payloads
        .iter()
        .map(|payload| {
            // Try to parse as JSON first
            if let Ok(json_str) = std::str::from_utf8(&payload.data) {
                if let Ok(value) = serde_json::from_str(json_str) {
                    return value;
                }
                // Return as string if valid UTF-8 but not JSON
                return serde_json::Value::String(json_str.to_string());
            }
            // Fall back to showing data length for binary data
            serde_json::json!({
                "binary_data": format!("<{} bytes>", payload.data.len()),
                "metadata": payload.metadata.iter()
                    .map(|(k, v)| (k.clone(), String::from_utf8_lossy(v).to_string()))
                    .collect::<std::collections::HashMap<_, _>>()
            })
        })
        .collect();

    if values.len() == 1 {
        values.into_iter().next().unwrap()
    } else {
        serde_json::Value::Array(values)
    }
}

/// Convert an event_type i32 to a human-readable name
fn event_type_name(event_type: i32) -> String {
    use proto::temporal::api::enums::v1::EventType;
    match EventType::try_from(event_type) {
        Ok(et) => format!("{:?}", et),
        Err(_) => format!("Unknown({})", event_type),
    }
}

/// Extract structured details JSON from a history event's attributes
fn extract_event_details(
    event: &proto::temporal::api::history::v1::HistoryEvent,
) -> serde_json::Value {
    use proto::temporal::api::history::v1::history_event::Attributes;

    match &event.attributes {
        Some(Attributes::ActivityTaskScheduledEventAttributes(attrs)) => {
            serde_json::json!({
                "activity_id": attrs.activity_id,
                "activity_type": attrs.activity_type.as_ref().map(|t| &t.name),
                "task_queue": attrs.task_queue.as_ref().map(|q| &q.name),
                "input": attrs.input.as_ref().map(|p| payloads_to_json(p)),
            })
        }
        Some(Attributes::ActivityTaskStartedEventAttributes(attrs)) => {
            let mut json = serde_json::json!({
                "scheduled_event_id": attrs.scheduled_event_id,
                "attempt": attrs.attempt,
                "identity": attrs.identity,
            });
            if let Some(ref f) = attrs.last_failure {
                json["last_failure"] = serde_json::json!({
                    "message": f.message,
                });
            }
            json
        }
        Some(Attributes::ActivityTaskCompletedEventAttributes(attrs)) => {
            serde_json::json!({
                "scheduled_event_id": attrs.scheduled_event_id,
                "started_event_id": attrs.started_event_id,
                "result": attrs.result.as_ref().map(|p| payloads_to_json(p)),
                "identity": attrs.identity,
            })
        }
        Some(Attributes::ActivityTaskFailedEventAttributes(attrs)) => {
            let mut json = serde_json::json!({
                "scheduled_event_id": attrs.scheduled_event_id,
                "started_event_id": attrs.started_event_id,
                "retry_state": attrs.retry_state,
                "identity": attrs.identity,
            });
            if let Some(ref f) = attrs.failure {
                json["failure"] = serde_json::json!({
                    "message": f.message,
                    "failure_type": f.failure_info.as_ref().map(|info| match info {
                        proto::temporal::api::failure::v1::failure::FailureInfo::ApplicationFailureInfo(_) => "ApplicationFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::TimeoutFailureInfo(_) => "TimeoutFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::CanceledFailureInfo(_) => "CanceledFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::TerminatedFailureInfo(_) => "TerminatedFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::ServerFailureInfo(_) => "ServerFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::ActivityFailureInfo(_) => "ActivityFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::ChildWorkflowExecutionFailureInfo(_) => "ChildWorkflowExecutionFailure",
                        _ => "Unknown",
                    }).unwrap_or("Unknown"),
                    "stack_trace": if f.stack_trace.is_empty() { None } else { Some(&f.stack_trace) },
                });
            }
            json
        }
        Some(Attributes::ActivityTaskTimedOutEventAttributes(attrs)) => {
            let mut json = serde_json::json!({
                "scheduled_event_id": attrs.scheduled_event_id,
                "started_event_id": attrs.started_event_id,
                "retry_state": attrs.retry_state,
            });
            if let Some(ref f) = attrs.failure {
                json["failure"] = serde_json::json!({
                    "message": f.message,
                });
            }
            json
        }
        Some(Attributes::ActivityTaskCanceledEventAttributes(attrs)) => {
            serde_json::json!({
                "scheduled_event_id": attrs.scheduled_event_id,
                "started_event_id": attrs.started_event_id,
                "identity": attrs.identity,
            })
        }
        Some(Attributes::ActivityTaskCancelRequestedEventAttributes(attrs)) => {
            serde_json::json!({
                "scheduled_event_id": attrs.scheduled_event_id,
            })
        }
        // Child workflow events
        Some(Attributes::StartChildWorkflowExecutionInitiatedEventAttributes(attrs)) => {
            serde_json::json!({
                "workflow_id": attrs.workflow_id,
                "workflow_type": attrs.workflow_type.as_ref().map(|t| &t.name),
                "task_queue": attrs.task_queue.as_ref().map(|q| &q.name),
                "namespace": attrs.namespace,
                "parent_close_policy": attrs.parent_close_policy,
                "input": attrs.input.as_ref().map(|p| payloads_to_json(p)),
            })
        }
        Some(Attributes::StartChildWorkflowExecutionFailedEventAttributes(attrs)) => {
            serde_json::json!({
                "workflow_id": attrs.workflow_id,
                "workflow_type": attrs.workflow_type.as_ref().map(|t| &t.name),
                "cause": attrs.cause,
                "initiated_event_id": attrs.initiated_event_id,
            })
        }
        Some(Attributes::ChildWorkflowExecutionStartedEventAttributes(attrs)) => {
            serde_json::json!({
                "initiated_event_id": attrs.initiated_event_id,
                "workflow_id": attrs.workflow_execution.as_ref().map(|e| &e.workflow_id),
                "run_id": attrs.workflow_execution.as_ref().map(|e| &e.run_id),
                "workflow_type": attrs.workflow_type.as_ref().map(|t| &t.name),
            })
        }
        Some(Attributes::ChildWorkflowExecutionCompletedEventAttributes(attrs)) => {
            serde_json::json!({
                "initiated_event_id": attrs.initiated_event_id,
                "started_event_id": attrs.started_event_id,
                "workflow_id": attrs.workflow_execution.as_ref().map(|e| &e.workflow_id),
                "run_id": attrs.workflow_execution.as_ref().map(|e| &e.run_id),
                "result": attrs.result.as_ref().map(|p| payloads_to_json(p)),
            })
        }
        Some(Attributes::ChildWorkflowExecutionFailedEventAttributes(attrs)) => {
            let mut json = serde_json::json!({
                "initiated_event_id": attrs.initiated_event_id,
                "started_event_id": attrs.started_event_id,
                "workflow_id": attrs.workflow_execution.as_ref().map(|e| &e.workflow_id),
                "run_id": attrs.workflow_execution.as_ref().map(|e| &e.run_id),
                "retry_state": attrs.retry_state,
            });
            if let Some(ref f) = attrs.failure {
                json["failure"] = serde_json::json!({
                    "message": f.message,
                    "failure_type": f.failure_info.as_ref().map(|info| match info {
                        proto::temporal::api::failure::v1::failure::FailureInfo::ApplicationFailureInfo(_) => "ApplicationFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::TimeoutFailureInfo(_) => "TimeoutFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::CanceledFailureInfo(_) => "CanceledFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::TerminatedFailureInfo(_) => "TerminatedFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::ServerFailureInfo(_) => "ServerFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::ActivityFailureInfo(_) => "ActivityFailure",
                        proto::temporal::api::failure::v1::failure::FailureInfo::ChildWorkflowExecutionFailureInfo(_) => "ChildWorkflowExecutionFailure",
                        _ => "Unknown",
                    }).unwrap_or("Unknown"),
                    "stack_trace": if f.stack_trace.is_empty() { None } else { Some(&f.stack_trace) },
                });
            }
            json
        }
        Some(Attributes::ChildWorkflowExecutionCanceledEventAttributes(attrs)) => {
            serde_json::json!({
                "initiated_event_id": attrs.initiated_event_id,
                "started_event_id": attrs.started_event_id,
                "workflow_id": attrs.workflow_execution.as_ref().map(|e| &e.workflow_id),
                "run_id": attrs.workflow_execution.as_ref().map(|e| &e.run_id),
            })
        }
        Some(Attributes::ChildWorkflowExecutionTimedOutEventAttributes(attrs)) => {
            serde_json::json!({
                "initiated_event_id": attrs.initiated_event_id,
                "started_event_id": attrs.started_event_id,
                "workflow_id": attrs.workflow_execution.as_ref().map(|e| &e.workflow_id),
                "run_id": attrs.workflow_execution.as_ref().map(|e| &e.run_id),
                "retry_state": attrs.retry_state,
            })
        }
        Some(Attributes::ChildWorkflowExecutionTerminatedEventAttributes(attrs)) => {
            serde_json::json!({
                "initiated_event_id": attrs.initiated_event_id,
                "started_event_id": attrs.started_event_id,
                "workflow_id": attrs.workflow_execution.as_ref().map(|e| &e.workflow_id),
                "run_id": attrs.workflow_execution.as_ref().map(|e| &e.run_id),
            })
        }
        _ => {
            // For unhandled events, include the event type name as a minimal detail
            serde_json::json!({
                "event_type": event_type_name(event.event_type),
            })
        }
    }
}

/// Convert proto Failure to domain FailureInfo
fn failure_to_domain(failure: proto::temporal::api::failure::v1::Failure) -> crate::domain::FailureInfo {
    crate::domain::FailureInfo {
        message: failure.message,
        failure_type: failure.failure_info.map(|info| match info {
            proto::temporal::api::failure::v1::failure::FailureInfo::ApplicationFailureInfo(_) => "ApplicationFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::TimeoutFailureInfo(_) => "TimeoutFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::CanceledFailureInfo(_) => "CanceledFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::TerminatedFailureInfo(_) => "TerminatedFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::ServerFailureInfo(_) => "ServerFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::ResetWorkflowFailureInfo(_) => "ResetWorkflowFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::ActivityFailureInfo(_) => "ActivityFailure",
            proto::temporal::api::failure::v1::failure::FailureInfo::ChildWorkflowExecutionFailureInfo(_) => "ChildWorkflowExecutionFailure",
            _ => "Unknown", // Handle any new failure types
        }).unwrap_or("Unknown").to_string(),
        stack_trace: if failure.stack_trace.is_empty() { None } else { Some(failure.stack_trace) },
        cause: failure.cause.map(|c| Box::new(failure_to_domain(*c))),
    }
}
