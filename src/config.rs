use crate::domain::InsightsConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
}

/// A named connection profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub address: String,
    pub namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// On-disk config file format (~/.tempurview/config.toml)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub insights: InsightsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct InsightsSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowlist: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<usize>,
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub temporal_address: String,
    pub temporal_namespace: String,
    pub temporal_api_key: Option<String>,
    pub refresh_interval: Duration,
    pub default_limit: u32,
    pub tick_rate: Duration,
    pub use_mock: bool,
    pub mock_workflow_count: usize,
    pub insights: InsightsConfig,
    pub active_profile: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            temporal_address: "localhost:7233".to_string(),
            temporal_namespace: "default".to_string(),
            temporal_api_key: None,
            refresh_interval: Duration::from_secs(30),
            default_limit: u32::MAX,
            tick_rate: Duration::from_millis(250),
            use_mock: false,
            mock_workflow_count: 100,
            insights: InsightsConfig::default(),
            active_profile: None,
        }
    }
}

/// Return the path to ~/.tempurview/config.toml
pub(crate) fn config_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tempurview").join("config.toml"))
}

/// Load and parse the config file, returning defaults on any error.
pub(crate) fn load_config_file() -> ConfigFile {
    let path = match config_file_path() {
        Some(p) => p,
        None => return ConfigFile::default(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return ConfigFile::default(),
    };
    match toml::from_str::<ConfigFile>(&content) {
        Ok(cf) => cf,
        Err(e) => {
            warn!("Failed to parse {}: {}", path.display(), e);
            ConfigFile::default()
        }
    }
}

/// Serialize and write the config file to disk.
pub(crate) fn save_config_file(cf: &ConfigFile) -> Result<(), ConfigError> {
    let path = config_file_path().ok_or_else(|| {
        ConfigError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine home directory",
        ))
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(cf)?;
    std::fs::write(&path, content)?;
    Ok(())
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        if let Ok(addr) = std::env::var("TEMPORAL_ADDRESS") {
            config.temporal_address = addr;
        }

        if let Ok(ns) = std::env::var("TEMPORAL_NAMESPACE") {
            config.temporal_namespace = ns;
        }

        config.temporal_api_key = std::env::var("TEMPORAL_API_KEY").ok();

        if let Ok(val) = std::env::var("TEMPORAL_TUI_REFRESH_INTERVAL") {
            config.refresh_interval = Duration::from_secs(val.parse().map_err(|_| {
                ConfigError::InvalidValue("TEMPORAL_TUI_REFRESH_INTERVAL".into(), val)
            })?);
        }

        if let Ok(val) = std::env::var("TEMPORAL_TUI_DEFAULT_LIMIT") {
            let v: u32 = val
                .parse()
                .map_err(|_| ConfigError::InvalidValue("TEMPORAL_TUI_DEFAULT_LIMIT".into(), val))?;
            config.default_limit = if v == 0 { u32::MAX } else { v };
        }

        if let Ok(val) = std::env::var("TEMPORAL_TUI_TICK_RATE") {
            config.tick_rate =
                Duration::from_millis(val.parse().map_err(|_| {
                    ConfigError::InvalidValue("TEMPORAL_TUI_TICK_RATE".into(), val)
                })?);
        }

        config.insights = load_insights_config();

        if let Ok(val) = std::env::var("TEMPURVIEW_INSIGHTS_CONCURRENCY") {
            if let Ok(c) = val.parse::<usize>() {
                config.insights.concurrency = c;
            }
        }

        Ok(config)
    }

    pub fn from_args(args: &[String]) -> Result<Self, ConfigError> {
        let mut config = Self::from_env()?;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--mock" => {
                    config.use_mock = true;
                }
                "--mock-count" => {
                    i += 1;
                    if i < args.len() {
                        config.mock_workflow_count = args[i].parse().map_err(|_| {
                            ConfigError::InvalidValue("--mock-count".into(), args[i].clone())
                        })?;
                    }
                }
                "--address" => {
                    i += 1;
                    if i < args.len() {
                        config.temporal_address = args[i].clone();
                    }
                }
                "--namespace" => {
                    i += 1;
                    if i < args.len() {
                        config.temporal_namespace = args[i].clone();
                    }
                }
                "--limit" => {
                    i += 1;
                    if i < args.len() {
                        let v: u32 = args[i].parse().map_err(|_| {
                            ConfigError::InvalidValue("--limit".into(), args[i].clone())
                        })?;
                        config.default_limit = if v == 0 { u32::MAX } else { v };
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Ok(config)
    }

    /// Build config from clap-parsed GlobalArgs, merging with env vars and profiles.
    ///
    /// Resolution priority for connection params:
    /// 1. Explicit CLI flags --address / --namespace (always win)
    /// 2. --profile <name> or TEMPURVIEW_PROFILE env var → look up in config.toml
    /// 3. default_profile from config.toml → look up that profile
    /// 4. TEMPORAL_ADDRESS / TEMPORAL_NAMESPACE / TEMPORAL_API_KEY env vars
    /// 5. Hard-coded defaults (localhost:7233, default, None)
    pub fn from_global_args(global: &crate::cli::GlobalArgs) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Load config file
        let cf = load_config_file();

        // Apply insights from config file
        let mut insights = InsightsConfig {
            allowlist: cf.insights.allowlist.clone(),
            ..InsightsConfig::default()
        };
        if let Some(c) = cf.insights.concurrency {
            insights.concurrency = c;
        }
        config.insights = insights;

        // Override insights concurrency from env
        if let Ok(val) = std::env::var("TEMPURVIEW_INSIGHTS_CONCURRENCY") {
            if let Ok(c) = val.parse::<usize>() {
                config.insights.concurrency = c;
            }
        }

        // Determine active profile: CLI flag → env var → config default
        let profile_name = global
            .profile
            .clone()
            .or_else(|| std::env::var("TEMPURVIEW_PROFILE").ok())
            .or_else(|| cf.default_profile.clone());

        // Apply profile if found
        if let Some(ref name) = profile_name {
            if let Some(profile) = cf.profiles.get(name) {
                config.temporal_address = profile.address.clone();
                config.temporal_namespace = profile.namespace.clone();
                config.temporal_api_key = profile.api_key.clone();
                config.active_profile = Some(name.clone());
            } else if global.profile.is_some() || std::env::var("TEMPURVIEW_PROFILE").is_ok() {
                // Only error if the user explicitly requested this profile
                return Err(ConfigError::ProfileNotFound(name.clone()));
            }
            // If default_profile points to a missing profile, silently fall through
        }

        // If no profile was applied, fall through to env vars
        if config.active_profile.is_none() {
            if let Ok(addr) = std::env::var("TEMPORAL_ADDRESS") {
                config.temporal_address = addr;
            }
            if let Ok(ns) = std::env::var("TEMPORAL_NAMESPACE") {
                config.temporal_namespace = ns;
            }
            config.temporal_api_key = std::env::var("TEMPORAL_API_KEY").ok();
        }

        // Apply non-connection env vars
        if let Ok(val) = std::env::var("TEMPORAL_TUI_REFRESH_INTERVAL") {
            config.refresh_interval = Duration::from_secs(val.parse().map_err(|_| {
                ConfigError::InvalidValue("TEMPORAL_TUI_REFRESH_INTERVAL".into(), val)
            })?);
        }
        if let Ok(val) = std::env::var("TEMPORAL_TUI_DEFAULT_LIMIT") {
            let v: u32 = val
                .parse()
                .map_err(|_| ConfigError::InvalidValue("TEMPORAL_TUI_DEFAULT_LIMIT".into(), val))?;
            config.default_limit = if v == 0 { u32::MAX } else { v };
        }
        if let Ok(val) = std::env::var("TEMPORAL_TUI_TICK_RATE") {
            config.tick_rate = Duration::from_millis(val.parse().map_err(|_| {
                ConfigError::InvalidValue("TEMPORAL_TUI_TICK_RATE".into(), val)
            })?);
        }

        // Apply CLI flags (always win)
        config.use_mock = global.mock;
        config.mock_workflow_count = global.mock_count;
        config.default_limit = if global.limit == 0 {
            config.default_limit // preserve env var or default
        } else {
            global.limit
        };
        if let Some(ref addr) = global.address {
            config.temporal_address = addr.clone();
        }
        if let Some(ref ns) = global.namespace {
            config.temporal_namespace = ns.clone();
        }

        Ok(config)
    }
}

