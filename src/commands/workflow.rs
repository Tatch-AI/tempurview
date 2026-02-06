use crate::cli::WorkflowAction;
use crate::client::TemporalClient;
use crate::domain::{parse_date_input, WorkflowFilter, WorkflowStatus};
use crate::output::{self, OutputFormat};

pub async fn handle(
    action: WorkflowAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
    limit: u32,
) -> color_eyre::Result<()> {
    match action {
        WorkflowAction::List {
            status,
            workflow_type,
            since,
            before,
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

            let workflows = client.list(&filter, limit).await?;
            output::print_list(&workflows, &workflows, format);
        }
        WorkflowAction::Get {
            workflow_id,
            run_id,
        } => {
            let detail = client.describe(&workflow_id, run_id.as_deref()).await?;
            output::print_output(&detail, format);
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
            output::print_count(count, format);
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
