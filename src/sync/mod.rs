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

/// Pull remote changes into the store, handling conflicts automatically.
///
/// Strategy:
///  1. `git fetch` — not fatal if the remote is offline or empty.
///  2. If `origin/main` doesn't exist yet the remote is empty; skip pull.
///  3. If we are already up-to-date; skip merge.
///  4. `git merge -X ours --no-edit --allow-unrelated-histories origin/main`
///     — integrates all new files/commits from the remote and auto-resolves
///     any within-file conflicts by keeping the local version.
///     `--allow-unrelated-histories` handles remotes that were initialised
///     independently (e.g. via GitHub's "Add a README" checkbox).
///  5. On the rare merge failure (binary conflicts, etc.) the merge is aborted
///     and a clear, actionable error is returned.
pub fn git_pull(store_path: &Path) -> Result<()> {
    // Step 1: fetch — not fatal (offline, empty remote, etc.)
    let _ = run_git(&["fetch", "origin"], store_path);

    // Step 2: skip if remote has no main branch yet (freshly created repo)
    if run_git(&["rev-parse", "--verify", "origin/main"], store_path).is_err() {
        return Ok(());
    }

    // Step 3: skip if already up-to-date
    let behind = run_git(
        &["rev-list", "--count", "HEAD..origin/main"],
        store_path,
    )
    .unwrap_or_default();
    if behind.trim() == "0" {
        return Ok(());
    }

    // Step 4: merge, auto-resolving conflicts by preferring the local version
    // for any conflicting hunks within a file.  New files from either side are
    // always taken in full — only true per-line conflicts are affected by -X.
    let merge_result = run_git(
        &[
            "merge",
            "--no-edit",
            "-X", "ours",
            "--allow-unrelated-histories",
            "origin/main",
        ],
        store_path,
    );

    // Step 5: surface unresolvable conflicts clearly
    if let Err(e) = merge_result {
        // Leave no partial merge state behind
        let _ = run_git(&["merge", "--abort"], store_path);
        return Err(PolyrcError::GitError {
            msg: format!(
                "could not auto-merge remote changes into the store.\n\
                 Run `git -C {} mergetool` to resolve manually, then retry sync-store.\n\
                 Details: {e}",
                store_path.display()
            ),
        });
    }

    Ok(())
}

