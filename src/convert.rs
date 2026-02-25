use anyhow::Context;
use crate::cli::ConvertArgs;
use crate::formats::Format;
use crate::ir::Scope;

pub fn run(args: ConvertArgs) -> anyhow::Result<()> {
    let from_format = Format::from_str(&args.from)
        .with_context(|| format!("invalid --from format '{}'", args.from))?;
    let to_format = Format::from_str(&args.to)
        .with_context(|| format!("invalid --to format '{}'", args.to))?;

    let parser = from_format.parser();
    let mut rules = parser
        .parse(&args.input)
        .with_context(|| format!("failed to parse {} config at {:?}", args.from, args.input))?;

    // Optional scope filter
    if let Some(scope_str) = &args.scope {
        let target_scope = parse_scope(scope_str)?;
        rules.retain(|r| r.scope == target_scope);
    }

    if rules.is_empty() {
        eprintln!("warning: no rules found after parsing");
        return Ok(());
    }

    if args.dry_run {
        println!(
            "Dry run: {} rule(s) from {} → {}",
            rules.len(),
            args.from,
            args.to
        );
        for (i, rule) in rules.iter().enumerate() {
            println!(
                "\n--- Rule {} ({:?}/{:?}) ---",
                i + 1,
                rule.scope,
                rule.activation
            );
            if let Some(name) = &rule.name {
                println!("name: {}", name);
            }
            if let Some(desc) = &rule.description {
                println!("description: {}", desc);
            }
            if let Some(globs) = &rule.globs {
                println!("globs: {:?}", globs);
            }
            let preview_len = rule.content.len().min(300);
            println!("{}", &rule.content[..preview_len]);
            if rule.content.len() > 300 {
                println!("... ({} chars total)", rule.content.len());
            }
        }
    } else {
        let writer = to_format.writer();
        writer
            .write(&rules, &args.output)
            .with_context(|| format!("failed to write {} config to {:?}", args.to, args.output))?;
        println!(
            "Converted {} rule(s) from {} to {}",
            rules.len(),
            args.from,
            args.to
        );
    }

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
