use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

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

pub(crate) fn render_document(metadata: &Map<String, Value>, body: &str) -> String {
    let ordered = order_metadata(metadata);
    let frontmatter = render_frontmatter_yaml(&ordered).unwrap_or_default();
    let mut rendered_body = body.to_string();
    if !rendered_body.is_empty() && !rendered_body.starts_with('\n') {
        rendered_body = format!("\n{rendered_body}");
    }
    format!("---\n{frontmatter}\n---{rendered_body}")
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
