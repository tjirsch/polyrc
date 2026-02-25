use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use crate::error::{PolyrcError, Result};
use crate::ir::{Activation, Rule, Scope};
use crate::parser::Parser;
use crate::writer::Writer;

pub struct AntigravityParser;
pub struct AntigravityWriter;

/// Returns the rules directory, checking both legacy (.agents) and current (.agent) paths.
fn rules_dir(path: &Path) -> Option<std::path::PathBuf> {
    let current = path.join(".agent/rules");
    if current.exists() {
        return Some(current);
    }
    let legacy = path.join(".agents/rules");
    if legacy.exists() {
        return Some(legacy);
    }
    None
}

impl Parser for AntigravityParser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        let Some(dir) = rules_dir(path) else {
            return Ok(vec![]);
        };
        let mut rules = vec![];
        for entry in WalkDir::new(&dir).min_depth(1).max_depth(1).sort_by_file_name() {
            let entry = entry.map_err(|e| PolyrcError::Io {
                path: dir.clone(),
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
                ..Default::default()
            });
        }
        Ok(rules)
    }
}

impl Writer for AntigravityWriter {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()> {
        let rules_dir = target.join(".agent/rules");
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
        Ok(())
    }
}
