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
        cli::Commands::Sync(a) => commands::sync(a)?,
        cli::Commands::ListProject(a) => commands::list_project(a)?,
        cli::Commands::PushRule(a) => commands::push_rule(a)?,
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
    use crate::cli::{ActivationArg, InitArgs, ListProjectArgs, ProjectArgs, ProjectCommands, PullFormatArgs, PullRuleArgs, PushFormatArgs, PushRuleArgs, SetEditorArgs, SyncArgs};
    use crate::config::Config;
    use crate::formats::Format;
    use crate::ir::Scope;
    use crate::store::{self, Store};
    use crate::sync;

    /// Normalize a project name to camelCase, stripping invalid characters.
    /// Rejects empty results and the reserved name "user".
    fn normalize_project_name(input: &str) -> anyhow::Result<String> {
        let segments: Vec<&str> = input
            .split(|c: char| matches!(c, ' ' | '\t' | '_' | '-' | '/' | '\\' | '.'))
            .filter(|s| !s.is_empty())
            .collect();

        if segments.is_empty() {
            anyhow::bail!("project name '{}' is empty after normalization", input);
        }

        let mut result = String::new();
        for seg in &segments {
            // Drop non-alphanumeric chars within each segment
            let cleaned: String = seg.chars().filter(|c| c.is_alphanumeric()).collect();
            if cleaned.is_empty() {
                continue;
            }
            if result.is_empty() {
                // First word: all lowercase
                result.push_str(&cleaned.to_lowercase());
            } else {
                // Subsequent words: capitalize first char, lowercase rest
                let mut chars = cleaned.chars();
                if let Some(first) = chars.next() {
                    result.push_str(&first.to_uppercase().to_string());
                    result.push_str(&chars.as_str().to_lowercase());
                }
            }
        }

        if result.is_empty() {
            anyhow::bail!("project name '{}' contains no valid characters", input);
        }
        if result == store::USER_PROJECT {
            anyhow::bail!(
                "'user' is a reserved project name; use --user for user-scope operations"
            );
        }
        Ok(result)
    }

    pub fn init(args: InitArgs) -> anyhow::Result<()> {
        let mut config = Config::load()?;
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

        config.store.path = Some(store_path.to_string_lossy().to_string());
        config.save().context("failed to save config")?;
        println!("Store ready at {}", store_path.display());
        Ok(())
    }

    pub fn push_format(args: PushFormatArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        // Determine routing
        let (user_mode, project_key) = resolve_routing(args.user, args.project.as_deref())?;

        if args.all {
            let mut pushed_names: Vec<&str> = vec![];
            for fmt in Format::all() {
                match push_one(&store, &fmt, &args.input, user_mode, args.dry_run, &project_key) {
                    Ok(0) => {} // push_one already printed the reason
                    Ok(_) => pushed_names.push(fmt.name()),
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
            let n = push_one(&store, &fmt, &args.input, user_mode, args.dry_run, &project_key)?;
            if n > 0 && !args.dry_run {
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
        user: bool,
        dry_run: bool,
        project_key: &str,
    ) -> anyhow::Result<usize> {
        let fmt_name = fmt.name();

        // Auto-detect user input dir when --user and --input is the default "."
        let user_dir;
        let effective_input: &std::path::Path = if user && input == std::path::Path::new(".") {
            match fmt.user_input_dir() {
                Some(dir) => { user_dir = dir; &user_dir }
                None => {
                    println!("  {} — skipped (no local user-level config; use --input to specify)", fmt_name);
                    return Ok(0);
                }
            }
        } else {
            input
        };

        let parser = fmt.parser();
        let mut rules = parser.parse(effective_input)
            .with_context(|| format!("failed to parse {} at {}", fmt_name, effective_input.display()))?;

        // When using --user, filter to user-scope rules only
        if user {
            rules.retain(|r| r.scope == Scope::User);
        }

        if rules.is_empty() {
            println!("  {} — skipped (no rules found)", fmt_name);
            return Ok(0);
        }

        if dry_run {
            println!("  {} — dry run: {} rule(s) → store/{}", fmt_name, rules.len(), project_key);
            print_rules_preview(&rules);
            return Ok(rules.len());
        }

        let stored = store.save_rules(Some(project_key), &rules, fmt_name)?;
        println!("  {} — stored {} rule(s) → store/{}", fmt_name, stored.len(), project_key);
        Ok(stored.len())
    }

    pub fn pull_format(args: PullFormatArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        let (user_mode, project_key) = resolve_routing(args.user, args.project.as_deref())?;

        if args.all {
            for fmt in Format::all() {
                match pull_one(&store, &fmt, &args.output, user_mode, args.dry_run, &project_key) {
                    Ok(_) => {} // pull_one prints its own per-format status
                    Err(e) => eprintln!("  {} — error: {:#}", fmt.name(), e),
                }
            }
        } else {
            let fmt_arg = args.format.expect("--format is required without --all");
            let fmt_name = fmt_arg.as_str();
            let fmt = Format::from_str(fmt_name)
                .with_context(|| format!("unknown format '{}'", fmt_name))?;
            pull_one(&store, &fmt, &args.output, user_mode, args.dry_run, &project_key)?;
        }
        Ok(())
    }

    /// Pull rules from the store and write them as one format. Returns the number of rules written.
    fn pull_one(
        store: &Store,
        fmt: &Format,
        output: &std::path::Path,
        user: bool,
        dry_run: bool,
        project_key: &str,
    ) -> anyhow::Result<usize> {
        let fmt_name = fmt.name();
        let mut rules = store.load_rules(Some(project_key))?;

        // When using --user, filter to user-scope rules only
        if user {
            rules.retain(|r| r.scope == Scope::User);
        }

        if rules.is_empty() {
            println!("  {} — skipped (no rules in store)", fmt_name);
            return Ok(0);
        }

        // Auto-detect user output dir when --user and output is the default "."
        let user_dir;
        let effective_output: &std::path::Path = if user && output == std::path::Path::new(".") {
            match fmt.user_input_dir() {
                Some(dir) => { user_dir = dir; &user_dir }
                None => {
                    println!("  {} — skipped (no local user-level config; use --output to specify)", fmt_name);
                    return Ok(0);
                }
            }
        } else {
            output
        };

        if dry_run {
            println!("  {} — dry run: {} rule(s) from store → {}", fmt_name, rules.len(), effective_output.display());
            print_rules_preview(&rules);
            return Ok(rules.len());
        }

        let writer = fmt.writer();
        writer.write(&rules, effective_output)
            .with_context(|| format!("failed to write {} to {}", fmt_name, effective_output.display()))?;
        println!("  {} — wrote {} rule(s) to {}", fmt_name, rules.len(), effective_output.display());
        Ok(rules.len())
    }

    pub fn sync(args: SyncArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized")?;

        if !args.push_only {
            // Pull phase
            println!("Pulling from remote...");
            sync::git_pull(&store_path).context("git pull failed")?;

            // Re-save all projects after pull to normalise IDs and metadata
            for project in store.list_projects()? {
                let rules = store.load_rules(Some(&project))?;
                if !rules.is_empty() {
                    let _ = store.save_rules(Some(&project), &rules, "sync");
                }
            }
            println!("Pull complete.");
        }

        if !args.pull_only {
            // Push phase
            println!("Pushing to remote...");
            sync::git_push(&store_path).context("git push failed")?;
            println!("Push complete.");
        }

        if !args.push_only && !args.pull_only {
            println!("Sync complete.");
        }
        Ok(())
    }

    pub fn project(args: ProjectArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized")?;

        match args.command {
            ProjectCommands::Rename { old_name, new_name } => {
                let old_norm = normalize_project_name(&old_name)
                    .with_context(|| format!("invalid old project name '{}'", old_name))?;
                let new_norm = normalize_project_name(&new_name)
                    .with_context(|| format!("invalid new project name '{}'", new_name))?;
                store.rename_project(&old_norm, &new_norm)?;
                let msg = format!("rename project {} → {}", old_norm, new_norm);
                sync::git_commit(&store_path, &msg)?;
                println!("Renamed '{}' → '{}' and committed.", old_norm, new_norm);
            }
        }
        Ok(())
    }

    pub fn list_project(args: ListProjectArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        if let Some(ref name) = args.name {
            // Show rules for a specific project (name can be "user")
            let rules = store.load_rules(Some(name))?;
            if rules.is_empty() {
                println!("No rules in project '{}'.", name);
                return Ok(());
            }

            const W_NAME: usize = 28;
            const W_SCOPE: usize = 7;
            const W_FMT: usize = 10;
            const W_ACT: usize = 10;
            const W_DATE: usize = 10;

            let header = format!(
                "  {:<W_NAME$}  {:<W_SCOPE$}  {:<W_FMT$}  {:<W_ACT$}  {:<W_DATE$}  {}",
                "NAME", "SCOPE", "FORMAT", "ACTIVATION", "UPDATED", "PATH"
            );
            let divider = "─".repeat(header.len());

            println!("PROJECT: {} ({} rule(s))", name, rules.len());
            println!("{}", divider);
            println!("{}", header);
            println!("{}", divider);

            for rule in &rules {
                let rule_name = rule.name.as_deref().unwrap_or("<unnamed>");
                let fmt_tag   = rule.source_format.as_deref().unwrap_or("?");
                let scope_tag = format!("{:?}", rule.scope).to_lowercase();
                let act_tag   = format!("{:?}", rule.activation).to_lowercase();
                let updated   = rule.updated_at.as_deref().unwrap_or("?");
                let date      = updated.get(..10).unwrap_or(updated);
                let path      = format!("{}/{}.yaml", name, rule.filename_stem());

                println!(
                    "  {:<W_NAME$}  {:<W_SCOPE$}  {:<W_FMT$}  {:<W_ACT$}  {:<W_DATE$}  {}",
                    rule_name, scope_tag, fmt_tag, act_tag, date, path
                );

                if args.verbose {
                    // Print full content
                    for line in rule.content.lines() {
                        println!("      {}", line);
                    }
                    println!();
                }
            }

            println!("{}", divider);
            println!("  {} rule(s)", rules.len());
        } else {
            // List all projects
            let all_projects = store.list_projects()?;
            if all_projects.is_empty() {
                println!("No projects in store.");
                return Ok(());
            }

            // user first, then alphabetical
            let mut ordered = all_projects.clone();
            if let Some(pos) = ordered.iter().position(|n| n == store::USER_PROJECT) {
                ordered.remove(pos);
                ordered.insert(0, store::USER_PROJECT.to_string());
            }

            println!("Projects in store:");
            for p in &ordered {
                let rules = store.load_rules(Some(p)).unwrap_or_default();
                if args.verbose {
                    println!("  {} ({} rule(s)):", p, rules.len());
                    for r in &rules {
                        println!("    - {}", r.name.as_deref().unwrap_or("<unnamed>"));
                    }
                } else {
                    println!("  {} ({} rule(s))", p, rules.len());
                }
            }
            println!("\nTotal: {} project(s)", ordered.len());
        }
        Ok(())
    }

    pub fn push_rule(args: PushRuleArgs) -> anyhow::Result<()> {
        use crate::ir::{Activation, Rule};
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        // Determine destination namespace
        let namespace_owned;
        let namespace: &str = if args.user {
            store::USER_PROJECT
        } else if let Some(ref p) = args.project {
            namespace_owned = normalize_project_name(p)
                .with_context(|| format!("invalid project name '{}'", p))?;
            &namespace_owned
        } else {
            anyhow::bail!("specify --user or --project <name> to choose where to store this rule");
        };

        let scope = if args.user {
            Scope::User
        } else {
            Scope::Project
        };

        let content = if let Some(ref file) = args.from_file {
            std::fs::read_to_string(file)
                .with_context(|| format!("failed to read {}", file.display()))?
        } else {
            anyhow::bail!("--from-file is required (interactive input not yet supported)");
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
            "Pushed '{}' → {}/{}/{}.yaml",
            args.name, store_path.display(), namespace, args.name
        );

        sync::git_commit(&store_path, &format!("push-rule: {}", args.name))
            .context("git commit failed")?;

        println!("Stored: {} ({})", stored.name.as_deref().unwrap_or(&args.name), namespace);
        Ok(())
    }

    pub fn pull_rule(args: PullRuleArgs) -> anyhow::Result<()> {
        let config = Config::load()?;
        let store_path = config.store_path();
        let store = Store::open(&store_path).context("store not initialized — run `polyrc init` first")?;

        // Determine which namespace to search
        let search_ns: Option<String> = if args.user {
            Some(store::USER_PROJECT.to_string())
        } else if let Some(ref p) = args.project {
            let norm = normalize_project_name(p)
                .with_context(|| format!("invalid project name '{}'", p))?;
            Some(norm)
        } else {
            None // search all
        };

        let (namespace, rule) = store.load_rule_by_name(&args.name, search_ns.as_deref())?
            .with_context(|| {
                let location = search_ns.as_deref()
                    .map(|ns| format!("in project '{}'", ns))
                    .unwrap_or_else(|| "in any project".to_string());
                format!("rule '{}' not found {}", args.name, location)
            })?;

        let fmt = crate::formats::Format::from_str(args.format.as_str())
            .with_context(|| format!("unknown format '{}'", args.format.as_str()))?;
        let writer = fmt.writer();

        let target = if let Some(ref out) = args.output {
            out.clone()
        } else {
            std::env::current_dir().context("failed to get current directory")?
        };

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

    /// Resolve (user_mode, project_key) from --user / --project flags.
    /// Errors if neither is given.
    fn resolve_routing(user: bool, project: Option<&str>) -> anyhow::Result<(bool, String)> {
        if user {
            Ok((true, store::USER_PROJECT.to_string()))
        } else if let Some(p) = project {
            let norm = normalize_project_name(p)?;
            Ok((false, norm))
        } else {
            anyhow::bail!("specify --user or --project <name> to choose where to store/load rules")
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
