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

    /// Sync local store with the remote git repo (pull then push)
    Sync(SyncArgs),

    /// Manage projects in the store
    Project(ProjectArgs),

    /// List projects and rules in the store
    #[command(name = "list-project")]
    ListProject(ListProjectArgs),

    /// Push a rule or file into the store
    #[command(name = "push-rule")]
    PushRule(PushRuleArgs),

    /// Pull a named rule from the store and write it to disk
    #[command(name = "pull-rule")]
    PullRule(PullRuleArgs),

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
    /// Format to read from (mutually exclusive with --all)
    #[arg(long, value_enum, required_unless_present = "all", conflicts_with = "all")]
    pub format: Option<FormatArg>,

    /// Push all supported formats
    #[arg(long, conflicts_with = "format")]
    pub all: bool,

    /// Store rules in user scope (store/user/); reads from the format's user config dir
    #[arg(long, conflicts_with = "project")]
    pub user: bool,

    /// Project name to store rules under (e.g. "myApp")
    #[arg(long, conflicts_with = "user")]
    pub project: Option<String>,

    /// Source project root directory (default: current dir; auto-detected for --user)
    #[arg(long, default_value = ".")]
    pub input: PathBuf,

    /// Print what would be written without touching the store
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

// ── pull-format ───────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct PullFormatArgs {
    /// Format to write (mutually exclusive with --all)
    #[arg(long, value_enum, required_unless_present = "all", conflicts_with = "all")]
    pub format: Option<FormatArg>,

    /// Pull and write all supported formats
    #[arg(long, conflicts_with = "format")]
    pub all: bool,

    /// Load from user scope (store/user/); writes to the format's user config dir
    #[arg(long, conflicts_with = "project")]
    pub user: bool,

    /// Project name to load rules from
    #[arg(long, conflicts_with = "user")]
    pub project: Option<String>,

    /// Target project root directory (default: current dir; auto-detected for --user)
    #[arg(long, default_value = ".")]
    pub output: PathBuf,

    /// Print what would be written without modifying local files
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

// ── sync ──────────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct SyncArgs {
    /// Only push local commits to the remote (skip pull)
    #[arg(long, conflicts_with = "pull_only")]
    pub push_only: bool,

    /// Only pull remote changes (skip push)
    #[arg(long, conflicts_with = "push_only")]
    pub pull_only: bool,
}

// ── project ───────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommands,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommands {
    /// Rename a project in the store
    RenameProject {
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

// ── list-project ──────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct ListProjectArgs {
    /// Project name to inspect. Omit to list all projects.
    pub name: Option<String>,

    /// Show full rule content (when a name is given) or rule names per project (when listing all)
    #[arg(long)]
    pub verbose: bool,
}

// ── push-rule ─────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct PushRuleArgs {
    /// Name for the rule in the store (e.g. "rust-gitignore")
    pub name: String,

    /// Read rule content from this file
    #[arg(long)]
    pub from_file: Option<std::path::PathBuf>,

    /// Store rule in user scope (store/user/)
    #[arg(long, conflicts_with = "project")]
    pub user: bool,

    /// Project name to store the rule under (e.g. "myApp")
    #[arg(long, conflicts_with = "user")]
    pub project: Option<String>,

    /// Activation mode of the rule
    #[arg(long, value_enum, default_value = "always")]
    pub activation: ActivationArg,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ActivationArg {
    Always,
    OnDemand,
    Glob,
    AiDecides,
}

// ── pull-rule ─────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct PullRuleArgs {
    /// Name of the rule to pull from the store (e.g. "rust-gitignore")
    pub name: String,

    /// Target format to write the rule as
    #[arg(long, value_enum, required = true)]
    pub format: FormatArg,

    /// Search in user scope (store/user/)
    #[arg(long, conflicts_with = "project")]
    pub user: bool,

    /// Project name to search in (e.g. "myApp")
    #[arg(long, conflicts_with = "user")]
    pub project: Option<String>,

    /// Directory to write the rule file into (default: current dir)
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Overwrite existing file without asking
    #[arg(long)]
    pub force: bool,
}

// ── discover ──────────────────────────────────────────────────────────────────

#[derive(clap::Args, Debug)]
pub struct DiscoverArgs {
    /// Scope to search: user (project scope planned for future)
    #[arg(long, conflicts_with = "user")]
    pub scope: Option<String>,

    /// Discover user-level configs — shorthand for --scope user
    #[arg(long, conflicts_with = "scope")]
    pub user: bool,

    /// Discover all supported formats (default when --format is omitted)
    #[arg(long, conflicts_with = "format")]
    pub all: bool,

    /// Limit to one format
    #[arg(long, value_enum, conflicts_with = "all")]
    pub format: Option<FormatArg>,
}
