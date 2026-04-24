use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::error::VaultliError;
use crate::frontmatter::render_frontmatter_yaml;
use crate::index::{build_index, parse_markdown_file};
use crate::infer::infer_frontmatter;
use crate::paths::{canonicalize_or_join, relative_path, resolve_root};
use crate::util::{map_from_pairs, order_metadata, INDEX_FILENAME, VAULT_MARKER};

pub fn init_vault(target: &Path) -> Result<Map<String, Value>, VaultliError> {
    let target = canonicalize_or_join(target)?;
    for candidate in target.ancestors() {
        if candidate.join(VAULT_MARKER).exists() {
            return Err(VaultliError::RootExists(candidate.display().to_string()));
        }
    }

    fs::create_dir_all(&target)?;
    fs::File::create(target.join(VAULT_MARKER))?;
    fs::write(target.join(INDEX_FILENAME), "")?;

    Ok(map_from_pairs(vec![
        ("root", Value::String(target.display().to_string())),
        (
            "marker",
            Value::String(target.join(VAULT_MARKER).display().to_string()),
        ),
        (
            "index",
            Value::String(target.join(INDEX_FILENAME).display().to_string()),
        ),
    ]))
}

pub fn scaffold_file(root: &Path, file: &Path) -> Result<Map<String, Value>, VaultliError> {
    let target = canonicalize_or_join(file)?;
    if !target.exists() {
        return Err(VaultliError::FileNotFound(target.display().to_string()));
    }
    if target.is_dir() {
        return Err(VaultliError::NotAFile(target.display().to_string()));
    }
    let root = resolve_root(root)?;
    let metadata = infer_frontmatter(&target, &root)?;

    let (mode, written_path) = if target.extension().and_then(|value| value.to_str()) == Some("md")
    {
        let parsed = parse_markdown_file(&target, &root)?;
        if parsed.has_frontmatter {
            return Err(VaultliError::FrontmatterExists(
                target.display().to_string(),
            ));
        }
        fs::write(&target, render_document(&metadata, &parsed.body))?;
        ("frontmatter".to_string(), target.clone())
    } else {
        let sidecar = target.with_file_name(format!(
            "{}.md",
            target.file_name().unwrap().to_string_lossy()
        ));
        if sidecar.exists() {
            return Err(VaultliError::SidecarExists(sidecar.display().to_string()));
        }
        fs::write(
            &sidecar,
            render_document(&metadata, &default_sidecar_body(&target)),
        )?;
        ("sidecar".to_string(), sidecar)
    };

    Ok(map_from_pairs(vec![
        ("root", Value::String(root.display().to_string())),
        ("mode", Value::String(mode)),
        ("file", Value::String(relative_path(&written_path, &root)?)),
        ("id", metadata.get("id").cloned().unwrap_or(Value::Null)),
        ("metadata", Value::Object(order_metadata(&metadata))),
    ]))
}

