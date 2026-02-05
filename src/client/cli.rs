use super::{ClientError, ClientResult, TemporalClient};
use crate::domain::{
    FailureInfo, HistoryEvent, WorkflowDetail, WorkflowFilter, WorkflowStatus, WorkflowSummary,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::process::Stdio;
use tokio::process::Command;

/// Implementation that shells out to `temporal` CLI
pub struct CliTemporalClient {
    address: String,
    namespace: String,
    api_key: Option<String>,
}

impl CliTemporalClient {
    pub fn new(address: String, namespace: String, api_key: Option<String>) -> Self {
        Self {
            address,
            namespace,
            api_key,
        }
    }

    pub fn from_env() -> ClientResult<Self> {
        let address = std::env::var("TEMPORAL_ADDRESS")
            .map_err(|_| ClientError::ConfigError("TEMPORAL_ADDRESS not set".into()))?;
        let namespace = std::env::var("TEMPORAL_NAMESPACE")
            .map_err(|_| ClientError::ConfigError("TEMPORAL_NAMESPACE not set".into()))?;
        let api_key = std::env::var("TEMPORAL_API_KEY").ok();

        Ok(Self::new(address, namespace, api_key))
    }

    fn base_args(&self) -> Vec<String> {
        let mut args = vec![
            "--address".to_string(),
            self.address.clone(),
            "--namespace".to_string(),
            self.namespace.clone(),
        ];

        if let Some(api_key) = &self.api_key {
            args.push("--api-key".to_string());
            args.push(api_key.clone());
        }

        args
    }

    async fn run_command(&self, subcommand: &[&str]) -> ClientResult<String> {
        let mut args = self.base_args();
        args.extend(subcommand.iter().map(|s| s.to_string()));

        let output = Command::new("temporal")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| ClientError::CommandFailed(format!("Failed to execute temporal CLI: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ClientError::CommandFailed(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl TemporalClient for CliTemporalClient {
    async fn count(&self, query: Option<&str>) -> ClientResult<u64> {
        let mut args = vec!["workflow", "count"];
        if let Some(q) = query {
            args.extend(["--query", q]);
        }
        let output = self.run_command(&args).await?;
        parse_count_output(&output)
    }

    async fn list(&self, filter: &WorkflowFilter, limit: u32) -> ClientResult<Vec<WorkflowSummary>> {
        let limit_str = limit.to_string();
        let mut args = vec!["workflow", "list", "--output", "json", "--limit", &limit_str];
        let query = filter.to_query();

        if let Some(ref q) = query {
            args.extend(["--query", q]);
        }

        let output = self.run_command(&args).await?;
        parse_workflow_list(&output)
    }

    async fn describe(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<WorkflowDetail> {
        let mut args = vec!["workflow", "describe", "--workflow-id", workflow_id, "--output", "json"];
        if let Some(rid) = run_id {
            args.extend(["--run-id", rid]);
        }

        let output = self.run_command(&args).await?;
        parse_workflow_detail(&output)
    }

    async fn get_history(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<Vec<HistoryEvent>> {
        let mut args = vec!["workflow", "show", "--workflow-id", workflow_id, "--output", "json"];
        if let Some(rid) = run_id {
            args.extend(["--run-id", rid]);
        }

        let output = self.run_command(&args).await?;
        parse_history_events(&output)
    }

    async fn cancel(&self, workflow_id: &str, run_id: Option<&str>) -> ClientResult<()> {
        let mut args = vec!["workflow", "cancel", "--workflow-id", workflow_id];
        if let Some(rid) = run_id {
            args.extend(["--run-id", rid]);
        }

        self.run_command(&args).await?;
        Ok(())
    }

    async fn terminate(&self, workflow_id: &str, run_id: Option<&str>, reason: &str) -> ClientResult<()> {
        let mut args = vec!["workflow", "terminate", "--workflow-id", workflow_id, "--reason", reason];
        if let Some(rid) = run_id {
            args.extend(["--run-id", rid]);
        }

        self.run_command(&args).await?;
        Ok(())
    }
}

/// Pure function: parse count output
pub fn parse_count_output(output: &str) -> ClientResult<u64> {
    // Temporal CLI count output format can vary
    // Try to find a number in the output
    for line in output.lines() {
        // Try parsing the whole line as a number first
        if let Ok(n) = line.trim().parse::<u64>() {
            return Ok(n);
        }

        // Look for "count" or "total" followed by a number
        let line_lower = line.to_lowercase();
        if line_lower.contains("count") || line_lower.contains("total") {
            for word in line.split_whitespace() {
                if let Ok(n) = word.trim_matches(|c: char| !c.is_ascii_digit()).parse::<u64>() {
                    return Ok(n);
                }
            }
        }
    }

    // Try parsing as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
        if let Some(count) = json.get("count").and_then(|v| v.as_u64()) {
            return Ok(count);
        }
    }

    Err(ClientError::ParseError(format!(
        "Could not parse count from output: {}",
        output
    )))
}

/// Pure function: parse workflow list JSON
pub fn parse_workflow_list(json: &str) -> ClientResult<Vec<WorkflowSummary>> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ClientError::ParseError(format!("Invalid JSON: {}", e)))?;

    let executions = if value.is_array() {
        value.as_array().unwrap()
    } else if let Some(arr) = value.get("executions").and_then(|v| v.as_array()) {
        arr
    } else {
        return Err(ClientError::ParseError("Expected array or object with executions field".into()));
    };

    let mut workflows = Vec::with_capacity(executions.len());

    for exec in executions {
        let workflow = parse_workflow_execution(exec)?;
        workflows.push(workflow);
    }

    Ok(workflows)
}

fn parse_workflow_execution(exec: &serde_json::Value) -> ClientResult<WorkflowSummary> {
    // Handle nested execution object
    let execution = exec.get("execution").unwrap_or(exec);

    let workflow_id = execution
        .get("workflowId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ClientError::ParseError("Missing workflowId".into()))?
        .to_string();

    let run_id = execution
        .get("runId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let workflow_type = exec
        .get("type")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .or_else(|| exec.get("workflowType").and_then(|v| v.as_str()))
        .unwrap_or("Unknown")
        .to_string();

    let status_str = exec
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("RUNNING");

    let status = WorkflowStatus::from_str(status_str)
        .unwrap_or(WorkflowStatus::Running);

    let start_time = exec
        .get("startTime")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let close_time = exec
        .get("closeTime")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let task_queue = exec
        .get("taskQueue")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    Ok(WorkflowSummary {
        workflow_id,
        run_id,
        workflow_type,
        status,
        start_time,
        close_time,
        task_queue,
    })
}

fn parse_workflow_detail(json: &str) -> ClientResult<WorkflowDetail> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ClientError::ParseError(format!("Invalid JSON: {}", e)))?;

    let summary = parse_workflow_execution(&value)?;

    let input = value
        .get("input")
        .or_else(|| value.get("workflowExecutionInfo").and_then(|v| v.get("input")))
        .cloned();

    let output = value
        .get("output")
        .or_else(|| value.get("result"))
        .cloned();

    let failure = value
        .get("failure")
        .and_then(parse_failure_info);

    let history_length = value
        .get("historyLength")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let memo = value
        .get("memo")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();

    let search_attributes = value
        .get("searchAttributes")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();

    Ok(WorkflowDetail {
        summary,
        input,
        output,
        failure,
        history_length,
        memo,
        search_attributes,
    })
}

fn parse_failure_info(value: &serde_json::Value) -> Option<FailureInfo> {
    let message = value.get("message").and_then(|v| v.as_str())?.to_string();

    let failure_type = value
        .get("failureType")
        .or_else(|| value.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let stack_trace = value
        .get("stackTrace")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let cause = value
        .get("cause")
        .and_then(parse_failure_info)
        .map(Box::new);

    Some(FailureInfo {
        message,
        failure_type,
        stack_trace,
        cause,
    })
}

fn parse_history_events(json: &str) -> ClientResult<Vec<HistoryEvent>> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| ClientError::ParseError(format!("Invalid JSON: {}", e)))?;

    let events = value
        .get("events")
        .and_then(|v| v.as_array())
        .or_else(|| value.as_array())
        .ok_or_else(|| ClientError::ParseError("Expected events array".into()))?;

    let mut result = Vec::with_capacity(events.len());

    for event in events {
        let event_id = event
            .get("eventId")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let event_type = event
            .get("eventType")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let timestamp = event
            .get("eventTime")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        result.push(HistoryEvent {
            event_id,
            event_type,
            timestamp,
            details: event.clone(),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_count_output_number() {
        assert_eq!(parse_count_output("42").unwrap(), 42);
        assert_eq!(parse_count_output("  42  ").unwrap(), 42);
    }

    #[test]
    fn test_parse_count_output_with_label() {
        assert_eq!(parse_count_output("Count: 42").unwrap(), 42);
        assert_eq!(parse_count_output("Total: 100").unwrap(), 100);
    }

    #[test]
    fn test_parse_count_output_json() {
        assert_eq!(parse_count_output(r#"{"count": 42}"#).unwrap(), 42);
    }

    #[test]
    fn test_parse_workflow_list() {
        let json = r#"[
            {
                "execution": {"workflowId": "wf-1", "runId": "run-1"},
                "type": {"name": "TestWorkflow"},
                "status": "RUNNING",
                "startTime": "2024-01-01T00:00:00Z",
                "taskQueue": "default"
            }
        ]"#;

        let workflows = parse_workflow_list(json).unwrap();
        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].workflow_id, "wf-1");
        assert_eq!(workflows[0].workflow_type, "TestWorkflow");
        assert_eq!(workflows[0].status, WorkflowStatus::Running);
    }

    #[test]
    fn test_parse_workflow_list_with_wrapper() {
        let json = r#"{
            "executions": [
                {
                    "execution": {"workflowId": "wf-2", "runId": "run-2"},
                    "type": {"name": "OtherWorkflow"},
                    "status": "COMPLETED",
                    "startTime": "2024-01-01T00:00:00Z",
                    "closeTime": "2024-01-01T01:00:00Z",
                    "taskQueue": "other-queue"
                }
            ]
        }"#;

        let workflows = parse_workflow_list(json).unwrap();
        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].workflow_id, "wf-2");
        assert_eq!(workflows[0].status, WorkflowStatus::Completed);
        assert!(workflows[0].close_time.is_some());
    }
}
