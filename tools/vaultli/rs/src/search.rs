use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

use crate::error::VaultliError;
use crate::index::{load_index_records, parse_markdown_file};
use crate::paths::{relative_path, resolve_root};
use crate::util::{to_sorted_json_string, which};

pub fn show_record(root: &Path, doc_id: &str) -> Result<Map<String, Value>, VaultliError> {
    for record in load_index_records(root)? {
        if record.get("id").and_then(Value::as_str) == Some(doc_id) {
            return Ok(record);
        }
    }
    Err(VaultliError::IdNotFound(doc_id.to_string()))
}

pub fn search_records(
    root: &Path,
    query: Option<&str>,
    jq_filter: Option<&str>,
    category: Option<&str>,
    status: Option<&str>,
    domain: Option<&str>,
    scope: Option<&str>,
    tags: &[String],
    limit: Option<usize>,
    sort: Option<&str>,
    order: &str,
    explain: bool,
    semantic: bool,
) -> Result<Vec<Map<String, Value>>, VaultliError> {
    if !matches!(order, "asc" | "desc") {
        return Err(VaultliError::Unsupported(
            "order must be asc or desc".into(),
        ));
    }
    let mut records = load_index_records(root)?;
    if let Some(query) = query {
        if semantic {
            records.retain(|record| semantic_score(record, query) > 0);
        } else {
            let needle = query.to_lowercase();
            records.retain(|record| {
                to_sorted_json_string(&Value::Object(record.clone()))
                    .to_lowercase()
                    .contains(&needle)
            });
        }
    }

    for (field, expected) in [
        ("category", category),
        ("status", status),
        ("domain", domain),
        ("scope", scope),
    ] {
        if let Some(expected) = expected {
            records.retain(|record| record.get(field).and_then(Value::as_str) == Some(expected));
        }
    }

    if !tags.is_empty() {
        records.retain(|record| {
            let Some(record_tags) = record.get("tags").and_then(Value::as_array) else {
                return false;
            };
            tags.iter().all(|tag| {
                record_tags
                    .iter()
                    .any(|value| value.as_str() == Some(tag.as_str()))
            })
        });
    }

    if let Some(filter) = jq_filter {
        let jq_path = which("jq").ok_or(VaultliError::JqUnavailable)?;
        let payload = records
            .iter()
            .map(|record| to_sorted_json_string(&Value::Object(record.clone())))
            .collect::<Vec<_>>()
            .join("\n");
        let mut child = Command::new(jq_path)
            .arg("-c")
            .arg(filter)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(payload.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(VaultliError::JqFilterFailed(message));
        }
        let mut filtered = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)?;
            match value {
                Value::Object(map) => filtered.push(map),
                _ => return Err(VaultliError::JqFilterInvalid),
            }
        }
        records = filtered;
    }
    if explain {
        records = records
            .into_iter()
            .map(|record| {
                with_match_explanation(
                    record, query, category, status, domain, scope, tags, semantic,
                )
            })
            .collect();
    }
    if let Some(sort) = sort {
        sort_records(&mut records, sort, order)?;
    }
    if let Some(limit) = limit {
        records.truncate(limit);
    }
    Ok(records)
}

pub fn resolve_record(
    root: &Path,
    doc_id: &str,
    include_body: bool,
    include_source: bool,
) -> Result<Map<String, Value>, VaultliError> {
    let root = resolve_root(root)?;
    let record = show_record(&root, doc_id)?;
    let file = record
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| VaultliError::Unsupported(format!("record {doc_id:?} is missing file")))?;
    let markdown_path = root.join(file);
    if !markdown_path.exists() {
        return Err(VaultliError::FileNotFound(file.into()));
    }
    let document = parse_markdown_file(&markdown_path, &root)?;

    let mut resolved = Map::new();
    resolved.insert("record".into(), Value::Object(record.clone()));
    resolved.insert("file".into(), Value::String(file.into()));
    resolved.insert(
        "path".into(),
        Value::String(markdown_path.display().to_string()),
    );
    if include_body {
        resolved.insert("body".into(), Value::String(document.body));
    }

    if let Some(source) = record.get("source").and_then(Value::as_str) {
        if !source.trim().is_empty() {
            let source_path = markdown_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(source);
            resolved.insert("source".into(), Value::String(source.into()));
            resolved.insert(
                "source_path".into(),
                Value::String(source_path.display().to_string()),
            );
            if source_path.exists() {
                resolved.insert(
                    "source_file".into(),
                    Value::String(relative_path(&source_path, &root)?),
                );
            }
            if include_source {
                if !source_path.exists() {
                    return Err(VaultliError::BrokenSource(doc_id.into(), source.into()));
                }
                resolved.insert(
                    "source_content".into(),
                    Value::String(fs::read_to_string(source_path)?),
                );
            }
            return Ok(resolved);
        }
    }

    if include_source {
        resolved.insert(
            "source_content".into(),
            Value::String(fs::read_to_string(markdown_path)?),
        );
    }
    Ok(resolved)
}