fn load_insights_config() -> InsightsConfig {
    let cf = load_config_file();
    let mut config = InsightsConfig {
        allowlist: cf.insights.allowlist,
        ..InsightsConfig::default()
    };
    if let Some(c) = cf.insights.concurrency {
        config.concurrency = c;
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.temporal_namespace, "default");
        assert!(!config.use_mock);
        assert!(config.active_profile.is_none());
    }

    #[test]
    fn test_from_args_mock() {
        let args = vec!["--mock".to_string()];
        let config = Config::from_args(&args).unwrap();
        assert!(config.use_mock);
    }

    #[test]
    fn test_from_args_address() {
        let args = vec!["--address".to_string(), "custom:7233".to_string()];
        let config = Config::from_args(&args).unwrap();
        assert_eq!(config.temporal_address, "custom:7233");
    }

    #[test]
    fn test_load_config_file_missing() {
        // load_config_file gracefully returns default when file is missing
        let cf = load_config_file();
        assert!(cf.profiles.is_empty());
        assert!(cf.default_profile.is_none());
    }

    #[test]
    fn test_config_file_roundtrip() {
        let mut cf = ConfigFile {
            default_profile: Some("local".to_string()),
            ..ConfigFile::default()
        };
        cf.profiles.insert(
            "local".to_string(),
            ProfileConfig {
                address: "localhost:7233".to_string(),
                namespace: "default".to_string(),
                api_key: None,
            },
        );
        cf.profiles.insert(
            "cloud".to_string(),
            ProfileConfig {
                address: "cloud.temporal.io:7233".to_string(),
                namespace: "my-ns".to_string(),
                api_key: Some("tctl_xxx".to_string()),
            },
        );
        cf.insights.allowlist = vec!["expected error".to_string()];

        let serialized = toml::to_string_pretty(&cf).unwrap();
        let deserialized: ConfigFile = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.default_profile, Some("local".to_string()));
        assert_eq!(deserialized.profiles.len(), 2);
        assert_eq!(deserialized.profiles["local"].address, "localhost:7233");
        assert_eq!(
            deserialized.profiles["cloud"].api_key,
            Some("tctl_xxx".to_string())
        );
        assert_eq!(deserialized.insights.allowlist, vec!["expected error"]);
    }

    #[test]
    fn test_from_args_mock_count() {
        let args = vec![
            "--mock".to_string(),
            "--mock-count".to_string(),
            "500".to_string(),
        ];
        let config = Config::from_args(&args).unwrap();
        assert!(config.use_mock);
        assert_eq!(config.mock_workflow_count, 500);
    }

    #[test]
    fn test_backwards_compat_insights_only_config() {
        // A config file with only [insights] should still parse
        let toml_str = r#"
[insights]
allowlist = ["expected error"]
concurrency = 50
"#;
        let cf: ConfigFile = toml::from_str(toml_str).unwrap();
        assert!(cf.profiles.is_empty());
        assert!(cf.default_profile.is_none());
        assert_eq!(cf.insights.allowlist, vec!["expected error"]);
        assert_eq!(cf.insights.concurrency, Some(50));
    }
}
