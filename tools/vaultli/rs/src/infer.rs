use std::fs;
use std::path::Path;

use serde_json::{Map, Number, Value};

use chrono::Utc;

use crate::error::VaultliError;
use crate::id::make_id;
use crate::paths::canonicalize_or_join;
use crate::util::order_metadata;

pub fn infer_frontmatter(file: &Path, root: &Path) -> Result<Map<String, Value>, VaultliError> {
    let file = canonicalize_or_join(file)?;
    if !file.exists() {
        return Err(VaultliError::FileNotFound(file.display().to_string()));
    }
    let today = Utc::now().date_naive().to_string();
    let category = infer_category(&file);
    let title = infer_title(&file);
    let description = infer_description(&file, &category, &title);
    let tags = infer_tags(&file, &category);
    let domain = infer_domain(&file);
    let sample_text = fs::read_to_string(&file).unwrap_or_default();
    let tokens = estimate_tokens(&sample_text);

    let mut metadata = Map::new();
    metadata.insert("id".into(), Value::String(make_id(&file, root)?));
    metadata.insert("title".into(), Value::String(title));
    metadata.insert("description".into(), Value::String(description));
    metadata.insert(
        "tags".into(),
        Value::Array(tags.into_iter().map(Value::String).collect()),
    );
    metadata.insert("category".into(), Value::String(category));
    metadata.insert("status".into(), Value::String("draft".into()));
    metadata.insert("created".into(), Value::String(today.clone()));
    metadata.insert("updated".into(), Value::String(today));
    metadata.insert("tokens".into(), Value::Number(Number::from(tokens)));
    metadata.insert("priority".into(), Value::Number(Number::from(3)));
    metadata.insert("scope".into(), Value::String("personal".into()));
    if let Some(domain) = domain {
        metadata.insert("domain".into(), Value::String(domain));
    }
    if file.extension().and_then(|value| value.to_str()) != Some("md") {
        metadata.insert(
            "source".into(),
            Value::String(format!("./{}", file.file_name().unwrap().to_string_lossy())),
        );
    }
    Ok(order_metadata(&metadata))
}

fn infer_category(path: &Path) -> String {
    let suffix = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let parts = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect::<Vec<_>>();

    if suffix == "md" {
        if name == "skill.md" || parts.iter().any(|part| part == "skills") {
            return "skill".into();
        }
        if parts.iter().any(|part| part == "runbooks") {
            return "runbook".into();
        }
        return "note".into();
    }
    if suffix == "sql" {
        return "query".into();
    }
    if suffix == "j2" || suffix == "jinja" || suffix == "jinja2" {
        return "template".into();
    }
    "reference".into()
}

fn infer_title(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let stem =
        if path.extension().and_then(|value| value.to_str()) == Some("md") && stem.contains('.') {
            Path::new(stem)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(stem)
                .to_string()
        } else {
            stem.to_string()
        };
    stem.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|token| {
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_description(path: &Path, category: &str, title: &str) -> String {
    match category {
        "query" => format!(
            "SQL query asset for {} stored in the vault.",
            title.to_lowercase()
        ),
        "template" => format!(
            "Template asset for {} stored in the vault.",
            title.to_lowercase()
        ),
        "skill" => format!(
            "Skill definition for {} stored in the vault.",
            title.to_lowercase()
        ),
        "runbook" => format!(
            "Runbook documenting {} for the vault.",
            title.to_lowercase()
        ),
        _ => {
            let _ = path;
            format!(
                "Markdown document for {} stored in the vault.",
                title.to_lowercase()
            )
        }
    }
}

fn infer_tags(path: &Path, category: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for component in path.components() {
        for token in component
            .as_os_str()
            .to_string_lossy()
            .replace(['.', '-', '_'], " ")
            .split_whitespace()
        {
            let token = token.to_lowercase();
            if !token.is_empty() && !tags.contains(&token) {
                tags.push(token);
            }
        }
    }
    if !tags.contains(&category.to_string()) {
        tags.push(category.to_string());
    }
    tags.truncate(8);
    tags
}

fn infer_domain(path: &Path) -> Option<String> {
    for component in path.components() {
        let value = component
            .as_os_str()
            .to_string_lossy()
            .replace('_', "-")
            .to_lowercase();
        if matches!(
            value.as_str(),
            "experimentation"
                | "marketing-analytics"
                | "infrastructure"
                | "tooling"
                | "finance"
                | "management"
        ) {
            return Some(value);
        }
    }
    None
}

fn estimate_tokens(text: &str) -> i64 {
    let words = text.split_whitespace().count() as i64;
    if words == 0 {
        0
    } else {
        ((words as f64) * 1.3).round() as i64
    }
}
