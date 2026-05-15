use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Map, Value};

use crate::error::VaultliError;
use crate::paths::{relative_path, resolve_root};
use crate::search::show_record;
use crate::util::which;

pub fn git_info(root: &Path, target: Option<&str>) -> Result<Map<String, Value>, VaultliError> {
    let root = resolve_root(root)?;
    let Some(git_path) = which("git") else {
        return Ok(unavailable(&root, "git executable not found"));
    };

    let file_path = resolve_target(&root, target);
    let Some(repo_root) = git_capture(&git_path, &root, &["rev-parse", "--show-toplevel"]) else {
        return Ok(unavailable(&root, "not a git repository"));
    };

    let branch = git_capture(&git_path, &root, &["branch", "--show-current"]);
    let head = git_capture(&git_path, &root, &["rev-parse", "HEAD"]);
    let dirty = git_capture(&git_path, &root, &["status", "--porcelain"])
        .map(|status| !status.is_empty())
        .unwrap_or(false);

    let mut result = Map::new();
    result.insert("root".into(), Value::String(root.display().to_string()));
    result.insert("available".into(), Value::Bool(true));
    result.insert("repo_root".into(), Value::String(repo_root.clone()));
    result.insert(
        "branch".into(),
        branch.map(Value::from).unwrap_or(Value::Null),
    );
    result.insert("head".into(), head.map(Value::from).unwrap_or(Value::Null));
    result.insert("dirty".into(), Value::Bool(dirty));

    if let Some(file_path) = file_path {
        let relative = relative_path(&file_path, Path::new(&repo_root))
            .unwrap_or_else(|_| file_path.display().to_string());
        let status = git_capture(&git_path, &root, &["status", "--short", "--", &relative])
            .unwrap_or_default();
        let tracked = git_capture(
            &git_path,
            &root,
            &["ls-files", "--error-unmatch", "--", &relative],
        )
        .is_some();
        let mut file = Map::new();
        file.insert(
            "path".into(),
            Value::String(file_path.display().to_string()),
        );
        file.insert("relative_path".into(), Value::String(relative.clone()));
        file.insert("status".into(), Value::String(status));
        file.insert("tracked".into(), Value::Bool(tracked));
        if let Some(raw) = git_capture(
            &git_path,
            &root,
            &["log", "-1", "--format=%H%x00%aI%x00%an", "--", &relative],
        ) {
            let mut parts = raw.split('\0');
            file.insert(
                "last_commit".into(),
                json!({
                    "hash": parts.next().unwrap_or_default(),
                    "committed_at": parts.next().unwrap_or_default(),
                    "author": parts.next().unwrap_or_default(),
                }),
            );
        }
        result.insert("file".into(), Value::Object(file));
    }

    Ok(result)
}

fn unavailable(root: &Path, reason: &str) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("root".into(), Value::String(root.display().to_string()));
    result.insert("available".into(), Value::Bool(false));
    result.insert("reason".into(), Value::String(reason.into()));
    result
}

fn resolve_target(root: &Path, target: Option<&str>) -> Option<PathBuf> {
    let target = target?;
    let candidate = PathBuf::from(target);
    let candidate = if candidate.is_absolute() {
        candidate
    } else {
        root.join(&candidate)
    };
    if candidate.exists() {
        return Some(candidate);
    }
    if let Ok(record) = show_record(root, target) {
        let file = record.get("file").and_then(Value::as_str)?;
        return Some(root.join(file));
    }
    Some(candidate)
}

fn git_capture(git_path: &Path, cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(git_path)
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string(),
    )
}
