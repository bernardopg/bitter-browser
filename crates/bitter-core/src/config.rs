use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub appearance: AppearanceConfig,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    pub default_search_engine: String,
    pub restore_session: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppearanceConfig {
    pub theme: String,
    pub accent_color: String,
    pub sidebar_position: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivacyConfig {
    pub block_ads: bool,
    pub block_trackers: bool,
    pub https_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                default_search_engine: "DuckDuckGo".to_string(),
                restore_session: true,
            },
            appearance: AppearanceConfig {
                theme: "system".to_string(),
                accent_color: "#ff4d00".to_string(),
                sidebar_position: "left".to_string(),
            },
            privacy: PrivacyConfig {
                block_ads: true,
                block_trackers: true,
                https_only: true,
            },
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if let Ok(contents) = fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str(&contents) {
                return config;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self).unwrap();
        fs::write(config_path, contents)
    }

    fn config_path() -> PathBuf {
        let mut path = crate::paths::config_dir();
        path.push("config.toml");
        path
    }
}
