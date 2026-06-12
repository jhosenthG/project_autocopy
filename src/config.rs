use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub last_source: Option<PathBuf>,
    pub last_dest: Option<PathBuf>,
    pub max_versions: usize,
    pub schedule_enabled: bool,
    pub schedule_time: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_source: None,
            last_dest: None,
            max_versions: 3,
            schedule_enabled: false,
            schedule_time: None,
        }
    }
}

fn get_config_path() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let app_data_path = PathBuf::from(app_data);
    let autocopy_dir = app_data_path.join("autocopy");
    autocopy_dir.join("config.json")
}

fn ensure_config_dir() -> std::io::Result<()> {
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = get_config_path();

        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Warning: failed to parse config, using defaults: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("Warning: failed to read config file, using defaults: {}", e);
                }
            }
        }

        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        ensure_config_dir()?;
        let config_path = get_config_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.max_versions, 3);
        assert!(!config.schedule_enabled);
        assert!(config.schedule_time.is_none());
    }
}
