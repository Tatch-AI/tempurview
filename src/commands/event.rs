use crate::cli::EventAction;
use crate::client::TemporalClient;
use crate::output::{self, OutputFormat};

pub async fn handle(
    action: EventAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
) -> color_eyre::Result<()> {
    match action {
        EventAction::List {
            workflow_id,
            run_id,
        } => {
            let events = client
                .get_history(&workflow_id, run_id.as_deref())
                .await?;
            output::print_list(&events, &events, format);
        }
    }
    Ok(())
}
