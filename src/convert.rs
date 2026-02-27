use anyhow::Context;
use crate::cli::ConvertArgs;
use crate::config::Config;
use crate::formats::Format;
use crate::ir::Scope;
use crate::store::Store;
use crate::sync;

pub fn run(args: ConvertArgs) -> anyhow::Result<()> {
    // When --project is specified, route through the store (push-format + pull-format)
    if let Some(project) = args.project.clone() {
        return run_via_store(args, project);
    }

    // Ephemeral convert (no store)
    let from_name = args.from.as_str();
    let to_name = args.to.as_str();

    let from_format = Format::from_str(from_name)
        .with_context(|| format!("invalid --from format '{}'", from_name))?;
    let to_format = Format::from_str(to_name)
        .with_context(|| format!("invalid --to format '{}'", to_name))?;

    let parser = from_format.parser();
    let mut rules = parser
        .parse(&args.input)
        .with_context(|| format!("failed to parse {} config at {:?}", from_name, args.input))?;

    if let Some(scope_str) = &args.scope {
        let target_scope = parse_scope(scope_str)?;
        rules.retain(|r| r.scope == target_scope);
    }

    if rules.is_empty() {
        eprintln!("warning: no rules found after parsing");
        return Ok(());
    }

    if args.dry_run {
        println!("Dry run: {} rule(s) from {} → {}", rules.len(), from_name, to_name);
        print_rules_preview(&rules);
    } else {
        let writer = to_format.writer();
        writer.write(&rules, &args.output)
            .with_context(|| format!("failed to write {} config to {:?}", to_name, args.output))?;
        println!("Converted {} rule(s) from {} to {}", rules.len(), from_name, to_name);
    }
    Ok(())
}

/// Convert via store: push-format source → pull-format target.
fn run_via_store(args: ConvertArgs, project: String) -> anyhow::Result<()> {
    let config = Config::load()?;
    let store_path = config.store_path();
    let store = Store::open(&store_path, &crate::config::polyrc_dir())
        .context("store not initialized — run `polyrc init` first")?;

    let from_name = args.from.as_str();
    let to_name = args.to.as_str();

    let from_format = Format::from_str(from_name)
        .with_context(|| format!("invalid --from format '{}'", from_name))?;
    let to_format = Format::from_str(to_name)
        .with_context(|| format!("invalid --to format '{}'", to_name))?;

    // Parse source format
    let parser = from_format.parser();
    let mut rules = parser.parse(&args.input)
        .with_context(|| format!("failed to parse {} at {:?}", from_name, args.input))?;

    if let Some(scope_str) = &args.scope {
        let s = parse_scope(scope_str)?;
        rules.retain(|r| r.scope == s);
    }

    if rules.is_empty() {
        eprintln!("warning: no rules found after parsing");
        return Ok(());
    }

    if args.dry_run {
        println!(
            "Dry run: {} rule(s) from {} → store/{} → {}",
            rules.len(), from_name, project, to_name
        );
        print_rules_preview(&rules);
        return Ok(());
    }

    // Push to store
    let stored = store.save_rules(Some(&project), &rules, from_name)?;
    let msg = format!(
        "convert from {} ({})",
        from_name,
        chrono::Utc::now().format("%Y-%m-%d")
    );
    sync::git_commit(&store_path, &msg).context("git commit failed")?;

    // Pull from store as target format
    let mut stored_rules = stored;
    if let Some(scope_str) = &args.scope {
        stored_rules.retain(|r| r.scope == parse_scope(scope_str).unwrap_or(Scope::Project));
    }

    let writer = to_format.writer();
    writer.write(&stored_rules, &args.output)
        .with_context(|| format!("failed to write {} to {:?}", to_name, args.output))?;

    println!(
        "Converted {} rule(s): {} → store/{} → {}",
        stored_rules.len(), from_name, project, to_name
    );
    Ok(())
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
        let preview = rule.content.len().min(300);
        println!("{}", &rule.content[..preview]);
        if rule.content.len() > 300 { println!("... ({} chars total)", rule.content.len()); }
    }
}
