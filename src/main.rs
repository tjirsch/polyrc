use anyhow::Context;
use clap::Parser as ClapParser;

mod cli;
mod config;
mod convert;
mod discover;
mod error;
mod formats;
mod ir;
mod parser;
mod store;
mod sync;
mod writer;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    match args.command {
        cli::Commands::Convert(a) => convert::run(a).context("conversion failed")?,
        cli::Commands::Discover(a) => discover::run(a).context("discover failed")?,
        cli::Commands::ListFormats => {
            for fmt in formats::Format::all() {
                println!("{:<15} {}", fmt.name(), fmt.description());
            }
        }
        cli::Commands::Init(a) => commands::init(a)?,
        cli::Commands::PushFormat(a) => commands::push_format(a)?,
        cli::Commands::PullFormat(a) => commands::pull_format(a)?,
        cli::Commands::PushStore => commands::push_store()?,
        cli::Commands::PullStore => commands::pull_store()?,
        cli::Commands::Project(a) => commands::project(a)?,
        cli::Commands::Completion { shell, install } => {
            run_completion(&shell, install)
                .with_context(|| format!("failed to generate completion for '{shell}'"))?;
        }
    }
    Ok(())
}

fn run_completion(shell_str: &str, install: bool) -> anyhow::Result<()> {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};
    use std::str::FromStr;

    let shell = Shell::from_str(shell_str).map_err(|_| {
        anyhow::anyhow!(
            "Unknown shell '{}'. Supported shells: bash, zsh, fish, powershell",
            shell_str
        )
    })?;

    let mut cmd = cli::Cli::command();
    let bin_name = "polyrc";

    if install {
        let (path, post_install_msg) = completion_install_path(shell)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&path)?;
        generate(shell, &mut cmd, bin_name, &mut file);
        println!("Completion script installed to: {}", path.display());
        if let Some(msg) = post_install_msg {
            println!("{}", msg);
        }
    } else {
        generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    }

    Ok(())
}

fn completion_install_path(shell: clap_complete::Shell) -> anyhow::Result<(std::path::PathBuf, Option<String>)> {
    use clap_complete::Shell;
    use std::path::PathBuf;

    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());

    let (path, msg): (PathBuf, Option<String>) = match shell {
        Shell::Bash => (
            PathBuf::from(format!(
                "{}/.local/share/bash-completion/completions/polyrc",
                home
            )),
            Some(
                "Ensure bash-completion is installed and sourced in your ~/.bashrc".to_string(),
            ),
        ),
        Shell::Zsh => (
            PathBuf::from(format!("{}/.zsh/completions/_polyrc", home)),
            Some(
                "Ensure ~/.zsh/completions is in your fpath — add to ~/.zshrc:\n  fpath=(~/.zsh/completions $fpath)\n  autoload -Uz compinit && compinit"
                    .to_string(),
            ),
        ),
        Shell::Fish => (
            PathBuf::from(format!(
                "{}/.config/fish/completions/polyrc.fish",
                home
            )),
            None,
        ),
        Shell::PowerShell => {
            let userprofile =
                std::env::var("USERPROFILE").unwrap_or_else(|_| home.clone());
            (
                PathBuf::from(format!(
                    r"{}\Documents\PowerShell\Completions\polyrc.ps1",
                    userprofile
                )),
                Some(
                    "Add to your $PROFILE:\n  . \"$env:USERPROFILE\\Documents\\PowerShell\\Completions\\polyrc.ps1\""
                        .to_string(),
                ),
            )
        }
        _ => anyhow::bail!("Unsupported shell: {:?}", shell),
    };

    Ok((path, msg))
}

mod commands {
    use anyhow::Context;
    use crate::cli::{InitArgs, ProjectArgs, ProjectCommands, PullFormatArgs, PushFormatArgs};
    use crate::config::Config;
    use crate::formats::Format;
    use crate::ir::Scope;
    use crate::store::{self, Store};
    use crate::sync;

    pub fn init(args: InitArgs) -> anyhow::Result<()> {
        let mut config = Config::load()?;
        let store_path = if let Some(p) = args.store {
            p
        } else {
            config.store_path()
        };

        if let Some(url) = &args.repo {
            // Clone remote repo into store path
            println!("Cloning {} → {}", url, store_path.display());
            sync::git_clone(url, &store_path)
                .with_context(|| format!("failed to clone {url}"))?;
            // If polyrc.toml doesn't exist in the clone, init it
            if !store_path.join("polyrc.toml").exists() {
                store::init_store(&store_path, Some(url))?;
            } else {
                // Update the manifest with the remote URL
                let mut manifest = crate::store::manifest::Manifest::load(&store_path)?;
                manifest.set_remote_url(url);
                manifest.save(&store_path)?;
            }
        } else {
            println!("Initializing local store at {}", store_path.display());
            store::init_store(&store_path, None)?;
        }

        config.set_store_path(&store_path)?;
        println!("Store ready at {}", store_path.display());
        Ok(())
    }

