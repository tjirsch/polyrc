use crate::error::{PolyrcError, Result};
use crate::parser::Parser;
use crate::writer::Writer;

pub mod antigravity;
pub mod claude;
pub mod copilot;
pub mod cursor;
pub mod gemini;
pub mod windsurf;

/// Canonical format identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Cursor,
    Windsurf,
    Copilot,
    Claude,
    Gemini,
    Antigravity,
}

impl Format {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "cursor" => Ok(Self::Cursor),
            "windsurf" => Ok(Self::Windsurf),
            "copilot" | "github-copilot" | "ghcopilot" => Ok(Self::Copilot),
            "claude" | "claude-code" => Ok(Self::Claude),
            "gemini" | "gemini-cli" => Ok(Self::Gemini),
            "antigravity" | "google-antigravity" => Ok(Self::Antigravity),
            other => Err(PolyrcError::UnknownFormat(other.to_string())),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::Copilot => "copilot",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Antigravity => "antigravity",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Cursor      => "Cursor (.cursor/rules/*.mdc, YAML frontmatter)",
            Self::Windsurf    => "Windsurf (.windsurf/rules/*.md, plain markdown)",
            Self::Copilot     => "GitHub Copilot (.github/copilot-instructions.md + .github/instructions/)",
            Self::Claude      => "Claude Code (CLAUDE.md + .claude/rules/*.md)",
            Self::Gemini      => "Gemini CLI (GEMINI.md)",
            Self::Antigravity => "Google Antigravity (.agent/rules/*.md)",
        }
    }

    pub fn parser(&self) -> Box<dyn Parser> {
        match self {
            Self::Cursor      => Box::new(cursor::CursorParser),
            Self::Windsurf    => Box::new(windsurf::WindsurfParser),
            Self::Copilot     => Box::new(copilot::CopilotParser),
            Self::Claude      => Box::new(claude::ClaudeParser),
            Self::Gemini      => Box::new(gemini::GeminiParser),
            Self::Antigravity => Box::new(antigravity::AntigravityParser),
        }
    }

    pub fn writer(&self) -> Box<dyn Writer> {
        match self {
            Self::Cursor      => Box::new(cursor::CursorWriter),
            Self::Windsurf    => Box::new(windsurf::WindsurfWriter),
            Self::Copilot     => Box::new(copilot::CopilotWriter),
            Self::Claude      => Box::new(claude::ClaudeWriter),
            Self::Gemini      => Box::new(gemini::GeminiWriter),
            Self::Antigravity => Box::new(antigravity::AntigravityWriter),
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Cursor,
            Self::Windsurf,
            Self::Copilot,
            Self::Claude,
            Self::Gemini,
            Self::Antigravity,
        ]
    }
}
