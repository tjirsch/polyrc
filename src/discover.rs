use std::path::PathBuf;

use anyhow::Result;

use crate::cli::DiscoverArgs;
use crate::formats::Format;

// ── types ─────────────────────────────────────────────────────────────────────

/// A single candidate location for a user-level config of one format.
pub enum UserLocation {
    /// A single config file (plain text or JSON).
    File {
        path: PathBuf,
        /// Extra context shown after the status (e.g. "edit via Settings UI").
        note: Option<&'static str>,
    },
    /// A flat directory whose direct *.ext children are config files.
    Dir { path: PathBuf, extension: &'static str },
    /// A directory where each subdirectory may contain a SKILL.md (Claude skills layout).
    SkillDir { path: PathBuf },
    /// Stored in a web / app UI — no local file to scan.
    WebUi { hint: &'static str },
}

// ── per-format user locations ─────────────────────────────────────────────────

/// Returns the canonical user-level config locations for `fmt` on the current OS.
pub fn user_locations(fmt: &Format) -> Vec<UserLocation> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));

    match fmt {
        Format::Claude => {
            // The config dir can be overridden via CLAUDE_CONFIG_DIR; fall back to ~/.claude
            let claude_dir = std::env::var("CLAUDE_CONFIG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join(".claude"));

            // Managed/system-level settings path varies by OS
            #[cfg(target_os = "macos")]
            let managed = PathBuf::from("/Library/Application Support/ClaudeCode/managed-settings.json");
            #[cfg(target_os = "linux")]
            let managed = PathBuf::from("/etc/claude-code/managed-settings.json");
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            let managed = PathBuf::from("C:\\Program Files\\ClaudeCode\\managed-settings.json");

            vec![
                // Global user config (outside ~/.claude/) — auth, theme, per-project state
                UserLocation::File {
                    path: home.join(".claude.json"),
                    note: Some("global user config — auth, theme, per-project state"),
                },
                // User settings (permissions, model, env, hooks, …)
                UserLocation::File {
                    path: claude_dir.join("settings.json"),
                    note: Some("user settings — permissions, model, env, hooks"),
                },
                // Main memory / instruction file
                UserLocation::File {
                    path: claude_dir.join("CLAUDE.md"),
                    note: None,
                },
                // Modular always-on rules
                UserLocation::Dir {
                    path: claude_dir.join("rules"),
                    extension: "md",
                },
                // Slash-command files (on-demand)
                UserLocation::Dir {
                    path: claude_dir.join("commands"),
                    extension: "md",
                },
                // Modern skills (each skill is a subdirectory containing SKILL.md)
                UserLocation::SkillDir {
                    path: claude_dir.join("skills"),
                },
                // Subagent definitions
                UserLocation::Dir {
                    path: claude_dir.join("agents"),
                    extension: "md",
                },
                // Managed settings (org/MDM — cannot be overridden)
                UserLocation::File {
                    path: managed,
                    note: Some("managed settings — org/MDM enforced, cannot be overridden"),
                },
            ]
        }

        Format::Gemini => vec![UserLocation::File {
            path: home.join(".gemini/GEMINI.md"),
            note: None,
        }],

        Format::Antigravity => vec![UserLocation::Dir {
            path: home.join(".gemini/antigravity/rules"),
            extension: "md",
        }],

        Format::Windsurf => vec![UserLocation::File {
            path: home.join(".codeium/windsurf/memories/global_rules.md"),
            note: None,
        }],

        Format::Cursor => {
            // User rules live inside the VS Code–style settings JSON, not a standalone file.
            let settings = dirs::config_dir()
                .unwrap_or_else(|| home.join("Library/Application Support"))
                .join("Cursor/User/settings.json");
            vec![UserLocation::File {
                path: settings,
                note: Some("user rules embedded in JSON — edit via Cursor Settings UI"),
            }]
        }

        Format::Copilot => vec![UserLocation::WebUi {
            hint: "github.com → Settings → Copilot → Personal instructions",
        }],
    }
}

