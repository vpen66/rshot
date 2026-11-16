use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub hotkeys: Hotkeys,
    #[serde(default)]
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_check_on_startup")]
    pub check_on_startup: bool,
}

fn default_check_on_startup() -> bool {
    true
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            check_on_startup: default_check_on_startup(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotkeys {
    #[serde(default = "default_capture")]
    pub capture: String,
    #[serde(default = "default_fullscreen")]
    pub fullscreen: String,
}

fn default_capture() -> String {
    if cfg!(target_os = "macos") {
        "Cmd+Option+A".to_string()
    } else {
        "Ctrl+Alt+A".to_string()
    }
}

fn default_fullscreen() -> String {
    if cfg!(target_os = "macos") {
        "Cmd+Option+F".to_string()
    } else {
        "Ctrl+Alt+F".to_string()
    }
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            capture: default_capture(),
            fullscreen: default_fullscreen(),
        }
    }
}

/// macOS modifier names map to the Win key on Windows, where those combos are
/// shell-reserved or already owned; rewrite them to Ctrl/Alt off macOS.
fn platform_hotkey(text: &str) -> String {
    if cfg!(target_os = "macos") {
        text.to_string()
    } else {
        rewrite_macos_modifiers(text)
    }
}

fn rewrite_macos_modifiers(text: &str) -> String {
    text.split('+')
        .map(|tok| match tok.trim().to_ascii_lowercase().as_str() {
            "cmd" | "command" => "Ctrl".to_string(),
            "option" => "Alt".to_string(),
            _ => tok.trim().to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
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
                Ok(cfg) => {
                    let mut cfg = cfg;
                    cfg.migrate_platform_hotkeys();
                    cfg
                }
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

    /// Rewrites legacy macOS-style bindings in an existing config so Windows /
    /// Linux don't keep trying to register Win+Alt combos that fail at startup.
    fn migrate_platform_hotkeys(&mut self) {
        let capture = platform_hotkey(&self.hotkeys.capture);
        let fullscreen = platform_hotkey(&self.hotkeys.fullscreen);
        if capture != self.hotkeys.capture || fullscreen != self.hotkeys.fullscreen {
            self.hotkeys.capture = capture;
            self.hotkeys.fullscreen = fullscreen;
            if let Err(e) = self.save() {
                log::warn!("Failed to migrate hotkeys to platform modifiers: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_modifiers_rewrite_to_ctrl_alt() {
        assert_eq!(rewrite_macos_modifiers("Cmd+Option+F"), "Ctrl+Alt+F");
        assert_eq!(rewrite_macos_modifiers("Cmd+Option+A"), "Ctrl+Alt+A");
        assert_eq!(rewrite_macos_modifiers("Cmd + Option + F"), "Ctrl+Alt+F");
    }

    #[test]
    fn non_macos_modifiers_are_preserved() {
        assert_eq!(rewrite_macos_modifiers("Ctrl+Alt+F"), "Ctrl+Alt+F");
        assert_eq!(rewrite_macos_modifiers("Ctrl+Shift+A"), "Ctrl+Shift+A");
        assert_eq!(rewrite_macos_modifiers("Alt+F"), "Alt+F");
    }
}
