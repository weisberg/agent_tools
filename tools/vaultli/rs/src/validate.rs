use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::VaultliError;
use crate::index::{build_index_record, load_index_records, parse_markdown_file};
use crate::model::{ParsedDocument, ValidationIssue, ValidationResult};
use crate::paths::{iter_markdown_files, relative_path, resolve_root};
use crate::util::{INDEX_FILENAME, INTEGER_FIELDS, LIST_FIELDS, REQUIRED_FIELDS, STRING_FIELDS};

pub fn validate_vault(root: &Path) -> Result<ValidationResult, VaultliError> {
    let root = resolve_root(root)?;
    let mut issues: Vec<ValidationIssue> = Vec::new();
    let mut documents = Vec::new();

    for path in iter_markdown_files(&root)? {
        match parse_markdown_file(&path, &root) {
            Ok(document) => {
                if document.is_sidecar() {
                    let sibling_source = path.with_extension("");
                    if !sibling_source.exists() {
                        issues.push(issue(
                            "ORPHANED_SIDECAR",
                            format!(
                                "Sidecar has no sibling source asset: {}",
                                sibling_source
                                    .file_name()
                                    .map(|value| value.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            ),
                            Some(document.relative_path.clone()),
                            document.doc_id().map(str::to_string),
                        ));
                    }
                }
                issues.extend(document_validation_issues(&path, &document));
                documents.push((path, document));
            }
            Err(error) => issues.push(issue(
                error.code(),
                error.to_string(),
                Some(relative_path(&path, &root).unwrap_or_else(|_| path.display().to_string())),
                None,
            )),
        }
    }

    let mut ids_to_docs: BTreeMap<String, Vec<&ParsedDocument>> = BTreeMap::new();
    for (_, document) in &documents {
        if let Some(doc_id) = document.doc_id() {
            ids_to_docs
                .entry(doc_id.to_string())
                .or_default()
                .push(document);
        }
    }
    for (doc_id, entries) in ids_to_docs {
        if entries.len() > 1 {
            for document in entries {
                issues.push(issue(
                    "DUPLICATE_ID",
                    format!("Duplicate id {doc_id:?} declared by multiple files"),
                    Some(document.relative_path.clone()),
                    Some(doc_id.clone()),
                ));
            }
        }
    }

    let referenceable_ids = documents
        .iter()
        .filter(|(path, document)| {
            document.doc_id().is_some() && index_blocking_issues(path, document).is_empty()
        })
        .filter_map(|(_, document)| document.doc_id().map(str::to_string))
        .collect::<BTreeSet<_>>();

    for (_, document) in &documents {
        if let Some(values) = document
            .metadata
            .get("depends_on")
            .and_then(Value::as_array)
        {
            for value in values {
                if let Some(target) = value.as_str() {
                    if !referenceable_ids.contains(target) {
                        issues.push(issue(
                            "DANGLING_DEPENDENCY",
                            format!("depends_on reference {target:?} does not resolve"),
                            Some(document.relative_path.clone()),
                            document.doc_id().map(str::to_string),
                        ));
                    }
                }
            }
        }
        if let Some(values) = document.metadata.get("related").and_then(Value::as_array) {
            for value in values {
                if let Some(target) = value.as_str() {
                    if !referenceable_ids.contains(target) {
                        issues.push(issue(
                            "DANGLING_RELATED",
                            format!("related reference {target:?} does not resolve"),
                            Some(document.relative_path.clone()),
                            document.doc_id().map(str::to_string),
                        ));
                    }
                }
            }
        }
    }

    issues.extend(index_staleness_issues(&root, &documents)?);
    issues.sort();
    issues.dedup();

    Ok(ValidationResult {
        root: root.display().to_string(),
        valid: issues.is_empty(),
        issue_count: issues.len(),
        issues,
    })
}

fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
    file: Option<String>,
    doc_id: Option<String>,
) -> ValidationIssue {
    ValidationIssue {
        code: code.into(),
        message: message.into(),
        file,
        doc_id,
        level: "error".into(),
    }
}

pub(crate) fn index_blocking_issues(
    path: &Path,
    document: &ParsedDocument,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let missing = REQUIRED_FIELDS
        .iter()
        .filter(|field| !document.metadata.contains_key(**field))
        .map(|field| (*field).to_string())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        issues.push(issue(
            "MISSING_REQUIRED_FIELDS",
            format!("Missing required fields: {}", missing.join(", ")),
            Some(document.relative_path.clone()),
            document.doc_id().map(str::to_string),
        ));
    }

    let source = document.metadata.get("source").and_then(Value::as_str);
    if document.is_sidecar() && source.is_none() {
        issues.push(issue(
            "MISSING_SOURCE_FIELD",
            "Sidecar markdown is missing required source field",
            Some(document.relative_path.clone()),
            document.doc_id().map(str::to_string),
        ));
    }
    if let Some(source) = source {
        let source_path = path.parent().unwrap_or_else(|| Path::new(".")).join(source);
        if !source_path.exists() {
            issues.push(issue(
                "BROKEN_SOURCE",
                format!("source target does not exist: {source}"),
                Some(document.relative_path.clone()),
                document.doc_id().map(str::to_string),
            ));
        }
    }
    issues
}

