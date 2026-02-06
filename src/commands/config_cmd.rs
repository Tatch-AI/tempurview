use crate::cli::ConfigAction;
use crate::config::Config;
use crate::output::{OutputFormat, TableDisplay};

pub fn handle(action: ConfigAction, config: &Config, format: OutputFormat) {
    match action {
        ConfigAction::Show => {
            match format {
                OutputFormat::Json => {
                    // Manually build a JSON object for config since Config doesn't derive Serialize
                    let json = serde_json::json!({
                        "temporal_address": config.temporal_address,
                        "temporal_namespace": config.temporal_namespace,
                        "api_key_set": config.temporal_api_key.is_some(),
                        "default_limit": config.default_limit,
                        "mock_mode": config.use_mock,
                        "mock_workflow_count": config.mock_workflow_count,
                        "insights_allowlist": config.insights.allowlist,
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Table => {
                    let table = config.to_table();
                    println!("{table}");
                }
            }
        }
    }
}
