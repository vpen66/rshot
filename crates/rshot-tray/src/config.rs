use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub hotkeys: Hotkeys,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotkeys {
    #[serde(default = "default_capture")]
    pub capture: String,
    #[serde(default = "default_fullscreen")]
    pub fullscreen: String,
}

fn default_capture() -> String {
    "Cmd+Option+A".to_string()
}

fn default_fullscreen() -> String {
    "Cmd+Option+F".to_string()
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            capture: default_capture(),
            fullscreen: default_fullscreen(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkeys: Hotkeys::default(),
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rshot")
        .join("config.toml")
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!(
                        "Failed to parse config {}: {e}; using defaults",
                        path.display()
                    );
                    Config::default()
                }
            },
            Err(_) => {
                let cfg = Config::default();
                if let Err(e) = cfg.save() {
                    log::warn!("Failed to write default config: {e}");
                }
                cfg
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&path, text)
    }
}