use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::*;

pub(crate) fn resolve_section(
    index: &DocumentIndex,
    selector: &str,
) -> Result<SectionInfo, MdliError> {
    let by_id = index
        .sections
        .iter()
        .filter(|s| s.id.as_deref() == Some(selector))
        .cloned()
        .collect::<Vec<_>>();
    if !by_id.is_empty() {
        return one_match(
            by_id,
            "E_SELECTOR_NOT_FOUND",
            format!("no section matched selector {selector}"),
            section_match_details,
        );
    }
    let wanted = split_path(selector);
    let matches = index
        .sections
        .iter()
        .filter(|s| {
            let current = split_path(&s.path);
            current == wanted || current.ends_with(&wanted)
        })
        .cloned()
        .collect::<Vec<_>>();
    one_match(
        matches,
        "E_SELECTOR_NOT_FOUND",
        format!("no section matched selector {selector}"),
        section_match_details,
    )
}

pub(crate) fn resolve_table_by_name(
    index: &DocumentIndex,
    name: &str,
) -> Result<TableInfo, MdliError> {
    let matches = index
        .tables
        .iter()
        .filter(|t| t.name.as_deref() == Some(name))
        .cloned()
        .collect::<Vec<_>>();
    one_match(
        matches,
        "E_SELECTOR_NOT_FOUND",
        format!("no table named {name}"),
        table_match_details,
    )
}

pub(crate) fn resolve_block(index: &DocumentIndex, id: &str) -> Result<BlockInfo, MdliError> {
    let matches = index
        .blocks
        .iter()
        .filter(|b| b.id == id)
        .cloned()
        .collect::<Vec<_>>();
    one_match(
        matches,
        "E_SELECTOR_NOT_FOUND",
        format!("no block with id {id}"),
        block_match_details,
    )
}

pub(crate) fn one_match<T, F>(
    matches: Vec<T>,
    missing_code: &'static str,
    missing: impl Into<String>,
    describe: F,
) -> Result<T, MdliError>
where
    F: Fn(&T) -> Value,
{
    match matches.len() {
        0 => Err(MdliError::user(missing_code, missing.into())),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let descriptions = matches.iter().map(&describe).collect::<Vec<_>>();
            Err(MdliError::user(
                "E_AMBIGUOUS_SELECTOR",
                "selector matched more than one structure",
            )
            .with_details(json!({"matches": descriptions})))
        }
    }
}

fn section_match_details(s: &SectionInfo) -> Value {
    json!({
        "id": s.id,
        "path": s.path,
        "line": s.line,
        "level": s.level,
    })
}

fn table_match_details(t: &TableInfo) -> Value {
    json!({
        "name": t.name,
        "section_id": t.section_id,
        "section_path": t.section_path,
        "line": t.line,
    })
}

fn block_match_details(b: &BlockInfo) -> Value {
    json!({
        "id": b.id,
        "line": b.line,
        "locked": b.locked,
    })
}

pub(crate) fn selector_from_id_path(
    id: Option<&str>,
    path: Option<&str>,
) -> Result<String, MdliError> {
    id.or(path)
        .map(ToString::to_string)
        .ok_or_else(|| MdliError::user("E_SELECTOR_REQUIRED", "--id or --path is required"))
}

pub(crate) fn split_path(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in path.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '>' {
            parts.push(normalize_heading(current.trim()));
            current.clear();
        } else {
            current.push(c);
        }
    }
    if !current.trim().is_empty() {
        parts.push(normalize_heading(current.trim()));
    }
    parts
}

pub(crate) fn validate_id(id: &str) -> Result<(), MdliError> {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return Err(MdliError::user("E_INVALID_ID", "stable ID cannot be empty"));
    };
    if !first.is_ascii_lowercase() {
        return Err(MdliError::user(
            "E_INVALID_ID",
            "stable ID must start with a lowercase ASCII letter",
        ));
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    {
        return Err(MdliError::user(
            "E_INVALID_ID",
            "stable ID must match [a-z][a-z0-9._-]*",
        ));
    }
    Ok(())
}

pub(crate) fn unique_slug(title: &str, existing: &mut BTreeSet<String>) -> String {
    let mut stripped = title.trim().to_string();
    let mut chars = stripped.chars().peekable();
    while chars
        .peek()
        .map(|c| c.is_ascii_digit() || *c == '.' || c.is_whitespace())
        .unwrap_or(false)
    {
        chars.next();
    }
    stripped = chars.collect::<String>();
    let mut slug = String::new();
    let mut last_dash = false;
    for c in stripped.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        slug = format!("section-{}", short_hash(title.as_bytes()));
    }
    let base = slug.clone();
    let mut suffix = 2;
    while existing.contains(&slug) {
        slug = format!("{base}-{suffix}");
        suffix += 1;
    }
    existing.insert(slug.clone());
    slug
}