// ── command entry point ───────────────────────────────────────────────────────

pub fn run(args: DiscoverArgs) -> Result<()> {
    // --user is shorthand for --scope user
    let scope = if args.user {
        "user".to_string()
    } else if let Some(ref s) = args.scope {
        s.clone()
    } else {
        anyhow::bail!(
            "specify --scope user (or --user) to discover user-level configs\n\
             (project-scope discovery planned for future versions)"
        );
    };

    if scope != "user" {
        anyhow::bail!("only --scope user is supported currently");
    }

    let formats: Vec<Format> = if let Some(ref fmt_arg) = args.format {
        let fmt = Format::from_str(fmt_arg.as_str())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        vec![fmt]
    } else {
        Format::all().to_vec()
    };

    let header = if args.format.is_some() {
        format!("User-level configs for {}:", formats[0].name())
    } else {
        "User-level configs (all formats):".to_string()
    };
    println!("{}\n", header);

    for fmt in &formats {
        println!("  {}:", fmt.name());
        let locs = user_locations(fmt);
        if locs.is_empty() {
            println!("    (no user-level config locations defined)");
        }
        for loc in &locs {
            print_location(loc);
        }
        println!();
    }

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn print_location(loc: &UserLocation) {
    match loc {
        UserLocation::File { path, note } => {
            let display = tilde(path);
            if path.exists() {
                let lines = line_count(path).unwrap_or(0);
                let note_str = note.map(|n| format!("  [{}]", n)).unwrap_or_default();
                println!("    {:<60}  found  ({} lines){}", display, lines, note_str);
            } else {
                println!("    {:<60}  not found", display);
            }
        }

        UserLocation::Dir { path, extension } => {
            let display = format!("{}/", tilde(path));
            if path.exists() {
                match dir_files(path, extension) {
                    Ok(files) if files.is_empty() => {
                        println!("    {:<60}  found  (empty)", display);
                    }
                    Ok(files) => {
                        let names: Vec<_> = files
                            .iter()
                            .filter_map(|p| p.file_name()?.to_str().map(str::to_string))
                            .collect();
                        println!(
                            "    {:<60}  found  ({} file(s): {})",
                            display,
                            names.len(),
                            names.join(", ")
                        );
                    }
                    Err(_) => {
                        println!("    {:<60}  found  (unreadable)", display);
                    }
                }
            } else {
                println!("    {:<60}  not found", display);
            }
        }

        UserLocation::SkillDir { path } => {
            let display = format!("{}/", tilde(path));
            if path.exists() {
                match skill_subdirs(path) {
                    Ok(skills) if skills.is_empty() => {
                        println!("    {:<60}  found  (empty)", display);
                    }
                    Ok(skills) => {
                        let names: Vec<_> = skills.iter().map(|s| s.as_str()).collect();
                        println!(
                            "    {:<60}  found  ({} skill(s): {})",
                            display,
                            names.len(),
                            names.join(", ")
                        );
                    }
                    Err(_) => {
                        println!("    {:<60}  found  (unreadable)", display);
                    }
                }
            } else {
                println!("    {:<60}  not found", display);
            }
        }

        UserLocation::WebUi { hint } => {
            println!("    web UI  →  {}", hint);
        }
    }
}

/// Replace the home directory prefix with `~`.
fn tilde(path: &PathBuf) -> String {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    match path.strip_prefix(&home) {
        Ok(rel) => format!("~/{}", rel.display()),
        Err(_) => path.display().to_string(),
    }
}

fn line_count(path: &PathBuf) -> Result<usize> {
    Ok(std::fs::read_to_string(path)?.lines().count())
}

fn dir_files(dir: &PathBuf, ext: &str) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some(ext))
        .collect();
    files.sort();
    Ok(files)
}

/// Returns names of subdirectories that contain a SKILL.md file.
fn skill_subdirs(dir: &PathBuf) -> Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join("SKILL.md").exists())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}
