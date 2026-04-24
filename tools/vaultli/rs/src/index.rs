use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::VaultliError;
use crate::frontmatter::parse_frontmatter_text;
use crate::model::{IndexBuildResult, ParsedDocument, WarningRecord};
use crate::paths::{canonicalize_or_join, iter_markdown_files, relative_path, resolve_root};
use crate::util::{order_metadata, INDEX_FILENAME, REQUIRED_FIELDS};

pub fn parse_markdown_file(path: &Path, root: &Path) -> Result<ParsedDocument, VaultliError> {
    let path = canonicalize_or_join(path)?;
    if !path.exists() {
        return Err(VaultliError::FileNotFound(path.display().to_string()));
    }
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
        return Err(VaultliError::NotMarkdown(path.display().to_string()));
    }
    let text = fs::read_to_string(&path)?;
    let (metadata, body, has_frontmatter) =
        parse_frontmatter_text(&text, &path.display().to_string())?;
    let rel = relative_path(&path, root)?;
    Ok(ParsedDocument {
        relative_path: rel,
        metadata: order_metadata(&metadata),
        body,
        has_frontmatter,
    })
}

pub fn load_index_records(root: &Path) -> Result<Vec<Map<String, Value>>, VaultliError> {
    let root = resolve_root(root)?;
    let index_path = root.join(INDEX_FILENAME);
    if !index_path.exists() {
        return Err(VaultliError::IndexMissing(index_path.display().to_string()));
    }
    let text = fs::read_to_string(index_path)?;
    let mut records = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        match value {
            Value::Object(map) => records.push(map),
            _ => return Err(VaultliError::InvalidIndex),
        }
    }
    Ok(records)
}