    pub fn push_format(args: PushFormatArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        let fmt = Format::from_str(&args.format)
            .with_context(|| format!("unknown format '{}'", args.format))?;

        let parser = fmt.parser();
        let mut rules = parser.parse(&args.input)
            .with_context(|| format!("failed to parse {} at {:?}", args.format, args.input))?;

        if let Some(scope_str) = &args.scope {
            let s = parse_scope(scope_str)?;
            rules.retain(|r| r.scope == s);
        }

        if rules.is_empty() {
            eprintln!("warning: no rules found");
            return Ok(());
        }

        let project_key = project_key(args.project.as_deref(), &args.scope);

        if args.dry_run {
            println!("Dry run: {} rule(s) from {} → store/{}", rules.len(), args.format,
                project_key.as_deref().unwrap_or("_user"));
            print_rules_preview(&rules);
            return Ok(());
        }

        let stored = store.save_rules(project_key.as_deref(), &rules, &args.format)?;
        println!("Stored {} rule(s) → {}", stored.len(), store_path.display());

        // Auto-commit
        let msg = format!(
            "push-format from {} ({})",
            args.format,
            chrono::Utc::now().format("%Y-%m-%d")
        );
        sync::git_commit(&store_path, &msg).context("git commit failed")?;
        println!("Committed: {}", msg);
        Ok(())
    }

    pub fn pull_format(args: PullFormatArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        let fmt = Format::from_str(&args.format)
            .with_context(|| format!("unknown format '{}'", args.format))?;

        let project_key = project_key(args.project.as_deref(), &args.scope);
        let mut rules = store.load_rules(project_key.as_deref())?;

        if let Some(scope_str) = &args.scope {
            let s = parse_scope(scope_str)?;
            rules.retain(|r| r.scope == s);
        }

        if rules.is_empty() {
            eprintln!("warning: no rules found in store for project {:?}", project_key);
            return Ok(());
        }

        if args.dry_run {
            println!("Dry run: {} rule(s) from store → {}", rules.len(), args.format);
            print_rules_preview(&rules);
            return Ok(());
        }

        let writer = fmt.writer();
        writer.write(&rules, &args.output)
            .with_context(|| format!("failed to write {} to {:?}", args.format, args.output))?;
        println!("Wrote {} rule(s) as {} to {}", rules.len(), args.format, args.output.display());
        Ok(())
    }

    pub fn push_store() -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        Store::open(&store_path).context("store not initialized")?;
        println!("Pushing store to remote...");
        sync::git_push(&store_path).context("git push failed")?;
        println!("Done.");
        Ok(())
    }

    pub fn pull_store() -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized")?;

        println!("Pulling from remote...");
        sync::git_pull(&store_path).context("git pull failed")?;

        // Run IR-level merge for each project
        for project in store.list_projects()? {
            let local = store.load_rules(Some(&project))?;
            // After git pull, files on disk ARE the merged state (git already merged).
            // Re-read and re-save to ensure IDs and metadata are consistent.
            if !local.is_empty() {
                let _ = store.save_rules(Some(&project), &local, "pull-store");
            }
        }
        println!("Pull complete.");
        Ok(())
    }

    pub fn project(args: ProjectArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized")?;

        match args.command {
            ProjectCommands::List => {
                let projects = store.list_projects()?;
                if projects.is_empty() {
                    println!("No projects in store.");
                } else {
                    println!("Projects in store:");
                    for p in &projects {
                        let rules = store.load_rules(Some(p)).unwrap_or_default();
                        println!("  {} ({} rule(s))", p, rules.len());
                    }
                }
            }
            ProjectCommands::Rename { old_name, new_name } => {
                store.rename_project(&old_name, &new_name)?;
                let msg = format!("rename project {} → {}", old_name, new_name);
                sync::git_commit(&store_path, &msg)?;
                println!("Renamed '{}' → '{}' and committed.", old_name, new_name);
            }
        }
        Ok(())
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Resolve the project key for store operations.
    /// User-scope rules use `_user`; everything else uses the --project name (or None).
    fn project_key(project: Option<&str>, scope: &Option<String>) -> Option<String> {
        if scope.as_deref().map(|s| s == "user").unwrap_or(false) {
            return None; // maps to _user dir
        }
        project.map(str::to_string)
    }

    fn parse_scope(s: &str) -> anyhow::Result<Scope> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Scope::User),
            "project" => Ok(Scope::Project),
            "path" => Ok(Scope::Path),
            other => anyhow::bail!("unknown scope '{}': expected user, project, or path", other),
        }
    }

    fn print_rules_preview(rules: &[crate::ir::Rule]) {
        for (i, rule) in rules.iter().enumerate() {
            println!("\n--- Rule {} ({:?}/{:?}) ---", i + 1, rule.scope, rule.activation);
            if let Some(n) = &rule.name { println!("name: {}", n); }
            if let Some(d) = &rule.description { println!("description: {}", d); }
            let preview = rule.content.len().min(200);
            println!("{}", &rule.content[..preview]);
            if rule.content.len() > 200 { println!("... ({} chars total)", rule.content.len()); }
        }
    }
}
