pub mod manifest;

use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;
use crate::error::{PolyrcError, Result};
use crate::ir::Rule;
pub use manifest::Manifest;

const RULES_DIR: &str = "rules";
/// Directory name for user-scope rules (always-on ambient + on-demand commands).
pub const USER_PROJECT: &str = "user";
/// Legacy name — migrated to USER_PROJECT on first open.
const USER_PROJECT_LEGACY: &str = "_user";
/// Directory name for reusable named recipes (pulled into any project on demand).
pub const PROJECTS_NAMESPACE: &str = "projects";

/// The polyrc local store — a git repo containing IR rules as YAML files.
pub struct Store {
    /// Root of the store git repo (~/.polyrc/store or user-configured).
    pub path: PathBuf,
}

impl Store {
    /// Open an existing store at `path`. Errors if the store is not initialized.
    /// Automatically migrates legacy `_user/` directory to `user/` if present.
    pub fn open(path: &Path) -> Result<Self> {
        let manifest_path = path.join("polyrc.toml");
        if !manifest_path.exists() {
            return Err(PolyrcError::StoreNotFound);
        }
        Manifest::load(path)?;
        let store = Self { path: path.to_path_buf() };
        store.migrate_legacy_user_dir()?;
        Ok(store)
    }

    /// Rename `rules/_user/` → `rules/user/` if it still exists.
    fn migrate_legacy_user_dir(&self) -> Result<()> {
        let legacy = self.path.join(RULES_DIR).join(USER_PROJECT_LEGACY);
        let current = self.path.join(RULES_DIR).join(USER_PROJECT);
        if legacy.exists() && !current.exists() {
            fs::rename(&legacy, &current).map_err(|e| PolyrcError::Io {
                path: legacy.clone(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Load all rules for a given project key from the store.
    /// Use `None` for user-scope rules (maps to `_user/` directory).
    pub fn load_rules(&self, project: Option<&str>) -> Result<Vec<Rule>> {
        let dir = self.project_dir(project);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut rules = vec![];
        for entry in WalkDir::new(&dir).min_depth(1).max_depth(1).sort_by_file_name() {
            let entry = entry.map_err(|e| PolyrcError::Io {
                path: dir.clone(),
                source: e.into(),
            })?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("yml") {
                continue;
            }
            let raw = fs::read_to_string(p).map_err(|e| PolyrcError::Io {
                path: p.to_path_buf(),
                source: e,
            })?;
            let rule: Rule = serde_yml::from_str(&raw).map_err(|e| PolyrcError::YamlParse {
                path: p.to_path_buf(),
                source: e,
            })?;
            rules.push(rule);
        }
        Ok(rules)
    }

    /// Save rules for a project into the store.
    /// Existing rules not in the new set are removed. Auto-assigns IDs and timestamps.
    pub fn save_rules(&self, project: Option<&str>, rules: &[Rule], source_format: &str) -> Result<Vec<Rule>> {
        let dir = self.project_dir(project);
        fs::create_dir_all(&dir).map_err(|e| PolyrcError::Io {
            path: dir.clone(),
            source: e,
        })?;

        // Load existing rules to preserve IDs and created_at
        let existing = self.load_rules(project).unwrap_or_default();

        // Remove old files
        for entry in WalkDir::new(&dir).min_depth(1).max_depth(1) {
            if let Ok(e) = entry {
                let p = e.path();
                if p.extension().and_then(|ex| ex.to_str()) == Some("yml") {
                    fs::remove_file(p).map_err(|err| PolyrcError::Io {
                        path: p.to_path_buf(),
                        source: err,
                    })?;
                }
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        let project_key = project.unwrap_or(USER_PROJECT).to_string();

        let mut stored = vec![];
        for rule in rules {
            // Look up existing rule by name to preserve ID and created_at
            let existing_match = existing.iter().find(|e| {
                !e.id.is_empty() && e.name == rule.name
            });

            let mut r = rule.clone();
            r.project = Some(project_key.clone());
            r.source_format = Some(source_format.to_string());
            r.store_version = "1".to_string();

            if let Some(ex) = existing_match {
                r.id = ex.id.clone();
                r.created_at = ex.created_at.clone();
            } else {
                if r.id.is_empty() {
                    r.id = Uuid::new_v4().to_string();
                }
                r.created_at = Some(now.clone());
            }
            r.updated_at = Some(now.clone());

            let filename = format!("{}.yml", r.filename_stem());
            let file = dir.join(&filename);
            let content = serde_yml::to_string(&r).map_err(|e| PolyrcError::YamlParse {
                path: file.clone(),
                source: e,
            })?;
            fs::write(&file, content).map_err(|e| PolyrcError::Io {
                path: file,
                source: e,
            })?;
            stored.push(r);
        }
        Ok(stored)
    }

    /// Find a rule by name, searching `projects/` then `user/`.
    /// Returns `(namespace_key, rule)`.
    pub fn load_rule_by_name(&self, name: &str) -> Result<Option<(String, Rule)>> {
        for ns in [PROJECTS_NAMESPACE, USER_PROJECT] {
            let dir = self.path.join(RULES_DIR).join(ns);
            if !dir.exists() {
                continue;
            }
            for entry in WalkDir::new(&dir).min_depth(1).max_depth(1).sort_by_file_name() {
                let entry = entry.map_err(|e| PolyrcError::Io { path: dir.clone(), source: e.into() })?;
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("yml") {
                    continue;
                }
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem == name {
                    let raw = fs::read_to_string(p).map_err(|e| PolyrcError::Io { path: p.to_path_buf(), source: e })?;
                    let rule: Rule = serde_yml::from_str(&raw).map_err(|e| PolyrcError::YamlParse { path: p.to_path_buf(), source: e })?;
                    return Ok(Some((ns.to_string(), rule)));
                }
            }
        }
        Ok(None)
    }

    /// Save a single named rule into the given namespace (`projects` or `user`).
    /// Returns the stored rule (with id and timestamps set).
    pub fn save_rule_to_namespace(&self, namespace: &str, name: &str, rule: &Rule) -> Result<Rule> {
        let dir = self.path.join(RULES_DIR).join(namespace);
        fs::create_dir_all(&dir).map_err(|e| PolyrcError::Io { path: dir.clone(), source: e })?;

        let now = chrono::Utc::now().to_rfc3339();

        // Preserve existing id / created_at if rule already exists
        let existing = self.load_rule_by_name(name).unwrap_or(None);
        let mut r = rule.clone();
        r.project = Some(namespace.to_string());
        r.store_version = "1".to_string();

        match existing {
            Some((_, ex)) => {
                r.id = ex.id;
                r.created_at = ex.created_at;
            }
            None => {
                if r.id.is_empty() { r.id = Uuid::new_v4().to_string(); }
                r.created_at = Some(now.clone());
            }
        }
        r.updated_at = Some(now);
        if r.name.is_none() { r.name = Some(name.to_string()); }

        let filename = format!("{}.yml", name);
        let file = dir.join(&filename);
        let content = serde_yml::to_string(&r).map_err(|e| PolyrcError::YamlParse { path: file.clone(), source: e })?;
        fs::write(&file, content).map_err(|e| PolyrcError::Io { path: file, source: e })?;
        Ok(r)
    }

    /// List all project keys in the store (directory names under rules/).
    pub fn list_projects(&self) -> Result<Vec<String>> {
        let rules_dir = self.path.join(RULES_DIR);
        if !rules_dir.exists() {
            return Ok(vec![]);
        }
        let mut projects = vec![];
        for entry in WalkDir::new(&rules_dir).min_depth(1).max_depth(1) {
            let entry = entry.map_err(|e| PolyrcError::Io {
                path: rules_dir.clone(),
                source: e.into(),
            })?;
            if entry.file_type().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    projects.push(name.to_string());
                }
            }
        }
        projects.sort();
        Ok(projects)
    }

    /// Rename a project directory in the store.
    pub fn rename_project(&self, old_name: &str, new_name: &str) -> Result<()> {
        let old_dir = self.path.join(RULES_DIR).join(old_name);
        let new_dir = self.path.join(RULES_DIR).join(new_name);
        if !old_dir.exists() {
            return Err(PolyrcError::WriteFailure {
                path: old_dir,
                reason: "project not found".to_string(),
            });
        }
        if new_dir.exists() {
            return Err(PolyrcError::WriteFailure {
                path: new_dir,
                reason: "target project already exists".to_string(),
            });
        }
        fs::rename(&old_dir, &new_dir).map_err(|e| PolyrcError::Io {
            path: old_dir,
            source: e,
        })
    }

    fn project_dir(&self, project: Option<&str>) -> PathBuf {
        let key = project.unwrap_or(USER_PROJECT);
        self.path.join(RULES_DIR).join(key)
    }
}

/// Initialize a new store at `path` (git init + manifest).
pub fn init_store(path: &Path, remote_url: Option<&str>) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| PolyrcError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut manifest = Manifest::new();
    if let Some(url) = remote_url {
        manifest.set_remote_url(url);
    }
    manifest.save(path)?;

    // git init (only if not already a repo)
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        crate::sync::git_init(path)?;
    }

    Ok(())
}
