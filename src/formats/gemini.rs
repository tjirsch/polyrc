use std::fs;
use std::path::Path;
use crate::error::{PolyrcError, Result};
use crate::ir::{Activation, Rule, Scope};
use crate::parser::Parser;
use crate::writer::Writer;

pub struct GeminiParser;
pub struct GeminiWriter;

impl Parser for GeminiParser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        let file = path.join("GEMINI.md");
        if !file.exists() {
            return Ok(vec![]);
        }
        let content = fs::read_to_string(&file).map_err(|e| PolyrcError::Io {
            path: file.clone(),
            source: e,
        })?;
        if content.trim().is_empty() {
            return Ok(vec![]);
        }
        Ok(vec![Rule {
            scope: Scope::Project,
            activation: Activation::Always,
            globs: None,
            name: Some("gemini".to_string()),
            description: None,
            content: content.trim_end().to_string(),
            ..Default::default()
        }])
    }
}

impl Writer for GeminiWriter {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()> {
        if rules.is_empty() {
            return Ok(());
        }
        let file = target.join("GEMINI.md");
        let content = join_rules(rules);
        fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })
    }
}

/// Concatenate multiple rules into a single markdown file with section headers.
pub(crate) fn join_rules(rules: &[Rule]) -> String {
    if rules.len() == 1 {
        return rules[0].content.clone() + "\n";
    }
    rules
        .iter()
        .map(|r| {
            let header = r.name.as_deref().unwrap_or("Rule");
            format!("## {}\n\n{}\n", header, r.content.trim_end())
        })
        .collect::<Vec<_>>()
        .join("\n")
}
