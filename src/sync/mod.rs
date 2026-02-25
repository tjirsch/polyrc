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
pub fn git_clone(url: &str, dest: &Path) -> Result<()> {
    // Run from parent directory
    let parent = dest.parent().unwrap_or(Path::new("."));
    let dest_str = dest.to_string_lossy();
    let output = Command::new("git")
        .args(["clone", url, &dest_str])
        .current_dir(parent)
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
pub fn git_push(store_path: &Path) -> Result<()> {
    run_git(&["push", "origin"], store_path)?;
    Ok(())
}

/// Pull from the configured remote (origin).
pub fn git_pull(store_path: &Path) -> Result<()> {
    run_git(&["pull", "origin"], store_path)?;
    Ok(())
}

/// Add a remote to the store repo.
pub fn git_add_remote(store_path: &Path, url: &str) -> Result<()> {
    // Remove existing origin if present, ignore error
    let _ = run_git(&["remote", "remove", "origin"], store_path);
    run_git(&["remote", "add", "origin", url], store_path)?;
    Ok(())
}

/// Return a human-readable git status string.
pub fn git_status(store_path: &Path) -> Result<String> {
    let branch = run_git(&["branch", "--show-current"], store_path).unwrap_or_else(|_| "unknown".to_string());
    let status = run_git(&["status", "--short"], store_path).unwrap_or_default();
    let log = run_git(
        &["log", "--oneline", "-5"],
        store_path,
    ).unwrap_or_else(|_| "(no commits yet)".to_string());

    let remote_info = run_git(
        &["status", "--short", "--branch"],
        store_path,
    ).unwrap_or_default();

    Ok(format!(
        "Branch: {branch}\n\nRecent commits:\n{log}\n\nWorking tree:\n{}\n\n{remote_info}",
        if status.is_empty() { "(clean)".to_string() } else { status }
    ))
}
