use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::error::{PolyrcError, Result};

const CONFIG_FILE: &str = "polyrc/config.toml";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub store: StoreConfig,

    /// Preferred editor command (e.g. "code", "zed", "vim").
    /// Falls back to $EDITOR env var, then OS default, when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_editor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Path to the local store git repo. Defaults to ~/.polyrc/store
    pub path: Option<String>,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self { path: None }
    }
}

impl Config {
    /// Load config from ~/.config/polyrc/config.toml.
    /// Returns a default config if the file does not exist.
    pub fn load() -> Result<Self> {
        let path = config_file_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| PolyrcError::Io {
            path: path.clone(),
            source: e,
        })?;
        toml::from_str(&raw).map_err(|e| PolyrcError::TomlParse { path, source: e })
    }

    /// Save config to ~/.config/polyrc/config.toml.
    pub fn save(&self) -> Result<()> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PolyrcError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| PolyrcError::ConfigError {
            msg: format!("failed to serialize config: {e}"),
        })?;
        std::fs::write(&path, content).map_err(|e| PolyrcError::Io { path, source: e })
    }

    /// Resolve the store path from config, falling back to ~/.polyrc/store.
    pub fn store_path(&self) -> PathBuf {
        if let Some(p) = &self.store.path {
            let expanded = expand_tilde(p);
            return PathBuf::from(expanded);
        }
        default_store_path()
    }

    /// Set the store path and save config.
    pub fn set_store_path(&mut self, path: &Path) -> Result<()> {
        self.store.path = Some(path.to_string_lossy().to_string());
        self.save()
    }
}

fn config_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_FILE)
}

pub fn default_store_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".polyrc/store")
}

fn expand_tilde(p: &str) -> String {
    if p.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), &p[2..]);
        }
    }
    p.to_string()
}
