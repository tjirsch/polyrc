use crate::ir::Rule;

pub struct MergeResult {
    /// Rules to keep in the merged store.
    pub merged: Vec<Rule>,
    /// Human-readable descriptions of conflicts resolved by last-write-wins.
    pub warnings: Vec<String>,
}

/// Merge `incoming` rules into `local` rules.
///
/// Matching is by `id`. New rules (no ID) in `incoming` are added directly.
/// Rules with the same ID are resolved by last `updated_at` timestamp (last-write-wins).
/// Rules only in `local` are kept unchanged.
pub fn merge_rules(mut local: Vec<Rule>, incoming: Vec<Rule>) -> MergeResult {
    let mut warnings = vec![];

    for inc in incoming {
        if inc.id.is_empty() {
            // No ID — treat as a new rule, just add
            local.push(inc);
            continue;
        }

        if let Some(pos) = local.iter().position(|r| r.id == inc.id) {
            let loc = &local[pos];
            if loc.content == inc.content
                && loc.scope == inc.scope
                && loc.activation == inc.activation
            {
                // Identical — no-op
                continue;
            }

            // Both changed — last-write-wins by updated_at
            let use_incoming = match (loc.updated_at.as_deref(), inc.updated_at.as_deref()) {
                (Some(lt), Some(it)) => it > lt,
                (None, Some(_)) => true,
                _ => false,
            };

            if use_incoming {
                let name = inc.name.as_deref().unwrap_or(&inc.id);
                warnings.push(format!(
                    "conflict on rule '{}': remote version is newer, keeping remote",
                    name
                ));
                local[pos] = inc;
            } else {
                let name = loc.name.as_deref().unwrap_or(&loc.id);
                warnings.push(format!(
                    "conflict on rule '{}': local version is newer or equal, keeping local",
                    name
                ));
            }
        } else {
            // Only in incoming — add to local
            local.push(inc);
        }
    }

    MergeResult { merged: local, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Activation, Scope};

    fn rule(id: &str, content: &str, updated_at: Option<&str>) -> Rule {
        Rule {
            scope: Scope::Project,
            activation: Activation::Always,
            globs: None,
            name: Some(id.to_string()),
            description: None,
            content: content.to_string(),
            id: id.to_string(),
            project: None,
            source_format: None,
            created_at: None,
            updated_at: updated_at.map(str::to_string),
            store_version: "1".to_string(),
        }
    }

    #[test]
    fn identical_rules_no_op() {
        let local = vec![rule("a", "hello", Some("2026-01-01T00:00:00Z"))];
        let incoming = vec![rule("a", "hello", Some("2026-01-01T00:00:00Z"))];
        let result = merge_rules(local, incoming);
        assert_eq!(result.merged.len(), 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn new_rule_in_incoming_added() {
        let local = vec![rule("a", "hello", None)];
        let incoming = vec![rule("b", "world", None)];
        let result = merge_rules(local, incoming);
        assert_eq!(result.merged.len(), 2);
    }

    #[test]
    fn remote_newer_wins() {
        let local = vec![rule("a", "old", Some("2026-01-01T00:00:00Z"))];
        let incoming = vec![rule("a", "new", Some("2026-02-01T00:00:00Z"))];
        let result = merge_rules(local, incoming);
        assert_eq!(result.merged[0].content, "new");
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn local_newer_wins() {
        let local = vec![rule("a", "local-new", Some("2026-02-01T00:00:00Z"))];
        let incoming = vec![rule("a", "remote-old", Some("2026-01-01T00:00:00Z"))];
        let result = merge_rules(local, incoming);
        assert_eq!(result.merged[0].content, "local-new");
        assert_eq!(result.warnings.len(), 1);
    }
}
