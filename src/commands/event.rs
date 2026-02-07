use crate::cli::EventAction;
use crate::client::TemporalClient;
use crate::output::{self, OutputFormat};
use std::io::Write;

/// Handle event commands, printing to stdout.
pub async fn handle(
    action: EventAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
) -> color_eyre::Result<()> {
    handle_to(action, client, format, &mut std::io::stdout()).await
}

/// Handle event commands, writing output to the given writer.
pub async fn handle_to(
    action: EventAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
    w: &mut (dyn Write + Send),
) -> color_eyre::Result<()> {
    match action {
        EventAction::List {
            workflow_id,
            run_id,
        } => {
            let events = client
                .get_history(&workflow_id, run_id.as_deref())
                .await?;
            output::write_list(&events, &events, format, w);
        }
    }
    Ok(())
}
