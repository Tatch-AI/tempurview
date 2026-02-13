use crate::cli::ConfigAction;
use crate::config::{load_config_file, save_config_file, Config, ProfileConfig};
use crate::output::{OutputFormat, TableDisplay};
use comfy_table::{presets::UTF8_FULL_CONDENSED, ContentArrangement, Table};

pub fn handle(action: ConfigAction, config: &Config, format: OutputFormat) {
    match action {
        ConfigAction::Show => {
            match format {
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "active_profile": config.active_profile,
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
        ConfigAction::ProfileAdd {
            name,
            address,
            namespace,
            api_key,
        } => {
            handle_profile_add(name, address, namespace, api_key);
        }
        ConfigAction::ProfileList => {
            handle_profile_list();
        }
        ConfigAction::ProfileRemove { name } => {
            handle_profile_remove(name);
        }
        ConfigAction::SetDefault { name } => {
            handle_set_default(name);
        }
    }
}

fn handle_profile_add(
    name: String,
    address: String,
    namespace: String,
    api_key: Option<String>,
) {
    let mut cf = load_config_file();
    let is_first = cf.profiles.is_empty();
    cf.profiles.insert(
        name.clone(),
        ProfileConfig {
            address,
            namespace,
            api_key,
        },
    );
    // Auto-set as default if it's the first profile added
    if is_first && cf.default_profile.is_none() {
        cf.default_profile = Some(name.clone());
        println!("Added profile '{}' (set as default)", name);
    } else {
        println!("Added profile '{}'", name);
    }
    if let Err(e) = save_config_file(&cf) {
        eprintln!("Failed to save config: {}", e);
    }
}

fn handle_profile_list() {
    let cf = load_config_file();
    if cf.profiles.is_empty() {
        println!("No profiles configured.");
        println!();
        println!("Add one with:");
        println!("  tempurview config profile-add <name> --address <addr> --namespace <ns>");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["NAME", "ADDRESS", "NAMESPACE", "API KEY", "DEFAULT"]);

    for (name, profile) in &cf.profiles {
        let is_default = cf.default_profile.as_deref() == Some(name.as_str());
        table.add_row(vec![
            name.as_str(),
            &profile.address,
            &profile.namespace,
            if profile.api_key.is_some() {
                "(set)"
            } else {
                ""
            },
            if is_default { "*" } else { "" },
        ]);
    }
    println!("{table}");
}

fn handle_profile_remove(name: String) {
    let mut cf = load_config_file();
    if cf.profiles.remove(&name).is_none() {
        eprintln!("Profile '{}' does not exist.", name);
        if !cf.profiles.is_empty() {
            eprintln!(
                "Available profiles: {}",
                cf.profiles
                    .keys()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        std::process::exit(1);
    }
    // Clear default if it pointed to the removed profile
    if cf.default_profile.as_deref() == Some(name.as_str()) {
        cf.default_profile = None;
    }
    if let Err(e) = save_config_file(&cf) {
        eprintln!("Failed to save config: {}", e);
        std::process::exit(1);
    }
    println!("Removed profile '{}'", name);
}

fn handle_set_default(name: String) {
    let mut cf = load_config_file();
    if !cf.profiles.contains_key(&name) {
        eprintln!("Profile '{}' does not exist.", name);
        if !cf.profiles.is_empty() {
            eprintln!(
                "Available profiles: {}",
                cf.profiles
                    .keys()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        std::process::exit(1);
    }
    cf.default_profile = Some(name.clone());
    if let Err(e) = save_config_file(&cf) {
        eprintln!("Failed to save config: {}", e);
        std::process::exit(1);
    }
    println!("Default profile set to '{}'", name);
}
