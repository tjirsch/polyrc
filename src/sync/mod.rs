use std::path::Path;
use std::process::Command;
use crate::error::{PolyrcError, Result};

fn run_git(args: &[&str], dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| PolyrcError::GitError {
            msg: format!("failed to run git: {e}"),
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(PolyrcError::GitError { msg: stderr })
    }
}

/// Initialize a new git repository at `path`.
pub fn git_init(path: &Path) -> Result<()> {
    run_git(&["init"], path)?;
    Ok(())
}

/// Clone `url` into `dest`.
///
/// If `dest` is already a git repo, the remote URL is updated to `url` instead
/// of re-cloning (idempotent re-init). Otherwise the parent directory is
/// created as needed before the clone.
pub fn git_clone(url: &str, dest: &Path) -> Result<()> {
    // Already a git repo → just point origin at the new URL
    if dest.join(".git").exists() {
        let set = run_git(&["remote", "set-url", "origin", url], dest);
        if set.is_err() {
            run_git(&["remote", "add", "origin", url], dest)?;
        }
        return Ok(());
    }

    // Create parent directory so that e.g. ~/.polyrc/ exists before cloning
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| PolyrcError::GitError {
            msg: format!("failed to create {}: {e}", parent.display()),
        })?;
    }

    let dest_str = dest.to_string_lossy().into_owned();

    // Use the home dir (or cwd) as the working directory — it is always
    // guaranteed to exist, unlike the not-yet-created dest parent.
    let work_dir = crate::config::home_dir();

    let output = Command::new("git")
        .args(["clone", url, &dest_str])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| PolyrcError::GitError {
            msg: format!("failed to run git clone: {e}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PolyrcError::GitError { msg: stderr });
    }
    Ok(())
}

/// Stage all changes and commit with `message`.
pub fn git_commit(store_path: &Path, message: &str) -> Result<()> {
    run_git(&["add", "-A"], store_path)?;

    // Check if there's anything to commit
    let status = run_git(&["status", "--porcelain"], store_path)?;
    if status.is_empty() {
        return Ok(()); // nothing to commit
    }

    run_git(&["commit", "-m", message], store_path)?;
    Ok(())
}

/// Push to the configured remote (origin).
///
/// Uses `--set-upstream` so it works correctly for both the initial push to an
/// empty remote and subsequent pushes.
pub fn git_push(store_path: &Path) -> Result<()> {
    run_git(&["push", "--set-upstream", "origin", "HEAD"], store_path)?;
    Ok(())
}

/// Pull from the configured remote (origin).
///
/// Fetches first to detect whether the remote has any commits.  If the remote
/// is empty (freshly initialised), the pull is skipped gracefully so that
/// `sync-store` does not fail on first use.
pub fn git_pull(store_path: &Path) -> Result<()> {
    // A fetch error (e.g. network, empty repo) is not fatal here — we just
    // won't have anything to merge, which is fine on first init.
    let _ = run_git(&["fetch", "origin"], store_path);

    // Check whether the remote branch actually exists yet.
    let has_remote = run_git(
        &["rev-parse", "--verify", "origin/main"],
        store_path,
    );
    if has_remote.is_err() {
        // Remote is empty or main doesn't exist yet — nothing to pull.
        return Ok(());
    }

    run_git(&["pull", "origin", "main"], store_path)?;
    Ok(())
}

