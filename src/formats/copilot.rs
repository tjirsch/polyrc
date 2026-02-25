use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use crate::error::{PolyrcError, Result};
use crate::ir::{Activation, Rule, Scope};
use crate::parser::Parser;
use crate::writer::Writer;

pub struct CopilotParser;
pub struct CopilotWriter;

#[derive(Debug, Serialize, Deserialize, Default)]
struct CopilotFrontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "applyTo", skip_serializing_if = "Option::is_none")]
    apply_to: Option<String>,
}

/// Split YAML frontmatter from markdown content.
/// Returns `(Option<frontmatter_str>, body_str)`.
pub(crate) fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let fm = &rest[..end];
            let body = &rest[end + 5..]; // skip "\n---\n"
            return (Some(fm), body);
        }
        // Handle trailing --- at end of file
        if let Some(end) = rest.find("\n---") {
            if end + 4 == rest.len() {
                let fm = &rest[..end];
                return (Some(fm), "");
            }
        }
    }
    (None, content)
}

impl Parser for CopilotParser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        let mut rules = vec![];

        // Project-wide instructions
        let main_file = path.join(".github/copilot-instructions.md");
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
                    name: Some("copilot-instructions".to_string()),
                    description: None,
                    content: content.trim_end().to_string(),
                });
            }
        }

        // Path-scoped instructions
        let instructions_dir = path.join(".github/instructions");
        if instructions_dir.exists() {
            for entry in WalkDir::new(&instructions_dir)
                .min_depth(1)
                .max_depth(1)
                .sort_by_file_name()
            {
                let entry = entry.map_err(|e| PolyrcError::Io {
                    path: instructions_dir.clone(),
                    source: e.into(),
                })?;
                let p = entry.path();
                let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
                if !fname.ends_with(".instructions.md") {
                    continue;
                }

                let raw = fs::read_to_string(p).map_err(|e| PolyrcError::Io {
                    path: p.to_path_buf(),
                    source: e,
                })?;

                let (fm_str, body) = split_frontmatter(&raw);
                let fm: CopilotFrontmatter = fm_str
                    .map(|s| {
                        serde_yml::from_str(s).map_err(|e| PolyrcError::YamlParse {
                            path: p.to_path_buf(),
                            source: e,
                        })
                    })
                    .transpose()?
                    .unwrap_or_default();

                let stem = fname
                    .strip_suffix(".instructions.md")
                    .unwrap_or(fname)
                    .to_string();
                let name = fm.name.unwrap_or(stem);

                let (activation, globs) = if let Some(apply_to) = fm.apply_to {
                    (Activation::Glob, Some(vec![apply_to]))
                } else {
                    (Activation::Always, None)
                };

                rules.push(Rule {
                    scope: Scope::Path,
                    activation,
                    globs,
                    name: Some(name),
                    description: fm.description,
                    content: body.trim_end().to_string(),
                });
            }
        }

        Ok(rules)
    }
}

impl Writer for CopilotWriter {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()> {
        let mut always_rules: Vec<&Rule> = vec![];
        let mut glob_rules: Vec<&Rule> = vec![];

        for rule in rules {
            if rule.activation == Activation::Glob || rule.globs.is_some() {
                glob_rules.push(rule);
            } else {
                always_rules.push(rule);
            }
        }

        // Write project-wide instructions
        if !always_rules.is_empty() {
            let github_dir = target.join(".github");
            fs::create_dir_all(&github_dir).map_err(|e| PolyrcError::Io {
                path: github_dir.clone(),
                source: e,
            })?;
            let file = github_dir.join("copilot-instructions.md");
            let content = if always_rules.len() == 1 {
                always_rules[0].content.trim_end().to_string() + "\n"
            } else {
                always_rules
                    .iter()
                    .map(|r| {
                        let header = r.name.as_deref().unwrap_or("Rule");
                        format!("## {}\n\n{}\n", header, r.content.trim_end())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
        }

        // Write path-scoped instructions
        if !glob_rules.is_empty() {
            let instructions_dir = target.join(".github/instructions");
            fs::create_dir_all(&instructions_dir).map_err(|e| PolyrcError::Io {
                path: instructions_dir.clone(),
                source: e,
            })?;
            for rule in glob_rules {
                let fm = CopilotFrontmatter {
                    name: rule.name.clone(),
                    description: rule.description.clone(),
                    apply_to: rule.globs.as_ref().and_then(|g| g.first()).cloned(),
                };
                let fm_str = serde_yml::to_string(&fm).map_err(|e| PolyrcError::YamlParse {
                    path: instructions_dir.clone(),
                    source: e,
                })?;
                let content = format!("---\n{}---\n\n{}\n", fm_str, rule.content.trim_end());
                let filename = format!("{}.instructions.md", rule.filename_stem());
                let file = instructions_dir.join(&filename);
                fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
            }
        }

        Ok(())
    }
}
