use crate::cli::ActivityAction;
use crate::client::TemporalClient;
use crate::domain::correlate_activities;
use crate::output::{self, OutputFormat};

pub async fn handle(
    action: ActivityAction,
    client: &dyn TemporalClient,
    format: OutputFormat,
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
            output::print_list(&activities, &activities, format);
        }
    }
    Ok(())
}