pub fn build_index(root: &Path, full: bool) -> Result<IndexBuildResult, VaultliError> {
    let root = resolve_root(root)?;
    let existing = load_index_records(&root).unwrap_or_default();
    let existing_by_id = existing
        .into_iter()
        .filter_map(|record| {
            record
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), record.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    let mut result = IndexBuildResult {
        root: root.display().to_string(),
        full,
        indexed: 0,
        updated: 0,
        pruned: 0,
        skipped: 0,
        warnings: Vec::new(),
    };

    let mut next_records: Vec<Map<String, Value>> = Vec::new();
    let mut emitted_ids: BTreeSet<String> = BTreeSet::new();

    for path in iter_markdown_files(&root)? {
        let rel = relative_path(&path, &root).unwrap_or_else(|_| path.display().to_string());
        let document = match parse_markdown_file(&path, &root) {
            Ok(doc) => doc,
            Err(error) => {
                result.warnings.push(WarningRecord {
                    code: error.code().into(),
                    message: error.to_string(),
                    file: Some(rel.clone()),
                });
                continue;
            }
        };

        let blocking = collect_blocking_issues(&path, &document);
        if !blocking.is_empty() {
            result.warnings.extend(blocking);
            continue;
        }

        let doc_id = document
            .doc_id()
            .expect("blocking check should have caught missing id")
            .to_string();

        if emitted_ids.contains(&doc_id) {
            result.warnings.push(WarningRecord {
                code: "DUPLICATE_ID".into(),
                message: format!("Duplicate id {:?} encountered during indexing", doc_id),
                file: Some(document.relative_path.clone()),
            });
            continue;
        }

        let record = match build_index_record(&root, &path, &document) {
            Ok(record) => record,
            Err(error) => {
                result.warnings.push(WarningRecord {
                    code: error.code().into(),
                    message: error.to_string(),
                    file: Some(document.relative_path.clone()),
                });
                continue;
            }
        };

        emitted_ids.insert(doc_id.clone());

        if full {
            result.indexed += 1;
            next_records.push(record);
            continue;
        }

        match existing_by_id.get(&doc_id) {
            None => {
                result.indexed += 1;
                next_records.push(record);
            }
            Some(previous) if previous == &record => {
                result.skipped += 1;
                next_records.push(previous.clone());
            }
            Some(_) => {
                result.updated += 1;
                next_records.push(record);
            }
        }
    }

    if !full {
        result.pruned = existing_by_id
            .keys()
            .filter(|id| !emitted_ids.contains(*id))
            .count();
    }

    write_index_records(&root, &next_records)?;
    Ok(result)
}

/// Build the list of blocking warnings that prevent a document from being
/// indexed (missing required fields, missing source on sidecars, broken source).
fn collect_blocking_issues(path: &Path, document: &ParsedDocument) -> Vec<WarningRecord> {
    let mut issues = Vec::new();
    let missing: Vec<&str> = REQUIRED_FIELDS
        .iter()
        .filter(|field| {
            document
                .metadata
                .get(**field)
                .and_then(Value::as_str)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        })
        .copied()
        .collect();
    if !missing.is_empty() {
        issues.push(WarningRecord {
            code: "MISSING_REQUIRED_FIELDS".into(),
            message: format!("Missing required fields: {}", missing.join(", ")),
            file: Some(document.relative_path.clone()),
        });
    }

    let source = document.metadata.get("source").and_then(Value::as_str);
    if document.is_sidecar() && source.map(|value| value.trim().is_empty()).unwrap_or(true) {
        issues.push(WarningRecord {
            code: "MISSING_SOURCE_FIELD".into(),
            message: "Sidecar markdown is missing required source field".into(),
            file: Some(document.relative_path.clone()),
        });
    }

    if let Some(source) = source {
        if !source.trim().is_empty() {
            let source_path = path.parent().unwrap_or_else(|| Path::new(".")).join(source);
            if !source_path.exists() {
                issues.push(WarningRecord {
                    code: "BROKEN_SOURCE".into(),
                    message: format!("source target does not exist: {source}"),
                    file: Some(document.relative_path.clone()),
                });
            }
        }
    }

    issues
}

pub(crate) fn build_index_record(
    root: &Path,
    path: &Path,
    document: &ParsedDocument,
) -> Result<Map<String, Value>, VaultliError> {
    for field in REQUIRED_FIELDS {
        if !document.metadata.contains_key(*field) {
            return Err(VaultliError::MissingRequiredFields(
                document.relative_path.clone(),
            ));
        }
    }
    if document.is_sidecar() && !document.metadata.contains_key("source") {
        return Err(VaultliError::MissingRequiredFields(
            document.relative_path.clone(),
        ));
    }

    let hash = compute_content_hash(path, document)?;
    let mut record = order_metadata(&document.metadata);
    record.insert("file".into(), Value::String(relative_path(path, root)?));
    record.insert("hash".into(), Value::String(hash));
    Ok(record)
}

fn compute_content_hash(path: &Path, document: &ParsedDocument) -> Result<String, VaultliError> {
    let bytes = if let Some(source) = document.metadata.get("source").and_then(Value::as_str) {
        let source_path = path.parent().unwrap_or_else(|| Path::new(".")).join(source);
        if !source_path.exists() {
            return Err(VaultliError::BrokenSource(
                document.relative_path.clone(),
                source.to_string(),
            ));
        }
        fs::read(source_path)?
    } else {
        document.body.as_bytes().to_vec()
    };
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    Ok(format!("{:x}", digest)[..12].to_string())
}

pub(crate) fn write_index_records(
    root: &Path,
    records: &[Map<String, Value>],
) -> Result<(), VaultliError> {
    let tmp_path = root.join(format!("{}.tmp", INDEX_FILENAME));
    let index_path = root.join(INDEX_FILENAME);
    let content = records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let rendered = if content.is_empty() {
        content
    } else {
        format!("{content}\n")
    };
    fs::write(&tmp_path, rendered)?;
    fs::rename(tmp_path, index_path)?;
    Ok(())
}
