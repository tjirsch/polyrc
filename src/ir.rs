use serde::{Deserialize, Serialize};

/// The canonical scope of a rule in the interlingua.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    #[default]
    Project,
    Path,
}

/// The activation mode of a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    /// Always injected into context.
    #[default]
    Always,
    /// Injected when a file matching any glob in `globs` is open/edited.
    Glob,
    /// User must manually invoke (slash command, @mention, etc.).
    OnDemand,
    /// AI decides whether to load based on `description`.
    AiDecides,
}

/// A single rule in the polyrc intermediate representation.
///
/// Core fields (scope, activation, globs, name, description, content) are used by
/// all format parsers and writers. Metadata fields (id, project, source_format,
/// created_at, updated_at, store_version) are only populated when rules pass through
/// the store; format writers ignore them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Rule {
    // --- Core IR fields ---
    pub scope: Scope,
    pub activation: Activation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub globs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Raw markdown content — opaque, not parsed by polyrc.
    pub content: String,

    // --- Store metadata (populated by push-format; ignored by format writers) ---
    /// Stable UUIDv4 identifier assigned on first push to the store.
    #[serde(default)]
    pub id: String,
    /// Project key in the store (e.g. "myapp", "_user").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The format that last wrote this rule (e.g. "cursor", "claude").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_format: Option<String>,
    /// RFC3339 timestamp of first push.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// RFC3339 timestamp of last push.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Store format version for future migration handling.
    #[serde(default = "default_store_version")]
    pub store_version: String,
}

fn default_store_version() -> String {
    "1".to_string()
}

impl Rule {
    /// Returns a stable filename stem for use in multi-file format writers.
    pub fn filename_stem(&self) -> String {
        match &self.name {
            Some(n) => sanitize_filename(n),
            None => format!("rule_{:08x}", fnv1a(self.content.as_bytes())),
        }
    }

    /// Returns true if this rule has been assigned a store identity.
    pub fn has_store_id(&self) -> bool {
        !self.id.is_empty()
    }
}

pub(crate) fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == ' ' {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

fn fnv1a(data: &[u8]) -> u32 {
    data.iter()
        .fold(2_166_136_261u32, |acc, &b| acc.wrapping_mul(16_777_619).wrapping_add(b as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(name: Option<&str>, content: &str) -> Rule {
        Rule {
            scope: Scope::Project,
            activation: Activation::Always,
            globs: None,
            name: name.map(str::to_string),
            description: None,
            content: content.to_string(),
            id: String::new(),
            project: None,
            source_format: None,
            created_at: None,
            updated_at: None,
            store_version: "1".to_string(),
        }
    }

    #[test]
    fn filename_stem_uses_name() {
        let rule = make_rule(Some("My Rule"), "content");
        assert_eq!(rule.filename_stem(), "my-rule");
    }

    #[test]
    fn filename_stem_fallback_is_stable() {
        let rule = make_rule(None, "hello world");
        let stem1 = rule.filename_stem();
        let stem2 = rule.filename_stem();
        assert_eq!(stem1, stem2);
        assert!(stem1.starts_with("rule_"));
    }

    #[test]
    fn has_store_id_false_for_new_rule() {
        let rule = make_rule(Some("test"), "content");
        assert!(!rule.has_store_id());
    }
}
