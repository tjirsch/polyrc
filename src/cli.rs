use std::path::PathBuf;
use clap::{Parser, Subcommand};

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
    /// Convert configuration from one tool to another
    Convert(ConvertArgs),
    /// List all supported formats
    ListFormats,
}

#[derive(clap::Args, Debug)]
pub struct ConvertArgs {
    /// Source format (cursor, windsurf, copilot, claude, gemini, antigravity)
    #[arg(long)]
    pub from: String,

    /// Target format
    #[arg(long)]
    pub to: String,

    /// Source project root directory
    #[arg(long, default_value = ".")]
    pub input: PathBuf,

    /// Target project root directory
    #[arg(long, default_value = ".")]
    pub output: PathBuf,

    /// Filter by scope: user, project, or path
    #[arg(long)]
    pub scope: Option<String>,

    /// Print what would be written without creating files
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}
