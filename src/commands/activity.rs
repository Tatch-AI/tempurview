use crate::cli::ActivityAction;
use crate::client::TemporalClient;
use crate::domain::correlate_activities;
use crate::output::{self, OutputFormat};
use std::io::Write;

/// Handle activity commands, printing to stdout.
pub async fn handle(
    action: ActivityAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
) -> color_eyre::Result<()> {
    handle_to(action, client, format, &mut std::io::stdout()).await
}

/// Handle activity commands, writing output to the given writer.
pub async fn handle_to(
    action: ActivityAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
    w: &mut (dyn Write + Send),
) -> color_eyre::Result<()> {
    match action {
        ActivityAction::List {
            workflow_id,
            run_id,
        } => {
            let events = client
                .get_history(&workflow_id, run_id.as_deref())
                .await?;
            let activities = correlate_activities(&events);
            output::write_list(&activities, &activities, format, w);
        }
    }
    Ok(())
}
