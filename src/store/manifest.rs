use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::error::{PolyrcError, Result};

const MANIFEST_FILE: &str = "polyrc.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub store: StoreSection,
    #[serde(default)]
    pub remote: RemoteSection,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoreSection {
    /// Format version for migration handling.
    pub version: String,
    /// RFC3339 timestamp of store creation.
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RemoteSection {
    /// Optional git remote URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            store: StoreSection {
                version: "1".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            remote: RemoteSection::default(),
        }
    }

    pub fn load(store_dir: &Path) -> Result<Self> {
        let path = store_dir.join(MANIFEST_FILE);
        let raw = std::fs::read_to_string(&path).map_err(|e| PolyrcError::Io {
            path: path.clone(),
            source: e,
        })?;
        toml::from_str(&raw).map_err(|e| PolyrcError::TomlParse { path, source: e })
    }

    pub fn save(&self, store_dir: &Path) -> Result<()> {
        let path = store_dir.join(MANIFEST_FILE);
        let content = toml::to_string_pretty(self).map_err(|e| PolyrcError::ConfigError {
            msg: format!("failed to serialize manifest: {e}"),
        })?;
        std::fs::write(&path, content).map_err(|e| PolyrcError::Io { path, source: e })
    }

    pub fn set_remote_url(&mut self, url: impl Into<String>) {
        self.remote.url = Some(url.into());
    }
}