pub fn add_file(root: &Path, file: &Path) -> Result<Map<String, Value>, VaultliError> {
    let scaffolded = scaffold_file(root, file)?;
    let resolved_root = scaffolded
        .get("root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| VaultliError::Unsupported("missing scaffold root".into()))?;
    let index = build_index(&resolved_root, false)?;
    Ok(map_from_pairs(vec![
        (
            "root",
            scaffolded.get("root").cloned().unwrap_or(Value::Null),
        ),
        (
            "file",
            scaffolded.get("file").cloned().unwrap_or(Value::Null),
        ),
        ("id", scaffolded.get("id").cloned().unwrap_or(Value::Null)),
        (
            "mode",
            scaffolded.get("mode").cloned().unwrap_or(Value::Null),
        ),
        (
            "index",
            serde_json::to_value(index).map_err(VaultliError::from)?,
        ),
    ]))
}

pub fn ingest_path(
    root: &Path,
    path: &Path,
    index: bool,
    dry_run: bool,
) -> Result<Map<String, Value>, VaultliError> {
    let target = canonicalize_or_join(path)?;
    if !target.exists() {
        return Err(VaultliError::FileNotFound(target.display().to_string()));
    }
    let root = resolve_root(root)?;
    let candidates = ingest_candidates(&target, &root)?;
    let mut scaffolded = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();

    for candidate in &candidates {
        if is_sidecar_markdown(candidate) {
            skipped.push(issue_entry(
                &root,
                candidate,
                "SIDECAR_MARKDOWN",
                "Sidecar markdown is not scaffolded directly",
            )?);
            continue;
        }

        match plan_scaffold(candidate, &root) {
            Ok(planned) => {
                if dry_run {
                    scaffolded.push(Value::Object(planned));
                } else {
                    scaffolded.push(Value::Object(scaffold_file(&root, candidate)?));
                }
            }
            Err(error)
                if matches!(
                    error,
                    VaultliError::FrontmatterExists(_) | VaultliError::SidecarExists(_)
                ) =>
            {
                skipped.push(issue_entry(
                    &root,
                    candidate,
                    error.code(),
                    &error.to_string(),
                )?);
            }
            Err(error) => {
                errors.push(issue_entry(
                    &root,
                    candidate,
                    error.code(),
                    &error.to_string(),
                )?);
            }
        }
    }

    let mut result = map_from_pairs(vec![
        ("root", Value::String(root.display().to_string())),
        (
            "path",
            Value::String(display_relative_path(&target, &root)?),
        ),
        ("dry_run", Value::Bool(dry_run)),
        ("indexed", Value::Bool(false)),
        ("total", json!(candidates.len())),
        ("scaffolded", Value::Array(scaffolded)),
        ("skipped", Value::Array(skipped)),
        ("errors", Value::Array(errors)),
    ]);

    if index && !dry_run {
        let index_result = build_index(&root, false)?;
        result.insert("index".to_string(), serde_json::to_value(index_result)?);
        result.insert("indexed".to_string(), Value::Bool(true));
    }

    Ok(result)
}

pub(crate) fn render_document(metadata: &Map<String, Value>, body: &str) -> String {
    let ordered = order_metadata(metadata);
    let frontmatter = render_frontmatter_yaml(&ordered).unwrap_or_default();
    let mut rendered_body = body.to_string();
    if !rendered_body.is_empty() && !rendered_body.starts_with('\n') {
        rendered_body = format!("\n{rendered_body}");
    }
    format!("---\n{frontmatter}\n---{rendered_body}")
}

fn ingest_candidates(target: &Path, root: &Path) -> Result<Vec<PathBuf>, VaultliError> {
    if target.is_file() {
        return Ok(vec![target.to_path_buf()]);
    }
    if !target.is_dir() {
        return Err(VaultliError::NotAFile(target.display().to_string()));
    }

    let mut files = Vec::new();
    visit_ingest_files(target, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit_ingest_files(
    path: &Path,
    root: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), VaultliError> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            if child
                .file_name()
                .and_then(|value| value.to_str())
                .map(|name| name.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }
            visit_ingest_files(&child, root, files)?;
            continue;
        }
        if should_skip_ingest_file(&child, root)? {
            continue;
        }
        files.push(child);
    }
    Ok(())
}

fn should_skip_ingest_file(path: &Path, root: &Path) -> Result<bool, VaultliError> {
    let relative = path
        .canonicalize()?
        .strip_prefix(root)
        .map_err(|_| VaultliError::PathOutsideRoot(path.display().to_string()))?
        .to_path_buf();
    if path.file_name().and_then(|value| value.to_str()) == Some(VAULT_MARKER)
        || path.file_name().and_then(|value| value.to_str()) == Some(INDEX_FILENAME)
        || path.file_name().and_then(|value| value.to_str()) == Some("INDEX.jsonl.tmp")
    {
        return Ok(true);
    }
    Ok(relative
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
        || is_sidecar_markdown(path))
}

fn plan_scaffold(file: &Path, root: &Path) -> Result<Map<String, Value>, VaultliError> {
    let metadata = infer_frontmatter(file, root)?;
    let (mode, written_path) = if file.extension().and_then(|value| value.to_str()) == Some("md") {
        let parsed = parse_markdown_file(file, root)?;
        if parsed.has_frontmatter {
            return Err(VaultliError::FrontmatterExists(file.display().to_string()));
        }
        ("frontmatter".to_string(), file.to_path_buf())
    } else {
        let sidecar = file.with_file_name(format!(
            "{}.md",
            file.file_name().unwrap().to_string_lossy()
        ));
        if sidecar.exists() {
            return Err(VaultliError::SidecarExists(sidecar.display().to_string()));
        }
        ("sidecar".to_string(), sidecar)
    };

    Ok(map_from_pairs(vec![
        ("root", Value::String(root.display().to_string())),
        ("mode", Value::String(mode)),
        ("file", Value::String(relative_path(&written_path, root)?)),
        ("id", metadata.get("id").cloned().unwrap_or(Value::Null)),
        ("metadata", Value::Object(order_metadata(&metadata))),
    ]))
}

fn issue_entry(root: &Path, file: &Path, code: &str, message: &str) -> Result<Value, VaultliError> {
    Ok(json!({
        "file": relative_path(file, root)?,
        "code": code,
        "message": message,
    }))
}

fn is_sidecar_markdown(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("md")
        && path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|stem| stem.contains('.'))
            .unwrap_or(false)
}

fn display_relative_path(path: &Path, root: &Path) -> Result<String, VaultliError> {
    let relative = relative_path(path, root)?;
    if relative.is_empty() {
        return Ok(".".to_string());
    }
    Ok(relative)
}

fn default_sidecar_body(source_path: &Path) -> String {
    format!(
        "\n## Purpose\n\nDescribe the purpose and usage of `{}`.\n",
        source_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default()
    )
}
