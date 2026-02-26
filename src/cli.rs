use std::path::PathBuf;
use clap::{Parser, Subcommand};

// ── format enum ───────────────────────────────────────────────────────────────

/// Canonical format names — drives tab-completion for all --format / --from / --to args.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum FormatArg {
    Cursor,
    Windsurf,
    #[value(alias = "github-copilot", alias = "ghcopilot")]
    Copilot,
    #[value(alias = "claude-code")]
    Claude,
    #[value(alias = "gemini-cli")]
    Gemini,
    #[value(alias = "google-antigravity")]
    Antigravity,
}

impl FormatArg {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::Copilot => "copilot",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Antigravity => "antigravity",
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "polyrc",
    about = "Convert AI coding agent configurations between tools",
    version,
    arg_required_else_help = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Convert configuration from one format to another (optionally via store)
    Convert(ConvertArgs),

    /// List all supported formats
    #[command(name = "supported-formats", alias = "list-formats")]
    SupportedFormats,

    /// Initialize the local interlingua store (git repo)
    Init(InitArgs),

    /// Read local format rules → convert to IR → save to store (auto-commits)
    PushFormat(PushFormatArgs),

    /// Load IR from store → write as local format
    PullFormat(PullFormatArgs),

    /// Push local store commits to the central remote repo
    PushStore,

    /// Pull from central remote repo into local store (with IR-level merge)
    PullStore,

    /// Manage projects in the store
    Project(ProjectArgs),

    /// Discover installed user-level configs for all (or one) format
    Discover(DiscoverArgs),

    /// Update polyrc to the latest release from GitHub
    SelfUpdate(SelfUpdateArgs),

    /// Get or set the preferred editor (used when opening files)
    SetEditor(SetEditorArgs),

    /// Generate shell completion script
    Completion {
        /// Shell to generate completions for: bash, zsh, fish, powershell
        shell: String,
        /// Install the completion script to the default location for the shell
        #[arg(long)]
        install: bool,
    },
}

// ── convert ──────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ConvertArgs {
    /// Source format
    #[arg(long, value_enum)]
    pub from: FormatArg,

    /// Target format
    #[arg(long, value_enum)]
    pub to: FormatArg,

    /// Project name in the store. When set, conversion goes through the store.
    #[arg(long)]
    pub project: Option<String>,

    /// Source project root directory
    #[arg(long, default_value = ".")]
    pub input: PathBuf,

    /// Target project root directory
    #[arg(long, default_value = ".")]
    pub output: PathBuf,

    /// Filter by scope: user, project, or path
    #[arg(long)]
    pub scope: Option<String>,

    /// Print what would be written without creating files or touching the store
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

// ── init ──────────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Git remote URL to clone. If omitted, a local-only store is created.
    #[arg(long)]
    pub repo: Option<String>,

    /// Path for the local store. Defaults to ~/.polyrc/store
    #[arg(long)]
    pub store: Option<PathBuf>,
}

// ── push-format ───────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct PushFormatArgs {
    /// Format to read from
    #[arg(long, value_enum)]
    pub format: FormatArg,

    /// Project name to store rules under
    #[arg(long)]
    pub project: Option<String>,

    /// Filter by scope: user, project, or path
    #[arg(long)]
    pub scope: Option<String>,

    /// Source project root directory
    #[arg(long, default_value = ".")]
    pub input: PathBuf,

    /// Print what would be written without touching the store
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

// ── pull-format ───────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct PullFormatArgs {
    /// Format to write
    #[arg(long, value_enum)]
    pub format: FormatArg,

    /// Project name to load rules from
    #[arg(long)]
    pub project: Option<String>,

    /// Filter by scope: user, project, or path
    #[arg(long)]
    pub scope: Option<String>,

    /// Target project root directory
    #[arg(long, default_value = ".")]
    pub output: PathBuf,

    /// Print what would be written without modifying local files
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

// ── project ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommands,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// List all projects in the store
    List,
    /// Rename a project in the store
    Rename {
        /// Current project name
        old_name: String,
        /// New project name
        new_name: String,
    },
}

// ── self-update ───────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SelfUpdateArgs {
    /// Check for an update but do not install it
    #[arg(long)]
    pub check_only: bool,

    /// Install even if no SHA-256 checksum sidecar is found in the release
    #[arg(long)]
    pub skip_checksum: bool,
}

// ── set-editor ────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SetEditorArgs {
    /// Editor command to use (e.g. "code", "zed", "vim"). Omit to show current value.
    pub editor: Option<String>,

    /// Clear the preferred_editor setting (revert to $EDITOR / OS default)
    #[arg(long)]
    pub clear: bool,
}

// ── discover ──────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct DiscoverArgs {
    /// Discover user-level configs (from ~/, ~/Library/Application Support, etc.)
    #[arg(long)]
    pub user: bool,

    /// Limit to one format
    #[arg(long, value_enum)]
    pub format: Option<FormatArg>,
}
