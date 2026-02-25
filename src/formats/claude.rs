use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use crate::error::{PolyrcError, Result};
use crate::ir::{Activation, Rule, Scope};
use crate::parser::Parser;
use crate::writer::Writer;

pub struct ClaudeParser;
pub struct ClaudeWriter;

impl Parser for ClaudeParser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        let mut rules = vec![];

        // Primary CLAUDE.md
        let main_file = path.join("CLAUDE.md");
        if main_file.exists() {
            let content = fs::read_to_string(&main_file).map_err(|e| PolyrcError::Io {
                path: main_file.clone(),
                source: e,
            })?;
            if !content.trim().is_empty() {
                rules.push(Rule {
                    scope: Scope::Project,
                    activation: Activation::Always,
                    globs: None,
                    name: Some("claude".to_string()),
                    description: None,
                    content: content.trim_end().to_string(),
                });
            }
        }

        // .claude/rules/*.md
        let rules_dir = path.join(".claude/rules");
        if rules_dir.exists() {
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
                    globs: None,
                    name: Some(name),
                    description: None,
                    content: content.trim_end().to_string(),
                });
            }
        }

        Ok(rules)
    }
}

impl Writer for ClaudeWriter {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()> {
        if rules.is_empty() {
            return Ok(());
        }

        if rules.len() == 1 {
            // Single rule → CLAUDE.md
            let file = target.join("CLAUDE.md");
            let content = rules[0].content.trim_end().to_string() + "\n";
            fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
        } else {
            // Multiple rules → .claude/rules/*.md
            let rules_dir = target.join(".claude/rules");
            fs::create_dir_all(&rules_dir).map_err(|e| PolyrcError::Io {
                path: rules_dir.clone(),
                source: e,
            })?;
            for rule in rules {
                let filename = format!("{}.md", rule.filename_stem());
                let file = rules_dir.join(&filename);
                let content = rule.content.trim_end().to_string() + "\n";
                fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
            }
        }

        Ok(())
    }
}
