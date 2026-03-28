use std::fs;
use std::path::{Path, PathBuf};

use crate::error::VaultliError;
use crate::util::{INDEX_FILENAME, VAULT_MARKER};

pub fn find_root(start: Option<&Path>) -> Result<PathBuf, VaultliError> {
    let current = start
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .canonicalize()?;

    for candidate in current.ancestors() {
        if candidate.join(VAULT_MARKER).exists() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(VaultliError::RootNotFound(current.display().to_string()))
}

pub(crate) fn resolve_root(root: &Path) -> Result<PathBuf, VaultliError> {
    if root.join(VAULT_MARKER).exists() {
        return Ok(root.canonicalize()?);
    }
    find_root(Some(root))
}

pub(crate) fn canonicalize_or_join(path: &Path) -> Result<PathBuf, VaultliError> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

pub(crate) fn relative_path(path: &Path, root: &Path) -> Result<String, VaultliError> {
    let root = resolve_root(root)?;
    let path = canonicalize_or_join(path)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| VaultliError::PathOutsideRoot(path.display().to_string()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn iter_markdown_files(root: &Path) -> Result<Vec<PathBuf>, VaultliError> {
    let mut files = Vec::new();
    visit_markdown(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit_markdown(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), VaultliError> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            visit_markdown(&child, files)?;
            continue;
        }
        if child.file_name().and_then(|value| value.to_str()) == Some(INDEX_FILENAME) {
            continue;
        }
        if child.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(child);
        }
    }
    Ok(())
}
