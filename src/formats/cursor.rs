use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use crate::error::{PolyrcError, Result};
use crate::ir::{Activation, Rule, Scope};
use crate::parser::Parser;
use crate::writer::Writer;
use crate::formats::copilot::split_frontmatter;

pub struct CursorParser;
pub struct CursorWriter;

/// Cursor's `globs` field can be a single string or a YAML sequence.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::Single(s) => {
                // A single string may be comma-separated
                s.split(',').map(|p| p.trim().to_string()).filter(|s| !s.is_empty()).collect()
            }
            StringOrVec::Multiple(v) => v,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CursorFrontmatter {
    description: Option<String>,
    globs: Option<StringOrVec>,
    always_apply: Option<bool>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct CursorFrontmatterOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    globs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    always_apply: Option<bool>,
}

impl Parser for CursorParser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        let rules_dir = path.join(".cursor/rules");
        if !rules_dir.exists() {
            return Ok(vec![]);
        }
        let mut rules = vec![];
        for entry in WalkDir::new(&rules_dir).min_depth(1).max_depth(1).sort_by_file_name() {
            let entry = entry.map_err(|e| PolyrcError::Io {
                path: rules_dir.clone(),
                source: e.into(),
            })?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("mdc") {
                continue;
            }

            let raw = fs::read_to_string(p).map_err(|e| PolyrcError::Io {
                path: p.to_path_buf(),
                source: e,
            })?;

            let (fm_str, body) = split_frontmatter(&raw);
            let fm: CursorFrontmatter = fm_str
                .map(|s| {
                    serde_yml::from_str(s).map_err(|e| PolyrcError::YamlParse {
                        path: p.to_path_buf(),
                        source: e,
                    })
                })
                .transpose()?
                .unwrap_or_default();

            let globs: Option<Vec<String>> = fm.globs.map(|g| g.into_vec()).filter(|v| !v.is_empty());

            let activation = if fm.always_apply == Some(true) {
                Activation::Always
            } else if globs.is_some() {
                Activation::Glob
            } else if fm.description.is_some() {
                Activation::AiDecides
            } else {
                Activation::OnDemand
            };

            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("rule").to_string();

            rules.push(Rule {
                scope: Scope::Project,
                activation,
                globs,
                name: Some(stem),
                description: fm.description,
                content: body.trim_end().to_string(),
                ..Default::default()
            });
        }
        Ok(rules)
    }
}

impl Writer for CursorWriter {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()> {
        let rules_dir = target.join(".cursor/rules");
        fs::create_dir_all(&rules_dir).map_err(|e| PolyrcError::Io {
            path: rules_dir.clone(),
            source: e,
        })?;

        for rule in rules {
            let fm = CursorFrontmatterOut {
                description: rule.description.clone(),
                globs: rule.globs.clone(),
                always_apply: if rule.activation == Activation::Always { Some(true) } else { None },
            };
            let fm_str = serde_yml::to_string(&fm).map_err(|e| PolyrcError::YamlParse {
                path: rules_dir.clone(),
                source: e,
            })?;
            let content = format!("---\n{}---\n\n{}\n", fm_str, rule.content.trim_end());
            let filename = format!("{}.mdc", rule.filename_stem());
            let file = rules_dir.join(&filename);
            fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
        }

        Ok(())
    }
}
