use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::error::{PolyrcError, Result};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub store: StoreConfig,
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
    /// Load config from ~/.polyrc/config.toml.
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

    /// Save config to ~/.polyrc/config.toml.
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

/// Root directory for all polyrc data and config: ~/polyrc/
pub fn polyrc_dir() -> PathBuf {
    home_dir().join("polyrc")
}

fn config_file_path() -> PathBuf {
    polyrc_dir().join("config.toml")
}

pub fn default_store_path() -> PathBuf {
    polyrc_dir().join("store")
}

/// Resolve the user's home directory.
///
/// Uses the `HOME` environment variable on Unix (the shell's value), falling
/// back to `dirs::home_dir()` (passwd lookup) if it is unset or empty.
pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn expand_tilde(p: &str) -> String {
    if p.starts_with("~/") {
        return format!("{}/{}", home_dir().display(), &p[2..]);
    }
    p.to_string()
}
