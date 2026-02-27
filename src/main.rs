use anyhow::Context;
use clap::Parser as ClapParser;

mod cli;
mod config;
mod convert;
mod discover;
mod error;
mod self_update;
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
        cli::Commands::SelfUpdate(a) => {
            self_update::run(a.check_only, a.skip_checksum).context("self-update failed")?
        }
        cli::Commands::SetEditor(a) => commands::set_editor(a)?,
        cli::Commands::SupportedFormats => {
            for fmt in formats::Format::all() {
                println!("{:<15} {}", fmt.name(), fmt.description());
            }
        }
        cli::Commands::Init(a) => commands::init(a)?,
        cli::Commands::PushFormat(a) => commands::push_format(a)?,
        cli::Commands::PullFormat(a) => commands::pull_format(a)?,
        cli::Commands::SyncStore => commands::sync_store()?,
        cli::Commands::ListStore(a) => commands::list_store(a)?,
        cli::Commands::SaveRule(a) => commands::save_rule(a)?,
        cli::Commands::PullRule(a) => commands::pull_rule(a)?,
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
    use crate::cli::{ActivationArg, InitArgs, ListStoreArgs, ProjectArgs, ProjectCommands, PullFormatArgs, PullRuleArgs, PushFormatArgs, SaveRuleArgs, ScopeArg, SetEditorArgs};
    use crate::config::Config;
    use crate::formats::Format;
    use crate::ir::Scope;
    use crate::store::{self, Store};
    use crate::sync;

    pub fn init(args: InitArgs) -> anyhow::Result<()> {
        let mut config = Config::load()?;
        // `init` is a setup command: always use the default store location
        // unless the user explicitly pins a different path with --store.
        // Reading the saved config here would silently reuse stale paths
        // from previous interrupted runs.
        let store_path = args.store.unwrap_or_else(crate::config::default_store_path);

        if let Some(url) = &args.repo {
            println!("Cloning {} → {}", url, store_path.display());
            sync::git_clone(url, &store_path)
                .with_context(|| format!("failed to clone {url}"))?;
            store::init_git(&store_path)?;
            config.init_store_config(Some(url));
        } else {
            println!("Initializing local store at {}", store_path.display());
            store::init_git(&store_path)?;
            config.init_store_config(None);
        }

        // Set store path + version/remote in a single save — no double-load overwrite
        config.store.path = Some(store_path.to_string_lossy().to_string());
        config.save().context("failed to save config")?;
        println!("Store ready at {}", store_path.display());
        Ok(())
    }

    pub fn push_format(args: PushFormatArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        if args.all {
            let mut pushed_names: Vec<&str> = vec![];
            for fmt in Format::all() {
                // Use format name as the namespace so origins stay clear
                let ns = fmt.name();
                match push_one(&store, &fmt, &args.input, &args.scope, args.dry_run, Some(ns)) {
                    Ok(0) => println!("  {} — skipped (no rules found)", fmt.name()),
                    Ok(n) => {
                        println!("  {} — {} rule(s) stored in {}/", fmt.name(), n, ns);
                        pushed_names.push(fmt.name());
                    }
                    Err(e) => eprintln!("  {} — error: {:#}", fmt.name(), e),
                }
            }
            if !args.dry_run && !pushed_names.is_empty() {
                let msg = format!(
                    "push-format --all ({}) ({})",
                    pushed_names.join(", "),
                    chrono::Utc::now().format("%Y-%m-%d")
                );
                sync::git_commit(&store_path, &msg).context("git commit failed")?;
                println!("Committed: {}", msg);
            }
        } else {
            let fmt_arg = args.format.expect("--format is required without --all");
            let fmt_name = fmt_arg.as_str();
            let fmt = Format::from_str(fmt_name)
                .with_context(|| format!("unknown format '{}'", fmt_name))?;
            let key = project_key(args.project.as_deref(), &args.scope);
            let n = push_one(&store, &fmt, &args.input, &args.scope, args.dry_run, key.as_deref())?;
            if n == 0 {
                eprintln!("warning: no rules found");
            } else if !args.dry_run {
                println!("Stored {} rule(s) → {}", n, store_path.display());
                let msg = format!(
                    "push-format from {} ({})",
                    fmt_name,
                    chrono::Utc::now().format("%Y-%m-%d")
                );
                sync::git_commit(&store_path, &msg).context("git commit failed")?;
                println!("Committed: {}", msg);
            }
        }
        Ok(())
    }

    /// Push one format into the store. Returns the number of rules stored (0 = nothing to push).
    fn push_one(
        store: &Store,
        fmt: &Format,
        input: &std::path::Path,
        scope: &Option<String>,
        dry_run: bool,
        project_key: Option<&str>,
    ) -> anyhow::Result<usize> {
        let fmt_name = fmt.name();
        let parser = fmt.parser();
        let mut rules = parser.parse(input)
            .with_context(|| format!("failed to parse {} at {}", fmt_name, input.display()))?;

        if let Some(scope_str) = scope {
            let s = parse_scope(scope_str)?;
            rules.retain(|r| r.scope == s);
        }

        if rules.is_empty() {
            return Ok(0);
        }

        if dry_run {
            println!("Dry run: {} rule(s) from {} → store/{}", rules.len(), fmt_name,
                project_key.unwrap_or(store::USER_PROJECT));
            print_rules_preview(&rules);
            return Ok(rules.len());
        }

        let stored = store.save_rules(project_key, &rules, fmt_name)?;
        Ok(stored.len())
    }

    pub fn pull_format(args: PullFormatArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        if args.all {
            for fmt in Format::all() {
                let key = project_key(args.project.as_deref(), &args.scope);
                match pull_one(&store, &fmt, &args.output, &args.scope, args.dry_run, key.as_deref()) {
                    Ok(0) => println!("  {} — skipped (no rules in store)", fmt.name()),
                    Ok(n) => println!("  {} — wrote {} rule(s)", fmt.name(), n),
                    Err(e) => eprintln!("  {} — error: {:#}", fmt.name(), e),
                }
            }
        } else {
            let fmt_arg = args.format.expect("--format is required without --all");
            let fmt_name = fmt_arg.as_str();
            let fmt = Format::from_str(fmt_name)
                .with_context(|| format!("unknown format '{}'", fmt_name))?;
            let key = project_key(args.project.as_deref(), &args.scope);
            let n = pull_one(&store, &fmt, &args.output, &args.scope, args.dry_run, key.as_deref())?;
            if n == 0 {
                eprintln!("warning: no rules found in store for project {:?}", key);
            } else {
                println!("Wrote {} rule(s) as {} to {}", n, fmt_name, args.output.display());
            }
        }
        Ok(())
    }

    /// Pull rules from the store and write them as one format. Returns the number of rules written.
    fn pull_one(
        store: &Store,
        fmt: &Format,
        output: &std::path::Path,
        scope: &Option<String>,
        dry_run: bool,
        project_key: Option<&str>,
    ) -> anyhow::Result<usize> {
        let fmt_name = fmt.name();
        let mut rules = store.load_rules(project_key)?;

        if let Some(scope_str) = scope {
            let s = parse_scope(scope_str)?;
            rules.retain(|r| r.scope == s);
        }

        if rules.is_empty() {
            return Ok(0);
        }

        if dry_run {
            println!("Dry run: {} rule(s) from store → {}", rules.len(), fmt_name);
            print_rules_preview(&rules);
            return Ok(rules.len());
        }

        let writer = fmt.writer();
        writer.write(&rules, output)
            .with_context(|| format!("failed to write {} to {}", fmt_name, output.display()))?;
        Ok(rules.len())
    }

    pub fn sync_store() -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized")?;

        // 1. Pull remote changes in first
        println!("Pulling from remote...");
        sync::git_pull(&store_path).context("git pull failed")?;

        // Re-save all projects after pull to normalise IDs and metadata
        for project in store.list_projects()? {
            let rules = store.load_rules(Some(&project))?;
            if !rules.is_empty() {
                let _ = store.save_rules(Some(&project), &rules, "sync-store");
            }
        }
        println!("Pull complete.");

        // 2. Push local commits to remote
        println!("Pushing to remote...");
        sync::git_push(&store_path).context("git push failed")?;
        println!("Sync complete.");
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

    pub fn list_store(args: ListStoreArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        // Determine which project keys to show.
        let all_projects = store.list_projects()?;
        let keys: Vec<String> = if args.user {
            vec![store::USER_PROJECT.to_string()]
        } else if args.projects {
            vec![store::PROJECTS_NAMESPACE.to_string()]
        } else if let Some(ref p) = args.project {
            vec![p.clone()]
        } else {
            // Show user first, then projects, then any other buckets
            let mut ordered = vec![];
            for preferred in [store::USER_PROJECT, store::PROJECTS_NAMESPACE] {
                if all_projects.iter().any(|k| k == preferred) {
                    ordered.push(preferred.to_string());
                }
            }
            for k in &all_projects {
                if k != store::USER_PROJECT && k != store::PROJECTS_NAMESPACE {
                    ordered.push(k.clone());
                }
            }
            ordered
        };

        let fmt_filter = args.format.as_ref().map(|f| f.as_str());
        let mut grand_total = 0usize;

        // Column widths
        const W_NAME: usize = 28;
        const W_SCOPE: usize = 7;
        const W_FMT: usize = 10;
        const W_ACT: usize = 10;
        const W_DATE: usize = 10;
        // PATH fills the rest

        let header = format!(
            "  {:<W_NAME$}  {:<W_SCOPE$}  {:<W_FMT$}  {:<W_ACT$}  {:<W_DATE$}  {}",
            "NAME", "SCOPE", "FORMAT", "ACTIVATION", "UPDATED", "PATH"
        );
        let divider = "─".repeat(header.len());

        for key in &keys {
            let project_arg = Some(key.as_str());
            let mut rules = store.load_rules(project_arg)?;

            // Apply --format filter.
            if let Some(fmt) = fmt_filter {
                rules.retain(|r| r.source_format.as_deref() == Some(fmt));
            }

            if rules.is_empty() {
                continue;
            }

            grand_total += rules.len();
            println!("\nPROJECT: {}", key);
            println!("{}", divider);
            println!("{}", header);
            println!("{}", divider);

            for rule in &rules {
                let name = rule.name.as_deref().unwrap_or("<unnamed>");
                let fmt_tag  = rule.source_format.as_deref().unwrap_or("?");
                let scope_tag = format!("{:?}", rule.scope).to_lowercase();
                let act_tag  = format!("{:?}", rule.activation).to_lowercase();
                let updated  = rule.updated_at.as_deref().unwrap_or("?");
                let date     = updated.get(..10).unwrap_or(updated);
                let path     = format!("{}/{}.yaml", key, rule.filename_stem());

                println!(
                    "  {:<W_NAME$}  {:<W_SCOPE$}  {:<W_FMT$}  {:<W_ACT$}  {:<W_DATE$}  {}",
                    name, scope_tag, fmt_tag, act_tag, date, path
                );

                if args.verbose {
                    let preview_len = rule.content.len().min(300);
                    let preview = &rule.content[..preview_len];
                    for line in preview.lines().take(6) {
                        println!("      {}", line);
                    }
                    if rule.content.len() > 300 || rule.content.lines().count() > 6 {
                        println!("      … ({} chars total)", rule.content.len());
                    }
                    println!();
                }
            }

            println!("{}", divider);
            println!("  {} rule(s)", rules.len());
        }

        if grand_total == 0 {
            println!("No rules found in the store matching the given filters.");
        } else {
            println!("\nTotal: {} rule(s)", grand_total);
        }

        Ok(())
    }

    pub fn save_rule(args: SaveRuleArgs) -> anyhow::Result<()> {
        use crate::ir::{Activation, Rule};
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        // Determine namespace
        let namespace = if args.namespace == "user" {
            store::USER_PROJECT
        } else {
            store::PROJECTS_NAMESPACE
        };

        // Build the rule
        let content = if let Some(ref file) = args.from_file {
            std::fs::read_to_string(file)
                .with_context(|| format!("failed to read {}", file.display()))?
        } else {
            anyhow::bail!("--from-file is required (interactive input not yet supported)");
        };

        let scope = match args.scope {
            ScopeArg::User    => crate::ir::Scope::User,
            ScopeArg::Project => crate::ir::Scope::Project,
            ScopeArg::Path    => crate::ir::Scope::Path,
        };
        let activation = match args.activation {
            ActivationArg::Always    => Activation::Always,
            ActivationArg::OnDemand  => Activation::OnDemand,
            ActivationArg::Glob      => Activation::Glob,
            ActivationArg::AiDecides => Activation::AiDecides,
        };

        let rule = Rule {
            name: Some(args.name.clone()),
            scope,
            activation,
            content: content.trim_end().to_string(),
            ..Default::default()
        };

        let stored = store.save_rule_to_namespace(namespace, &args.name, &rule)?;
        println!(
            "Saved '{}' → {}/{}/{}.yaml",
            args.name, store_path.display(), namespace, args.name
        );

        // Auto-commit
        sync::git_commit(&store_path, &format!("save-rule: {}", args.name))
            .context("git commit failed")?;

        println!("Stored: {} ({})", stored.name.as_deref().unwrap_or(&args.name), namespace);
        Ok(())
    }

    pub fn pull_rule(args: PullRuleArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        let (namespace, rule) = store.load_rule_by_name(&args.name)?
            .with_context(|| format!("rule '{}' not found in store (checked projects/ and user/)", args.name))?;

        let fmt = crate::formats::Format::from_str(args.format.as_str())
            .with_context(|| format!("unknown format '{}'", args.format.as_str()))?;
        let writer = fmt.writer();
        let target = std::env::current_dir().context("failed to get current directory")?;

        writer.write(std::slice::from_ref(&rule), &target)
            .with_context(|| format!("failed to write rule as {}", fmt.name()))?;

        println!(
            "Pulled '{}' from {} → {} format in {}",
            args.name, namespace, fmt.name(), target.display()
        );
        Ok(())
    }

    pub fn set_editor(args: SetEditorArgs) -> anyhow::Result<()> {
        let mut config = Config::load()?;
        if args.clear {
            config.preferred_editor = None;
            config.save().map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("preferred_editor cleared (falls back to $EDITOR / OS default).");
        } else if let Some(editor) = args.editor {
            config.preferred_editor = Some(editor.clone());
            config.save().map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("preferred_editor set to \"{}\".", editor);
        } else {
            match &config.preferred_editor {
                Some(e) => println!("preferred_editor = \"{}\"", e),
                None => println!("preferred_editor is not set (using $EDITOR / OS default)."),
            }
        }
        Ok(())
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Resolve the project key for store operations.
    /// User-scope rules use `store::USER_PROJECT`; everything else uses the --project name.
    fn project_key(project: Option<&str>, scope: &Option<String>) -> Option<String> {
        if scope.as_deref().map(|s| s == "user").unwrap_or(false) {
            return Some(store::USER_PROJECT.to_string());
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
