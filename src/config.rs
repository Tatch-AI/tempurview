use crate::domain::InsightsConfig;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Invalid value for {0}: {1}")]
    InvalidValue(String, String),
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            temporal_address: "localhost:7233".to_string(),
            temporal_namespace: "default".to_string(),
            temporal_api_key: None,
            refresh_interval: Duration::from_secs(30),
            default_limit: 50,
            tick_rate: Duration::from_millis(250),
            use_mock: false,
            mock_workflow_count: 100,
            insights: InsightsConfig::default(),
        }
    }
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
            config.default_limit = val
                .parse()
                .map_err(|_| ConfigError::InvalidValue("TEMPORAL_TUI_DEFAULT_LIMIT".into(), val))?;
        }

        if let Ok(val) = std::env::var("TEMPORAL_TUI_TICK_RATE") {
            config.tick_rate =
                Duration::from_millis(val.parse().map_err(|_| {
                    ConfigError::InvalidValue("TEMPORAL_TUI_TICK_RATE".into(), val)
                })?);
        }

        config.insights = load_insights_config();

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
                        config.default_limit = args[i].parse().map_err(|_| {
                            ConfigError::InvalidValue("--limit".into(), args[i].clone())
                        })?;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Ok(config)
    }
}

#[derive(Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    insights: InsightsSection,
}

#[derive(Deserialize, Default)]
struct InsightsSection {
    #[serde(default)]
    allowlist: Vec<String>,
}

fn load_insights_config() -> InsightsConfig {
    let config_dir = dirs::home_dir().map(|h| h.join(".tempurview"));
    let path = match config_dir {
        Some(d) => d.join("config.toml"),
        None => return InsightsConfig::default(),
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return InsightsConfig::default(),
    };

    match toml::from_str::<ConfigFile>(&content) {
        Ok(cf) => InsightsConfig {
            allowlist: cf.insights.allowlist,
        },
        Err(e) => {
            warn!("Failed to parse {}: {}", path.display(), e);
            InsightsConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.temporal_namespace, "default");
        assert!(!config.use_mock);
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
    fn test_load_insights_config_missing_file() {
        // load_insights_config gracefully returns default when file is missing
        let config = load_insights_config();
        assert!(config.allowlist.is_empty());
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
}
