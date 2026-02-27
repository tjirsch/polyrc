use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::error::{PolyrcError, Result};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub store: StoreConfig,

    /// Preferred editor command (e.g. "code", "zed", "vim").
    /// Falls back to $EDITOR env var, then OS default, when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_editor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StoreConfig {
    /// Path to the local store git repo. Defaults to ~/polyrc/store.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Store format version — set on `polyrc init`, absent if not yet initialised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// RFC3339 timestamp of store creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Optional git remote URL for sync.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
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

    /// Returns true if the store has been initialised (version is set).
    pub fn store_initialized(&self) -> bool {
        self.store.version.is_some()
    }

    /// Mark the store as initialised with version + timestamp, and optionally set remote URL.
    pub fn init_store_config(&mut self, remote_url: Option<&str>) {
        self.store.version = Some("1".to_string());
        self.store.created_at = Some(chrono::Utc::now().to_rfc3339());
        if let Some(url) = remote_url {
            self.store.remote_url = Some(url.to_string());
        }
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
