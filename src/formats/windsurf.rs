use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use crate::error::{PolyrcError, Result};
use crate::ir::{Activation, Rule, Scope};
use crate::parser::Parser;
use crate::writer::Writer;

const FILE_CHAR_LIMIT: usize = 6_000;
const TOTAL_CHAR_LIMIT: usize = 12_000;

pub struct WindsurfParser;
pub struct WindsurfWriter;

impl Parser for WindsurfParser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        // User layout: ~/.codeium/windsurf/memories/global_rules.md (single file)
        let global_rules = path.join("global_rules.md");
        if global_rules.exists() {
            let content = fs::read_to_string(&global_rules).map_err(|e| PolyrcError::Io {
                path: global_rules.clone(),
                source: e,
            })?;
            if content.trim().is_empty() {
                return Ok(vec![]);
            }
            return Ok(vec![Rule {
                scope: Scope::User,
                activation: Activation::Always,
                name: Some("global-rules".to_string()),
                content: content.trim_end().to_string(),
                ..Default::default()
            }]);
        }

        // Project layout: .windsurf/rules/*.md
        let rules_dir = path.join(".windsurf/rules");
        if !rules_dir.exists() {
            return Ok(vec![]);
        }
        let mut rules = vec![];
        for entry in WalkDir::new(&rules_dir)
            .min_depth(1)
            .max_depth(1)
            .sort_by_file_name()
        {
            let entry = entry.map_err(|e| PolyrcError::Io {
                path: rules_dir.clone(),
                source: e.into(),
            })?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let content = fs::read_to_string(p).map_err(|e| PolyrcError::Io {
                path: p.to_path_buf(),
                source: e,
            })?;
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("rule").to_string();
            rules.push(Rule {
                scope: Scope::Project,
                activation: Activation::Always,
                name: Some(name),
                content: content.trim_end().to_string(),
                ..Default::default()
            });
        }
        Ok(rules)
    }
}

impl Writer for WindsurfWriter {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()> {
        // User layout: target is the memories dir → write everything as global_rules.md
        let is_user = rules.iter().any(|r| r.scope == Scope::User);
        if is_user {
            fs::create_dir_all(target).map_err(|e| PolyrcError::Io {
                path: target.to_path_buf(),
                source: e,
            })?;
            let content = crate::formats::gemini::join_rules(rules);
            let file = target.join("global_rules.md");
            return fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e });
        }

        // Project layout: .windsurf/rules/*.md (one file per rule)
        let rules_dir = target.join(".windsurf/rules");
        fs::create_dir_all(&rules_dir).map_err(|e| PolyrcError::Io {
            path: rules_dir.clone(),
            source: e,
        })?;

        let mut total_chars = 0usize;
        for rule in rules {
            let content = rule.content.trim_end().to_string() + "\n";
            let char_count = content.chars().count();
            let name = rule.name.as_deref().unwrap_or("rule");

            if char_count > FILE_CHAR_LIMIT {
                eprintln!(
                    "warning: rule '{}' is {} chars, exceeds Windsurf per-file limit of {}",
                    name, char_count, FILE_CHAR_LIMIT
                );
            }
            total_chars += char_count;

            let filename = format!("{}.md", rule.filename_stem());
            let file = rules_dir.join(&filename);
            fs::write(&file, &content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
        }

        if total_chars > TOTAL_CHAR_LIMIT {
            eprintln!(
                "warning: total rules content is {} chars, exceeds Windsurf total limit of {}",
                total_chars, TOTAL_CHAR_LIMIT
            );
        }

        Ok(())
    }
}
