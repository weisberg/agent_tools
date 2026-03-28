use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::Utc;
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

use crate::error::VaultliError;
use crate::frontmatter::parse_frontmatter_text;
use crate::model::{
    IndexBuildResult, ParsedDocument, ValidationIssue, ValidationResult, WarningRecord,
};

pub const VAULT_MARKER: &str = ".kbroot";
pub const INDEX_FILENAME: &str = "INDEX.jsonl";

const FRONTMATTER_FIELD_ORDER: &[&str] = &[
    "id",
    "title",
    "description",
    "tags",
    "category",
    "aliases",
    "author",
    "status",
    "created",
    "updated",
    "source",
    "depends_on",
    "related",
    "tokens",
    "priority",
    "scope",
    "domain",
    "version",
];
const REQUIRED_FIELDS: &[&str] = &["id", "title", "description"];
const LIST_FIELDS: &[&str] = &["tags", "aliases", "depends_on", "related"];
const STRING_FIELDS: &[&str] = &[
    "id",
    "title",
    "description",
    "category",
    "author",
    "status",
    "source",
    "scope",
    "domain",
];
const INTEGER_FIELDS: &[&str] = &["tokens", "priority"];

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

pub fn make_id(file: &Path, root: &Path) -> Result<String, VaultliError> {
    let root = root.canonicalize()?;
    let file = canonicalize_or_join(file)?;
    let relative = file
        .strip_prefix(&root)
        .map_err(|_| VaultliError::PathOutsideRoot(file.display().to_string()))?;
    let mut rendered = relative.to_string_lossy().replace('\\', "/");
    if rendered.ends_with(".md") {
        rendered.truncate(rendered.len() - 3);
    }
    if rendered
        .rsplit('/')
        .next()
        .map(|name| name.contains('.'))
        .unwrap_or(false)
    {
        if let Some(index) = rendered.rfind('.') {
            rendered.truncate(index);
        }
    }
    Ok(rendered.replace('_', "-").replace(' ', "-").to_lowercase())
}

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
    let relative_path = relative_path(&path, root)?;
    Ok(ParsedDocument {
        relative_path,
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
) -> Result<Vec<Map<String, Value>>, VaultliError> {
    let mut records = load_index_records(root)?;
    if let Some(query) = query {
        let needle = query.to_lowercase();
        records = records
            .into_iter()
            .filter(|record| {
                serde_json::to_string(record)
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&needle)
            })
            .collect();
    }

    if let Some(filter) = jq_filter {
        let jq_path = which("jq").ok_or(VaultliError::JqUnavailable)?;
        let payload = records
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
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
    Ok(records)
}

pub fn build_index(root: &Path, full: bool) -> Result<IndexBuildResult, VaultliError> {
    let root = resolve_root(root)?;
    let existing = load_index_records(&root).unwrap_or_default();
    let existing_by_id = existing
        .into_iter()
        .filter_map(|record| match record.get("id").and_then(Value::as_str) {
            Some(id) => Some((id.to_string(), record)),
            None => None,
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

    let mut next_records = Vec::new();
    let mut emitted_ids = BTreeSet::new();

    for path in iter_markdown_files(&root)? {
        match parse_markdown_file(&path, &root) {
            Ok(document) => match build_index_record(&root, &path, &document) {
                Ok(record) => {
                    let doc_id = record
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if doc_id.is_empty() {
                        result.warnings.push(WarningRecord {
                            code: "MISSING_REQUIRED_FIELDS".into(),
                            message: format!(
                                "Missing required fields in {}",
                                document.relative_path
                            ),
                            file: Some(document.relative_path.clone()),
                        });
                        continue;
                    }
                    emitted_ids.insert(doc_id.clone());
                    if full {
                        result.indexed += 1;
                    } else if let Some(previous) = existing_by_id.get(&doc_id) {
                        if previous == &record {
                            result.skipped += 1;
                        } else {
                            result.updated += 1;
                        }
                    } else {
                        result.indexed += 1;
                    }
                    next_records.push(record);
                }
                Err(error) => result.warnings.push(WarningRecord {
                    code: error.code().into(),
                    message: error.to_string(),
                    file: Some(
                        relative_path(&path, &root).unwrap_or_else(|_| path.display().to_string()),
                    ),
                }),
            },
            Err(error) => result.warnings.push(WarningRecord {
                code: error.code().into(),
                message: error.to_string(),
                file: Some(
                    relative_path(&path, &root).unwrap_or_else(|_| path.display().to_string()),
                ),
            }),
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

fn build_index_record(
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

fn write_index_records(root: &Path, records: &[Map<String, Value>]) -> Result<(), VaultliError> {
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

fn render_document(metadata: &Map<String, Value>, body: &str) -> String {
    let frontmatter = render_metadata_yaml(&order_metadata(metadata));
    let mut rendered_body = body.to_string();
    if !rendered_body.is_empty() && !rendered_body.starts_with('\n') {
        rendered_body = format!("\n{rendered_body}");
    }
    format!("---\n{frontmatter}\n---{rendered_body}")
}

fn render_metadata_yaml(metadata: &Map<String, Value>) -> String {
    let mut lines = Vec::new();
    for (key, value) in metadata {
        match value {
            Value::Array(items) => {
                lines.push(format!("{key}:"));
                for item in items {
                    lines.push(format!("  - {}", yaml_scalar(item)));
                }
            }
            Value::String(text) if text.contains('\n') => {
                lines.push(format!("{key}: >-"));
                for line in text.lines() {
                    lines.push(format!("  {line}"));
                }
            }
            _ => lines.push(format!("{key}: {}", yaml_scalar(value))),
        }
    }
    lines.join("\n")
}

fn iter_markdown_files(root: &Path) -> Result<Vec<PathBuf>, VaultliError> {
    let mut files = Vec::new();
    visit_markdown(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn yaml_scalar(value: &Value) -> String {
    match value {
        Value::String(text) => {
            if text.is_empty()
                || text.contains(':')
                || text.starts_with('[')
                || text.starts_with('{')
                || text.starts_with('#')
            {
                format!("{text:?}")
            } else {
                text.clone()
            }
        }
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".into(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
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

fn resolve_root(root: &Path) -> Result<PathBuf, VaultliError> {
    if root.join(VAULT_MARKER).exists() {
        return Ok(root.canonicalize()?);
    }
    find_root(Some(root))
}

fn canonicalize_or_join(path: &Path) -> Result<PathBuf, VaultliError> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn relative_path(path: &Path, root: &Path) -> Result<String, VaultliError> {
    let root = resolve_root(root)?;
    let path = canonicalize_or_join(path)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| VaultliError::PathOutsideRoot(path.display().to_string()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
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

fn default_sidecar_body(source_path: &Path) -> String {
    format!(
        "\n## Purpose\n\nDescribe the purpose and usage of `{}`.\n",
        source_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default()
    )
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

fn index_blocking_issues(path: &Path, document: &ParsedDocument) -> Vec<ValidationIssue> {
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

fn which(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for entry in std::env::split_paths(&path_var) {
        let candidate = entry.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn order_metadata(metadata: &Map<String, Value>) -> Map<String, Value> {
    let mut ordered = Map::new();
    for field in FRONTMATTER_FIELD_ORDER {
        if let Some(value) = metadata.get(*field) {
            ordered.insert((*field).to_string(), value.clone());
        }
    }
    for (key, value) in metadata {
        if !ordered.contains_key(key) {
            ordered.insert(key.clone(), value.clone());
        }
    }
    ordered
}

fn map_from_pairs(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), value);
    }
    map
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Value;

    use super::{
        add_file, build_index, find_root, infer_frontmatter, init_vault, make_id,
        parse_markdown_file, scaffold_file, validate_vault, INDEX_FILENAME, VAULT_MARKER,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("vaultli-rs-{name}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn finds_root_upwards() {
        let root = temp_dir("root");
        fs::write(root.join(VAULT_MARKER), "").unwrap();
        let nested = root.join("docs/notes");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(
            find_root(Some(&nested)).unwrap(),
            root.canonicalize().unwrap()
        );
    }

    #[test]
    fn derives_ids_for_sidecars() {
        let root = temp_dir("id");
        let file = root.join("queries/report.sql.md");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "").unwrap();
        assert_eq!(make_id(&file, &root).unwrap(), "queries/report");
    }

    #[test]
    fn parses_markdown_and_indexes() {
        let root = temp_dir("index");
        init_vault(&root).unwrap();
        let doc = root.join("docs/guide.md");
        fs::create_dir_all(doc.parent().unwrap()).unwrap();
        fs::write(
            &doc,
            "---\nid: docs/guide\ntitle: Guide\ndescription: Helpful guide\n---\nBody\n",
        )
        .unwrap();
        let parsed = parse_markdown_file(&doc, &root).unwrap();
        assert!(parsed.has_frontmatter);
        assert_eq!(parsed.doc_id(), Some("docs/guide"));
        let result = build_index(&root, true).unwrap();
        assert_eq!(result.indexed, 1);
        let index_text = fs::read_to_string(root.join(INDEX_FILENAME)).unwrap();
        assert!(index_text.contains("\"id\":\"docs/guide\""));
    }

    #[test]
    fn infers_frontmatter_for_templates() {
        let root = temp_dir("infer");
        init_vault(&root).unwrap();
        let template = root.join("templates/report.j2");
        fs::create_dir_all(template.parent().unwrap()).unwrap();
        fs::write(&template, "hello {{ name }}").unwrap();
        let inferred = infer_frontmatter(&template, &root).unwrap();
        assert_eq!(
            inferred.get("category"),
            Some(&Value::String("template".into()))
        );
        assert_eq!(
            inferred.get("source"),
            Some(&Value::String("./report.j2".into()))
        );
    }

    #[test]
    fn scaffolds_sidecar_and_adds_markdown() {
        let root = temp_dir("scaffold");
        init_vault(&root).unwrap();
        let sql = root.join("queries/report.sql");
        fs::create_dir_all(sql.parent().unwrap()).unwrap();
        fs::write(&sql, "select 1;").unwrap();
        let scaffolded = scaffold_file(&root, &sql).unwrap();
        assert_eq!(
            scaffolded.get("mode"),
            Some(&Value::String("sidecar".into()))
        );
        assert!(root.join("queries/report.sql.md").exists());

        let md = root.join("docs/notes.md");
        fs::create_dir_all(md.parent().unwrap()).unwrap();
        fs::write(&md, "# Notes\n").unwrap();
        let added = add_file(&root, &md).unwrap();
        assert_eq!(
            added.get("mode"),
            Some(&Value::String("frontmatter".into()))
        );
        let contents = fs::read_to_string(&md).unwrap();
        assert!(contents.starts_with("---\n"));
    }

    #[test]
    fn validates_broken_sources_and_duplicates() {
        let root = temp_dir("validate");
        init_vault(&root).unwrap();
        let doc1 = root.join("docs/one.md");
        fs::create_dir_all(doc1.parent().unwrap()).unwrap();
        fs::write(
            &doc1,
            "---\nid: docs/dup\ntitle: One\ndescription: First\ndepends_on:\n  - docs/missing\n---\nBody\n",
        )
        .unwrap();
        let doc2 = root.join("docs/two.md");
        fs::write(
            &doc2,
            "---\nid: docs/dup\ntitle: Two\ndescription: Second\n---\nBody\n",
        )
        .unwrap();
        let broken = root.join("queries/broken.sql.md");
        fs::create_dir_all(broken.parent().unwrap()).unwrap();
        fs::write(
            &broken,
            "---\nid: queries/broken\ntitle: Broken\ndescription: Broken\nsource: ./broken.sql\n---\nBody\n",
        )
        .unwrap();
        build_index(&root, true).unwrap();
        let validation = validate_vault(&root).unwrap();
        let codes = validation
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("BROKEN_SOURCE"));
        assert!(codes.contains("ORPHANED_SIDECAR"));
        assert!(codes.contains("DUPLICATE_ID"));
        assert!(codes.contains("DANGLING_DEPENDENCY"));
    }
}
