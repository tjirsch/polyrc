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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
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
}

impl Rule {
    /// Returns a stable filename stem for use in multi-file format writers.
    pub fn filename_stem(&self) -> String {
        match &self.name {
            Some(n) => sanitize_filename(n),
            None => format!("rule_{:08x}", fnv1a(self.content.as_bytes())),
        }
    }
}

fn sanitize_filename(name: &str) -> String {
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

    #[test]
    fn filename_stem_uses_name() {
        let rule = Rule {
            scope: Scope::Project,
            activation: Activation::Always,
            globs: None,
            name: Some("My Rule".to_string()),
            description: None,
            content: "content".to_string(),
        };
        assert_eq!(rule.filename_stem(), "my-rule");
    }

    #[test]
    fn filename_stem_fallback_is_stable() {
        let rule = Rule {
            scope: Scope::Project,
            activation: Activation::Always,
            globs: None,
            name: None,
            description: None,
            content: "hello world".to_string(),
        };
        let stem1 = rule.filename_stem();
        let stem2 = rule.filename_stem();
        assert_eq!(stem1, stem2);
        assert!(stem1.starts_with("rule_"));
    }
}