fn document_validation_issues(path: &Path, document: &ParsedDocument) -> Vec<ValidationIssue> {
    let mut issues = index_blocking_issues(path, document);
    for field in LIST_FIELDS {
        if let Some(value) = document.metadata.get(*field) {
            if !value.is_array() {
                issues.push(issue(
                    "INVALID_FIELD_TYPE",
                    format!("Field {field:?} must be a list"),
                    Some(document.relative_path.clone()),
                    document.doc_id().map(str::to_string),
                ));
            }
        }
    }
    for field in STRING_FIELDS {
        if let Some(value) = document.metadata.get(*field) {
            if !value.is_string() {
                issues.push(issue(
                    "INVALID_FIELD_TYPE",
                    format!("Field {field:?} must be a string"),
                    Some(document.relative_path.clone()),
                    document.doc_id().map(str::to_string),
                ));
            }
        }
    }
    for field in INTEGER_FIELDS {
        if let Some(value) = document.metadata.get(*field) {
            if !value.is_i64() && !value.is_u64() {
                issues.push(issue(
                    "INVALID_FIELD_TYPE",
                    format!("Field {field:?} must be an integer"),
                    Some(document.relative_path.clone()),
                    document.doc_id().map(str::to_string),
                ));
            }
        }
    }
    if let Some(priority) = document.metadata.get("priority").and_then(Value::as_i64) {
        if !(1..=5).contains(&priority) {
            issues.push(issue(
                "INVALID_PRIORITY",
                "priority must be between 1 and 5",
                Some(document.relative_path.clone()),
                document.doc_id().map(str::to_string),
            ));
        }
    }
    for field in ["created", "updated"] {
        if let Some(value) = document.metadata.get(field).and_then(Value::as_str) {
            if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
                issues.push(issue(
                    "INVALID_DATE",
                    format!("Field {field:?} must be an ISO date string (YYYY-MM-DD)"),
                    Some(document.relative_path.clone()),
                    document.doc_id().map(str::to_string),
                ));
            }
        } else if document.metadata.contains_key(field) {
            issues.push(issue(
                "INVALID_DATE",
                format!("Field {field:?} must be an ISO date string (YYYY-MM-DD)"),
                Some(document.relative_path.clone()),
                document.doc_id().map(str::to_string),
            ));
        }
    }
    issues
}

#[allow(clippy::manual_map)] // clippy's suggestion doesn't compile (borrow + move)
fn index_staleness_issues(
    root: &Path,
    documents: &[(PathBuf, ParsedDocument)],
) -> Result<Vec<ValidationIssue>, VaultliError> {
    let index_path = root.join(INDEX_FILENAME);
    if !index_path.exists() {
        return Ok(vec![issue(
            "MISSING_INDEX",
            "INDEX.jsonl is missing",
            Some(INDEX_FILENAME.into()),
            None,
        )]);
    }
    let indexed_records = load_index_records(root)?;
    let indexed_by_id = indexed_records
        .into_iter()
        .filter_map(|record| match record.get("id").and_then(Value::as_str) {
            Some(id) => Some((id.to_string(), record)),
            None => None,
        })
        .collect::<BTreeMap<_, _>>();

    let mut valid_docs = Vec::new();
    let mut seen_ids = BTreeSet::new();
    for (path, document) in documents {
        let Some(doc_id) = document.doc_id() else {
            continue;
        };
        if !index_blocking_issues(path, document).is_empty() {
            continue;
        }
        if seen_ids.contains(doc_id) {
            continue;
        }
        seen_ids.insert(doc_id.to_string());
        valid_docs.push((path, document));
    }

    let valid_ids = valid_docs
        .iter()
        .filter_map(|(_, document)| document.doc_id().map(str::to_string))
        .collect::<BTreeSet<_>>();

    let mut issues = Vec::new();
    for (path, document) in valid_docs {
        let doc_id = document.doc_id().unwrap();
        let current = build_index_record(root, path, document)?;
        let existing = indexed_by_id.get(doc_id);
        if existing != Some(&current) {
            issues.push(issue(
                "STALE_INDEX",
                format!("Index entry is stale for {doc_id}"),
                Some(document.relative_path.clone()),
                Some(doc_id.to_string()),
            ));
        }
    }

    for (stale_id, stale_record) in indexed_by_id {
        if !valid_ids.contains(&stale_id) {
            issues.push(issue(
                "STALE_INDEX",
                format!("Index contains removed or invalid record for {stale_id}"),
                stale_record
                    .get("file")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                Some(stale_id),
            ));
        }
    }
    Ok(issues)
}
