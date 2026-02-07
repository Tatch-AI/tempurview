use crate::cli::{SortOrder, WorkflowAction};
use crate::client::TemporalClient;
use crate::domain::{parse_date_input, WorkflowFilter, WorkflowStatus};
use crate::output::{self, OutputFormat};
use std::io::Write;

/// Handle workflow commands, printing to stdout.
pub async fn handle(
    action: WorkflowAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
    limit: u32,
) -> color_eyre::Result<()> {
    handle_to(action, client, format, limit, &mut std::io::stdout()).await
}

/// Handle workflow commands, writing output to the given writer.
pub async fn handle_to(
    action: WorkflowAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
    limit: u32,
    w: &mut (dyn Write + Send),
) -> color_eyre::Result<()> {
    match action {
        WorkflowAction::List {
            status,
            workflow_type,
            since,
            before,
            sort,
        } => {
            let mut filter = WorkflowFilter::new();
            if let Some(s) = status {
                let ws: WorkflowStatus = s
                    .parse()
                    .map_err(|_| color_eyre::eyre::eyre!("Invalid status: {s}"))?;
                filter = filter.with_status(ws);
            }
            if let Some(t) = workflow_type {
                filter = filter.with_type(t);
            }
            if let Some(ref s) = since {
                let dt = parse_date_input(s)
                    .ok_or_else(|| color_eyre::eyre::eyre!("Invalid --since value: {s}"))?;
                filter = filter.with_start_time_after(dt);
            }
            if let Some(ref b) = before {
                let dt = parse_date_input(b)
                    .ok_or_else(|| color_eyre::eyre::eyre!("Invalid --before value: {b}"))?;
                filter = filter.with_start_time_before(dt);
            }
            let mut workflows = client.list(&filter, limit).await?;

            // Sort client-side by start_time
            match sort {
                SortOrder::Asc => workflows.sort_by(|a, b| a.start_time.cmp(&b.start_time)),
                SortOrder::Desc => workflows.sort_by(|a, b| b.start_time.cmp(&a.start_time)),
            }

            output::write_list(&workflows, &workflows, format, w);
        }
        WorkflowAction::Get {
            workflow_id,
            run_id,
        } => {
            let detail = client.describe(&workflow_id, run_id.as_deref()).await?;
            output::write_output(&detail, format, w);
        }
        WorkflowAction::Count { status, query } => {
            let q = if let Some(s) = status {
                let ws: WorkflowStatus = s
                    .parse()
                    .map_err(|_| color_eyre::eyre::eyre!("Invalid status: {s}"))?;
                Some(format!("ExecutionStatus='{}'", ws.as_query_value()))
            } else {
                query
            };
            let count = client.count(q.as_deref()).await?;
            output::write_count(count, format, w);
        }
        WorkflowAction::Cancel {
            workflow_id,
            run_id,
        } => {
            client.cancel(&workflow_id, run_id.as_deref()).await?;
            eprintln!("Workflow {workflow_id} cancelled.");
        }
        WorkflowAction::Terminate {
            workflow_id,
            run_id,
            reason,
        } => {
            client
                .terminate(&workflow_id, run_id.as_deref(), &reason)
                .await?;
            eprintln!("Workflow {workflow_id} terminated.");
        }
    }
    Ok(())
}
