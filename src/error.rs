use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PolyrcError>;

#[derive(Debug, Error)]
pub enum PolyrcError {
    #[error("IO error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("YAML parse error in {path}: {source}")]
    YamlParse {
        path: PathBuf,
        #[source]
        source: serde_yml::Error,
    },

    #[error("Unknown format: '{0}'. Use `polyrc supported-formats` to see valid formats.")]
    UnknownFormat(String),

    #[error("Cannot write to {path}: {reason}")]
    WriteFailure { path: PathBuf, reason: String },

    #[error("Store not found. Run `polyrc init` first.")]
    StoreNotFound,

    #[error("Git error: {msg}")]
    GitError { msg: String },

    #[error("Config error: {msg}")]
    ConfigError { msg: String },

    #[error("TOML parse error in {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}
