use std::fs;
use std::path::Path;

use chrono::Utc;
use serde_json::{Map, Number, Value};

use crate::error::VaultliError;
use crate::id::make_id;
use crate::paths::canonicalize_or_join;
use crate::util::{order_metadata, COMMON_TAGS, DOMAIN_CANDIDATES};

pub fn infer_frontmatter(file: &Path, root: &Path) -> Result<Map<String, Value>, VaultliError> {
    let file = canonicalize_or_join(file)?;
    if !file.exists() {
        return Err(VaultliError::FileNotFound(file.display().to_string()));
    }
    let today = Utc::now().date_naive().to_string();
    let category = infer_category(&file);
    let title = infer_title(&file);
    let description = infer_description(&category, &title);
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

pub(crate) fn infer_category(path: &Path) -> String {
    let suffix = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect();

    if suffix == "md" {
        if name == "skill.md" || parts.iter().any(|part| part == "skills") {
            return "skill".into();
        }
        if parts.iter().any(|part| part == "runbooks") || stem.contains("runbook") {
            return "runbook".into();
        }
        if stem.contains("tutorial") {
            return "tutorial".into();
        }
        if stem.contains("guide")
            || stem.contains("reference")
            || stem.contains("readme")
            || stem.contains("spec")
        {
            return "reference".into();
        }
        return "note".into();
    }
    if suffix == "sql" {
        return "query".into();
    }
    if suffix == "j2" || suffix == "jinja" || suffix == "jinja2" {
        return "template".into();
    }
    if matches!(suffix.as_str(), "py" | "json" | "yaml" | "yml" | "toml") {
        return "reference".into();
    }
    "reference".into()
}

pub(crate) fn infer_title(path: &Path) -> String {
    let mut stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    if path.extension().and_then(|value| value.to_str()) == Some("md") && stem.contains('.') {
        if let Some(idx) = stem.rfind('.') {
            stem.truncate(idx);
        }
    }
    stem.replace(['-', '_'], " ")
        .split_whitespace()
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(token: &str) -> String {
    let mut chars = token.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn infer_description(category: &str, title: &str) -> String {
    let lower = title.to_lowercase();
    match category {
        "query" => format!("SQL query asset for {lower} stored in the vault."),
        "template" => format!("Template asset for {lower} stored in the vault."),
        "skill" => format!("Skill definition for {lower} stored in the vault."),
        "runbook" => format!("Runbook documenting {lower} for the vault."),
        _ => format!("Markdown document for {lower} stored in the vault."),
    }
}

pub(crate) fn infer_tags(path: &Path, category: &str) -> Vec<String> {
    let common: std::collections::BTreeSet<&str> = COMMON_TAGS.iter().copied().collect();
    let components: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect();

    let mut tokens: Vec<String> = Vec::new();
    // parts[:-1] — path components excluding the filename.
    if components.len() > 1 {
        for part in &components[..components.len() - 1] {
            tokens.extend(slug_tokens(part));
        }
    }
    // target.stem — filename stem.
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    tokens.extend(slug_tokens(stem));
    tokens.push(category.to_string());

    let mut deduped: Vec<String> = Vec::new();
    for token in tokens {
        if token.is_empty() || common.contains(token.as_str()) {
            continue;
        }
        if !deduped.contains(&token) {
            deduped.push(token);
        }
    }
    deduped.truncate(8);
    deduped
}

fn slug_tokens(raw: &str) -> Vec<String> {
    raw.replace(['.', '-', '_'], " ")
        .split_whitespace()
        .map(|token| token.to_lowercase())
        .collect()
}

pub(crate) fn infer_domain(path: &Path) -> Option<String> {
    let candidates: std::collections::BTreeSet<&str> = DOMAIN_CANDIDATES.iter().copied().collect();
    let mut parts_lower: Vec<String> = Vec::new();
    for component in path.components() {
        let normalized = component
            .as_os_str()
            .to_string_lossy()
            .replace('_', "-")
            .replace(' ', "-")
            .to_lowercase();
        if candidates.contains(normalized.as_str()) {
            return Some(normalized);
        }
        parts_lower.push(component.as_os_str().to_string_lossy().to_lowercase());
    }
    if parts_lower.iter().any(|part| part == "tools") {
        return Some("tooling".into());
    }
    None
}

pub(crate) fn estimate_tokens(text: &str) -> i64 {
    let words = text.split_whitespace().count() as i64;
    if words == 0 {
        return 0;
    }
    let raw = (words as f64) * 1.3;
    std::cmp::max(1, raw as i64) // truncation, matching Python int(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn category_rules_match_python() {
        assert_eq!(infer_category(&PathBuf::from("skills/foo.md")), "skill");
        assert_eq!(infer_category(&PathBuf::from("anywhere/skill.md")), "skill");
        assert_eq!(
            infer_category(&PathBuf::from("runbooks/rb.md")),
            "runbook"
        );
        assert_eq!(
            infer_category(&PathBuf::from("docs/db-runbook.md")),
            "runbook"
        );
        assert_eq!(
            infer_category(&PathBuf::from("docs/python-tutorial.md")),
            "tutorial"
        );
        assert_eq!(
            infer_category(&PathBuf::from("docs/user-guide.md")),
            "reference"
        );
        assert_eq!(
            infer_category(&PathBuf::from("docs/api-reference.md")),
            "reference"
        );
        assert_eq!(infer_category(&PathBuf::from("README.md")), "reference");
        assert_eq!(
            infer_category(&PathBuf::from("docs/arch-spec.md")),
            "reference"
        );
        assert_eq!(infer_category(&PathBuf::from("docs/notes.md")), "note");
        assert_eq!(infer_category(&PathBuf::from("q/report.sql")), "query");
        assert_eq!(
            infer_category(&PathBuf::from("tpl/report.j2")),
            "template"
        );
        assert_eq!(
            infer_category(&PathBuf::from("config/settings.yaml")),
            "reference"
        );
    }

    #[test]
    fn tags_filter_common_and_limit_to_eight() {
        let tags = infer_tags(
            &PathBuf::from("templates/marketing/report-weekly.j2"),
            "template",
        );
        // "templates" and "j2" are in COMMON_TAGS and dropped; category is "template"
        // (also COMMON_TAGS) and is dropped. The directory name "marketing" and filename
        // tokens remain.
        assert!(tags.contains(&"marketing".to_string()));
        assert!(tags.contains(&"report".to_string()));
        assert!(tags.contains(&"weekly".to_string()));
        assert!(!tags.contains(&"templates".to_string()));
        assert!(!tags.contains(&"j2".to_string()));
        assert!(tags.len() <= 8);
    }

    #[test]
    fn domain_detects_candidates_and_tools_fallback() {
        assert_eq!(
            infer_domain(&PathBuf::from("experimentation/foo.md")),
            Some("experimentation".into())
        );
        assert_eq!(
            infer_domain(&PathBuf::from("marketing_analytics/foo.md")),
            Some("marketing-analytics".into())
        );
        assert_eq!(
            infer_domain(&PathBuf::from("tools/foo/bar.py")),
            Some("tooling".into())
        );
        assert_eq!(infer_domain(&PathBuf::from("misc/foo.md")), None);
    }

    #[test]
    fn estimate_tokens_uses_truncation() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("one"), 1); // 1 * 1.3 = 1.3 -> 1
        assert_eq!(estimate_tokens("one two three"), 3); // 3 * 1.3 = 3.9 -> 3
        assert_eq!(estimate_tokens("one two three four"), 5); // 4 * 1.3 = 5.2 -> 5
    }

    #[test]
    fn title_strips_sidecar_middle_extension() {
        assert_eq!(infer_title(&PathBuf::from("queries/report.sql.md")), "Report");
        assert_eq!(
            infer_title(&PathBuf::from("docs/my-notes.md")),
            "My Notes"
        );
    }
}