pub fn cat_record(
    root: &Path,
    doc_id: &str,
    source: bool,
) -> Result<Map<String, Value>, VaultliError> {
    let resolved = resolve_record(root, doc_id, !source, source)?;
    let mut result = Map::new();
    result.insert("id".into(), Value::String(doc_id.into()));
    result.insert(
        "mode".into(),
        Value::String(if source { "source" } else { "body" }.into()),
    );
    result.insert(
        "file".into(),
        resolved
            .get(if source { "source_file" } else { "file" })
            .cloned()
            .or_else(|| resolved.get("file").cloned())
            .unwrap_or(Value::Null),
    );
    result.insert(
        "content".into(),
        resolved
            .get(if source { "source_content" } else { "body" })
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    Ok(result)
}

fn with_match_explanation(
    mut record: Map<String, Value>,
    query: Option<&str>,
    category: Option<&str>,
    status: Option<&str>,
    domain: Option<&str>,
    scope: Option<&str>,
    tags: &[String],
    semantic: bool,
) -> Map<String, Value> {
    let mut matched_fields = Vec::new();
    if let Some(query) = query {
        let needle = query.to_lowercase();
        for (field, value) in &record {
            if value.to_string().to_lowercase().contains(&needle) {
                matched_fields.push(Value::String(field.clone()));
            }
        }
    }
    let mut filters = Map::new();
    for (key, value) in [
        ("category", category),
        ("status", status),
        ("domain", domain),
        ("scope", scope),
    ] {
        if let Some(value) = value {
            filters.insert(key.into(), Value::String(value.into()));
        }
    }
    let mut explanation = Map::new();
    explanation.insert(
        "query".into(),
        query.map(Value::from).unwrap_or(Value::Null),
    );
    explanation.insert("score".into(), Value::from(match_score(&record, query)));
    explanation.insert("semantic".into(), Value::Bool(semantic));
    explanation.insert(
        "semantic_score".into(),
        Value::from(if semantic {
            query.map(|q| semantic_score(&record, q)).unwrap_or(0)
        } else {
            0
        }),
    );
    explanation.insert("fields".into(), Value::Array(matched_fields));
    explanation.insert("filters".into(), Value::Object(filters));
    explanation.insert(
        "tags".into(),
        Value::Array(tags.iter().cloned().map(Value::String).collect()),
    );
    record.insert("_match".into(), Value::Object(explanation));
    record
}

fn match_score(record: &Map<String, Value>, query: Option<&str>) -> i64 {
    let Some(query) = query else { return 0 };
    let needle = query.to_lowercase();
    let mut score = 0;
    for (field, weight) in [
        ("title", 8),
        ("description", 5),
        ("tags", 3),
        ("aliases", 3),
        ("id", 2),
    ] {
        if let Some(value) = record.get(field) {
            score += value.to_string().to_lowercase().matches(&needle).count() as i64 * weight;
        }
    }
    score
        + Value::Object(record.clone())
            .to_string()
            .to_lowercase()
            .matches(&needle)
            .count() as i64
}

fn semantic_score(record: &Map<String, Value>, query: &str) -> i64 {
    let query_tokens = slug_tokens(query);
    let haystack = [
        "id",
        "title",
        "description",
        "tags",
        "aliases",
        "category",
        "domain",
    ]
    .iter()
    .filter_map(|field| record.get(*field))
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join(" ");
    let record_tokens = slug_tokens(&haystack);
    query_tokens
        .iter()
        .filter(|token| record_tokens.contains(token))
        .count() as i64
}

fn slug_tokens(raw: &str) -> Vec<String> {
    raw.replace(
        ['.', '-', '_', '"', '\'', '[', ']', ',', ':', '{', '}'],
        " ",
    )
    .split_whitespace()
    .map(|token| token.to_lowercase())
    .collect()
}

fn sort_records(
    records: &mut [Map<String, Value>],
    sort: &str,
    order: &str,
) -> Result<(), VaultliError> {
    let allowed = [
        "score", "id", "title", "updated", "created", "priority", "tokens", "category", "status",
    ];
    if !allowed.contains(&sort) {
        return Err(VaultliError::Unsupported(format!(
            "unsupported sort field: {sort}"
        )));
    }
    let reverse = order == "desc";
    records.sort_by(|left, right| {
        let ordering = if sort == "score" {
            left.get("_match")
                .and_then(|value| value.get("score"))
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .cmp(
                    &right
                        .get("_match")
                        .and_then(|value| value.get("score"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0),
                )
        } else {
            left.get(sort)
                .map(Value::to_string)
                .cmp(&right.get(sort).map(Value::to_string))
        };
        if reverse {
            ordering.reverse()
        } else {
            ordering
        }
    });
    Ok(())
}
