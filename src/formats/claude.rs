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
    /// Parse Claude Code config from `path`.
    ///
    /// Two layouts are supported:
    ///
    /// **Project layout** — `path` is a project root (e.g. `/home/user/myapp`):
    /// - `{path}/CLAUDE.md`                    always-on, project scope
    /// - `{path}/.claude/rules/*.md`           always-on, project scope
    /// - `{path}/.claude/commands/*.md`        on-demand (slash commands), project scope
    /// - `{path}/.claude/skills/*/SKILL.md`   ai-decides (skill descriptions), project scope
    /// - `{path}/.claude/agents/*.md`          ai-decides, project scope
    ///
    /// **User layout** — `path` is `~/.claude` (detected by dir name ending in `.claude`):
    /// - `{path}/CLAUDE.md`                    always-on, user scope
    /// - `{path}/rules/*.md`                   always-on, user scope
    /// - `{path}/commands/*.md`                on-demand (slash commands), user scope
    /// - `{path}/skills/*/SKILL.md`           ai-decides, user scope
    /// - `{path}/agents/*.md`                  ai-decides, user scope
    fn parse(&self, path: &Path) -> Result<Vec<Rule>> {
        // Detect whether path IS the .claude config directory (user root) or a project root.
        let is_user_root = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == ".claude")
            .unwrap_or(false);

        let scope = if is_user_root { Scope::User } else { Scope::Project };

        // Helper: path to the .claude subdirectory (only used in project layout)
        let dot_claude = path.join(".claude");

        let (rules_dir, commands_dir, skills_dir, agents_dir) = if is_user_root {
            (
                path.join("rules"),
                path.join("commands"),
                path.join("skills"),
                path.join("agents"),
            )
        } else {
            (
                dot_claude.join("rules"),
                dot_claude.join("commands"),
                dot_claude.join("skills"),
                dot_claude.join("agents"),
            )
        };

        let mut rules = vec![];

        // ── CLAUDE.md ────────────────────────────────────────────────────────
        let main_file = path.join("CLAUDE.md");
        if main_file.exists() {
            let content = fs::read_to_string(&main_file).map_err(|e| PolyrcError::Io {
                path: main_file.clone(),
                source: e,
            })?;
            if !content.trim().is_empty() {
                rules.push(Rule {
                    scope: scope.clone(),
                    activation: Activation::Always,
                    name: Some("claude".to_string()),
                    content: content.trim_end().to_string(),
                    ..Default::default()
                });
            }
        }

        // ── rules/*.md — always-on ────────────────────────────────────────────
        parse_md_dir(&rules_dir, scope.clone(), Activation::Always, &mut rules)?;

        // ── commands/*.md — on-demand (slash commands) ────────────────────────
        parse_md_dir(&commands_dir, scope.clone(), Activation::OnDemand, &mut rules)?;

        // ── skills/*/SKILL.md — ai-decides ───────────────────────────────────
        parse_skill_dir(&skills_dir, scope.clone(), &mut rules)?;

        // ── agents/*.md — ai-decides ──────────────────────────────────────────
        parse_md_dir(&agents_dir, scope.clone(), Activation::AiDecides, &mut rules)?;

        Ok(rules)
    }
}

/// Read all `*.md` files directly inside `dir`, push as rules with the given scope/activation.
fn parse_md_dir(
    dir: &Path,
    scope: Scope,
    activation: Activation,
    rules: &mut Vec<Rule>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(dir).min_depth(1).max_depth(1).sort_by_file_name() {
        let entry = entry.map_err(|e| PolyrcError::Io {
            path: dir.to_path_buf(),
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
        if content.trim().is_empty() {
            continue;
        }
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("rule").to_string();
        rules.push(Rule {
            scope: scope.clone(),
            activation: activation.clone(),
            name: Some(name),
            content: content.trim_end().to_string(),
            ..Default::default()
        });
    }
    Ok(())
}

/// Read `skills/*/SKILL.md` — each skill is a subdirectory; the subdirectory name is the skill name.
fn parse_skill_dir(dir: &Path, scope: Scope, rules: &mut Vec<Rule>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(dir).min_depth(1).max_depth(1).sort_by_file_name() {
        let entry = entry.map_err(|e| PolyrcError::Io {
            path: dir.to_path_buf(),
            source: e.into(),
        })?;
        let subdir = entry.path();
        if !subdir.is_dir() {
            continue;
        }
        let skill_file = subdir.join("SKILL.md");
        if !skill_file.exists() {
            continue;
        }
        let content = fs::read_to_string(&skill_file).map_err(|e| PolyrcError::Io {
            path: skill_file.clone(),
            source: e,
        })?;
        if content.trim().is_empty() {
            continue;
        }
        let name = subdir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("skill")
            .to_string();
        rules.push(Rule {
            scope: scope.clone(),
            activation: Activation::AiDecides,
            name: Some(name),
            content: content.trim_end().to_string(),
            ..Default::default()
        });
    }
    Ok(())
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
