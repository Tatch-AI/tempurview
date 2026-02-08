use crate::cli::InsightAction;
use crate::client::TemporalClient;
use crate::config::Config;
use crate::domain::{parse_date_input, run_insights_scan, WorkflowFilter};
use crate::output::{self, OutputFormat};
use std::io::Write;
use std::sync::Arc;

/// Handle insight commands, printing to stdout.
pub async fn handle(
    action: InsightAction,
    client: Arc<dyn TemporalClient>,
    format: OutputFormat,
    limit: u32,
    config: &Config,
) -> color_eyre::Result<()> {
    handle_to(
        action,
        client,
        format,
        limit,
        config,
        &mut std::io::stdout(),
    )
    .await
}

/// Handle insight commands, writing output to the given writer.
pub async fn handle_to(
    action: InsightAction,
    client: Arc<dyn TemporalClient>,
    format: OutputFormat,
    limit: u32,
    config: &Config,
    w: &mut (dyn Write + Send),
) -> color_eyre::Result<()> {
    match action {
        InsightAction::Scan { since, before } => {
            let mut filter = WorkflowFilter::new();
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

            let result =
                run_insights_scan(client, &filter, limit, &config.insights, None, 0).await?;
            output::write_output(&result, format, w);
        }
    }
    Ok(())
}
