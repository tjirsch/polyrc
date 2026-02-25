use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::process::Command;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Development tasks for the project", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build release binary and install to ~/.local/bin (no sudo required)
    Install,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Install => install()?,
    }
    Ok(())
}

fn install() -> Result<()> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .context("Failed to find workspace root")?;

    std::env::set_current_dir(workspace_root)
        .context("Failed to change directory to workspace root")?;

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    println!("Building release binary...");
    let status = Command::new(&cargo)
        .arg("build")
        .arg("--release")
        .status()
        .context("Failed to run cargo build")?;

    if !status.success() {
        anyhow::bail!("Cargo build failed");
    }

    let home = env::var("HOME").context("HOME environment variable not set")?;
    let install_dir = std::path::Path::new(&home).join(".local").join("bin");

    std::fs::create_dir_all(&install_dir)
        .with_context(|| format!("Failed to create directory: {}", install_dir.display()))?;

    let install_path = install_dir.join("polyrc");

    println!("Installing to {}...", install_dir.display());
    std::fs::copy("target/release/polyrc", &install_path)
        .with_context(|| format!("Failed to copy binary to {}", install_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&install_path, std::fs::Permissions::from_mode(0o755))
            .context("Failed to set executable permissions")?;
    }

    println!("Successfully installed polyrc to {}", install_dir.display());
    println!("Make sure {} is in your PATH", install_dir.display());
    Ok(())
}
