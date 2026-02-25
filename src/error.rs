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

    #[error("Unknown format: '{0}'. Use `polyrc list-formats` to see valid formats.")]
    UnknownFormat(String),

    #[error("No rules found when parsing {path}")]
    NoRulesFound { path: PathBuf },

    #[error("Rule '{name}' exceeds Windsurf limit of {limit} characters ({actual} actual)")]
    WindsurfCharLimit { name: String, limit: usize, actual: usize },

    #[error("Cannot write to {path}: {reason}")]
    WriteFailure { path: PathBuf, reason: String },
}
