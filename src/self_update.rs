use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

const REPO: &str = "tjirsch/polyrc";
const INSTALLER: &str = "polyrc-installer.sh";
const API_BASE: &str = "https://api.github.com/repos";

pub fn run(check_only: bool, skip_checksum: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("Current version: {}", current);
    print!("Checking for updates... ");

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("polyrc/{}", current))
        .build()
        .context("failed to build HTTP client")?;

    let url = format!("{}/{}/releases/latest", API_BASE, REPO);
    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .context("GitHub API request failed")?
        .json()
        .context("failed to parse GitHub API response")?;

    let latest_tag = resp["tag_name"]
        .as_str()
        .context("GitHub release had no tag_name")?;
    let latest = latest_tag.trim_start_matches('v');

    if compare_versions(current, latest) >= 0 {
        println!("you are up to date ({})", current);
        return Ok(());
    }

    println!("update available: {} → {}", current, latest);

    if check_only {
        println!("Run `polyrc self-update` to install.");
        return Ok(());
    }

    // Locate installer and optional checksum sidecar in the release assets
    let assets = resp["assets"]
        .as_array()
        .context("GitHub release had no assets")?;

    let installer_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(INSTALLER))
        .and_then(|a| a["browser_download_url"].as_str())
        .with_context(|| {
            format!("installer '{}' not found in release {}", INSTALLER, latest_tag)
        })?
        .to_string();

    let checksum_name = format!("{}.sha256", INSTALLER);
    let checksum_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(checksum_name.as_str()))
        .and_then(|a| a["browser_download_url"].as_str())
        .map(str::to_string);

    // Download installer bytes
    println!("Downloading {}...", INSTALLER);
    let installer_bytes = client
        .get(&installer_url)
        .send()
        .context("failed to download installer")?
        .bytes()
        .context("failed to read installer bytes")?;

    // Verify SHA-256
    match checksum_url {
        Some(url) => {
            let sidecar = client
                .get(&url)
                .send()
                .context("failed to download checksum sidecar")?
                .text()
                .context("failed to read checksum sidecar")?;

            let expected = sidecar
                .split_whitespace()
                .next()
                .context("malformed SHA-256 sidecar")?;

            let mut hasher = Sha256::new();
            hasher.update(&installer_bytes);
            let actual = hex::encode(hasher.finalize());

            if actual != expected {
                bail!(
                    "checksum mismatch — installer may have been tampered with\n  expected: {}\n  actual:   {}",
                    expected,
                    actual
                );
            }
            println!("Checksum verified.");
        }
        None if !skip_checksum => {
            bail!(
                "no SHA-256 sidecar found for release {}; use --skip-checksum to install anyway",
                latest_tag
            );
        }
        None => {
            eprintln!("warning: no checksum sidecar found — proceeding without verification");
        }
    }

    // Run installer (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let tmp = std::env::temp_dir()
            .join(format!("polyrc-installer-{}.sh", std::process::id()));

        std::fs::write(&tmp, &installer_bytes)
            .with_context(|| format!("failed to write installer to {}", tmp.display()))?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            .context("failed to chmod installer")?;

        let status = std::process::Command::new("sh")
            .arg(&tmp)
            .status()
            .context("failed to run installer")?;

        std::fs::remove_file(&tmp).ok();

        if !status.success() {
            bail!("installer exited with status {}", status);
        }

        println!(
            "Updated to {}. You may need to `source ~/.profile` or open a new shell.",
            latest
        );
    }

    #[cfg(not(unix))]
    bail!("self-update is only supported on Unix (macOS / Linux)");

    Ok(())
}

/// Numeric semver comparison: returns >0 if a > b, 0 if equal, <0 if a < b.
fn compare_versions(a: &str, b: &str) -> i32 {
    let parse = |s: &str| -> (u64, u64, u64) {
        let mut parts = s.trim_start_matches('v').splitn(3, '.');
        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts
            .next()
            .and_then(|p| p.split('-').next()?.parse().ok())
            .unwrap_or(0);
        (major, minor, patch)
    };
    let av = parse(a);
    let bv = parse(b);
    if av > bv {
        1
    } else if av < bv {
        -1
    } else {
        0
    }
}
