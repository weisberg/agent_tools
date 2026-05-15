use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Number, Value};

use crate::error::VaultliError;
use crate::index::{build_index, parse_markdown_file};
use crate::infer::infer_frontmatter;
use crate::paths::{canonicalize_or_join, resolve_root};
use crate::scaffold::render_document;
use crate::search::show_record;
use crate::util::{order_metadata, INTEGER_FIELDS, LIST_FIELDS, VAULT_MARKER};

pub fn set_metadata_field(
    root: &Path,
    target: &str,
    field: &str,
    raw_value: &str,
    index: bool,
) -> Result<Map<String, Value>, VaultliError> {
    let root = resolve_root(root)?;
    let path = resolve_metadata_target(&root, target)?;
    let parsed = parse_markdown_file(&path, &root)?;
    if !parsed.has_frontmatter {
        return Err(VaultliError::Unsupported(format!(
            "markdown file has no frontmatter: {}",
            parsed.relative_path
        )));
    }

    let mut metadata = parsed.metadata.clone();
    metadata.insert(field.to_string(), parse_metadata_value(field, raw_value)?);
    fs::write(&path, render_document(&metadata, &parsed.body))?;

    let mut out = Map::new();
    out.insert("root".into(), Value::String(root.display().to_string()));
    out.insert("file".into(), Value::String(parsed.relative_path));
    out.insert("field".into(), Value::String(field.to_string()));
    out.insert(
        "value".into(),
        metadata.get(field).cloned().unwrap_or(Value::Null),
    );
    if index {
        out.insert(
            "index".into(),
            serde_json::to_value(build_index(&root, false)?)?,
        );
    }
    Ok(out)
}

pub fn unset_metadata_field(
    root: &Path,
    target: &str,
    field: &str,
    index: bool,
) -> Result<Map<String, Value>, VaultliError> {
    let root = resolve_root(root)?;
    let path = resolve_metadata_target(&root, target)?;
    let parsed = parse_markdown_file(&path, &root)?;
    if !parsed.has_frontmatter {
        return Err(VaultliError::Unsupported(format!(
            "markdown file has no frontmatter: {}",
            parsed.relative_path
        )));
    }

    let mut metadata = parsed.metadata.clone();
    let removed = metadata.remove(field).unwrap_or(Value::Null);
    fs::write(&path, render_document(&metadata, &parsed.body))?;

    let mut out = Map::new();
    out.insert("root".into(), Value::String(root.display().to_string()));
    out.insert("file".into(), Value::String(parsed.relative_path));
    out.insert("field".into(), Value::String(field.to_string()));
    out.insert("removed".into(), removed);
    if index {
        out.insert(
            "index".into(),
            serde_json::to_value(build_index(&root, false)?)?,
        );
    }
    Ok(out)
}

pub fn refresh_metadata(
    root: &Path,
    target: &str,
    fields: &[String],
    index: bool,
) -> Result<Map<String, Value>, VaultliError> {
    let root = resolve_root(root)?;
    let path = resolve_metadata_target(&root, target)?;
    let parsed = parse_markdown_file(&path, &root)?;
    if !parsed.has_frontmatter {
        return Err(VaultliError::Unsupported(format!(
            "markdown file has no frontmatter: {}",
            parsed.relative_path
        )));
    }

    let inference_target = parsed
        .metadata
        .get("source")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|source| path.parent().unwrap_or_else(|| Path::new(".")).join(source))
        .unwrap_or_else(|| path.clone());
    let inferred = infer_frontmatter(&inference_target, &root)?;
    let refreshable: Vec<String> = if fields.is_empty() {
        [
            "title",
            "description",
            "tags",
            "category",
            "domain",
            "tokens",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect()
    } else {
        fields.to_vec()
    };

    let mut metadata = parsed.metadata.clone();
    let mut changed = Map::new();
    for field in refreshable {
        if matches!(field.as_str(), "id" | "source" | "created") {
            continue;
        }
        if let Some(value) = inferred.get(&field) {
            metadata.insert(field.clone(), value.clone());
            changed.insert(field, value.clone());
        }
    }
    let today = chrono::Utc::now().date_naive().to_string();
    metadata.insert("updated".into(), Value::String(today.clone()));
    changed.insert("updated".into(), Value::String(today));
    fs::write(&path, render_document(&metadata, &parsed.body))?;

    let mut out = Map::new();
    out.insert("root".into(), Value::String(root.display().to_string()));
    out.insert("file".into(), Value::String(parsed.relative_path));
    out.insert("fields".into(), Value::Object(changed));
    if index {
        out.insert(
            "index".into(),
            serde_json::to_value(build_index(&root, false)?)?,
        );
    }
    Ok(out)
}

pub fn load_vault_defaults(root: &Path) -> Result<Map<String, Value>, VaultliError> {
    let marker = root.join(VAULT_MARKER);
    if !marker.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(marker)?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    let value: Value = serde_yaml::from_str(&text)
        .map_err(|err| VaultliError::InvalidFrontmatter(".kbroot".into(), err.to_string()))?;
    let defaults = value.get("defaults").cloned().unwrap_or(value);
    match defaults {
        Value::Object(map) => Ok(order_metadata(&map)),
        _ => Ok(Map::new()),
    }
}

pub(crate) fn resolve_metadata_target(root: &Path, target: &str) -> Result<PathBuf, VaultliError> {
    if let Ok(path) = canonicalize_or_join(Path::new(target)) {
        if path.exists() {
            return Ok(path);
        }
    }
    let root_relative = root.join(target);
    if root_relative.exists() {
        return Ok(root_relative);
    }
    let record = show_record(root, target)?;
    let file = record
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| VaultliError::Unsupported(format!("record {target} missing file")))?;
    Ok(root.join(file))
}

fn parse_metadata_value(field: &str, raw: &str) -> Result<Value, VaultliError> {
    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.into()));
    if LIST_FIELDS.contains(&field) {
        if parsed.is_array() {
            return Ok(parsed);
        }
        return Ok(Value::Array(
            raw.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect(),
        ));
    }
    if INTEGER_FIELDS.contains(&field) {
        let value = parsed
            .as_i64()
            .or_else(|| raw.parse::<i64>().ok())
            .ok_or_else(|| {
                VaultliError::Unsupported(format!("field {field} must be an integer"))
            })?;
        return Ok(Value::Number(Number::from(value)));
    }
    Ok(parsed)
}
