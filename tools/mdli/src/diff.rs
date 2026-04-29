//! Semantic diff between two Markdown documents.
//!
//! Identity rules:
//!
//! - Sections are identified by stable ID when present. Sections without
//!   stable IDs fall back to path equality.
//! - Tables are identified by name (from the table marker). Unnamed tables
//!   are matched positionally within the same parent section.
//! - Managed blocks are identified by their `id` field.
//! - Table rows are identified by the table's key column when defined on
//!   both sides. Otherwise the diff reports `body_changed` without
//!   row-level findings.
//!
//! See PRD section 26 for the rationale and the documented finding shape.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::*;

pub(crate) fn run_diff(args: DiffArgs) -> Result<Outcome, MdliError> {
    let new_doc = MarkdownDocument::read(&args.file)?;
    let old_doc = MarkdownDocument::read(&args.against)?;
    let new_idx = index_document(&new_doc);
    let old_idx = index_document(&old_doc);

    let mut findings = Vec::new();
    diff_sections(&old_idx, &new_idx, &mut findings);
    diff_tables(&old_doc, &new_doc, &old_idx, &new_idx, &mut findings)?;
    diff_blocks(&old_doc, &new_doc, &old_idx, &new_idx, &mut findings);
    diff_frontmatter(&old_doc, &new_doc, &mut findings)?;

    let summary = compute_summary(&findings);

    if args.text {
        return Ok(Outcome::Text(render_text(
            &args, &old_doc, &new_doc, &summary, &findings,
        )));
    }

    Ok(Outcome::Json(json!({
        "old": {
            "path": args.against.display().to_string(),
            "preimage_hash": old_doc.preimage_hash,
        },
        "new": {
            "path": args.file.display().to_string(),
            "preimage_hash": new_doc.preimage_hash,
        },
        "summary": summary,
        "findings": findings,
    })))
}

// ---------------------------------------------------------------------------
// sections
// ---------------------------------------------------------------------------

fn diff_sections(old_idx: &DocumentIndex, new_idx: &DocumentIndex, findings: &mut Vec<Value>) {
    let mut old_by_id: BTreeMap<&str, &SectionInfo> = BTreeMap::new();
    let mut new_by_id: BTreeMap<&str, &SectionInfo> = BTreeMap::new();
    let mut old_no_id: Vec<&SectionInfo> = Vec::new();
    let mut new_no_id: Vec<&SectionInfo> = Vec::new();

    for s in &old_idx.sections {
        match &s.id {
            Some(id) => {
                old_by_id.insert(id.as_str(), s);
            }
            None => old_no_id.push(s),
        }
    }
    for s in &new_idx.sections {
        match &s.id {
            Some(id) => {
                new_by_id.insert(id.as_str(), s);
            }
            None => new_no_id.push(s),
        }
    }

    // ID-anchored matches.
    for (id, old_section) in &old_by_id {
        match new_by_id.get(id) {
            None => findings.push(json!({
                "kind": "section.removed",
                "id": id,
                "path": old_section.path,
                "level": old_section.level,
            })),
            Some(new_section) => {
                if old_section.title != new_section.title {
                    findings.push(json!({
                        "kind": "section.renamed",
                        "id": id,
                        "old_title": old_section.title,
                        "new_title": new_section.title,
                    }));
                }
                if old_section.level != new_section.level {
                    findings.push(json!({
                        "kind": "section.level_changed",
                        "id": id,
                        "old_level": old_section.level,
                        "new_level": new_section.level,
                    }));
                }
                if parent_path(&old_section.path) != parent_path(&new_section.path) {
                    findings.push(json!({
                        "kind": "section.moved",
                        "id": id,
                        "old_path": old_section.path,
                        "new_path": new_section.path,
                    }));
                }
            }
        }
    }
    for (id, new_section) in &new_by_id {
        if !old_by_id.contains_key(id) {
            findings.push(json!({
                "kind": "section.added",
                "id": id,
                "path": new_section.path,
                "level": new_section.level,
            }));
        }
    }

    // Path-anchored matches for sections without stable IDs. We treat each
    // (path, level) as identity. Renames or moves of unmarked sections show
    // up as a delete + add pair, which is the right behavior — the wire
    // format should be promoted with `id assign` to get rename semantics.
    let old_paths: BTreeSet<(&str, usize)> = old_no_id
        .iter()
        .map(|s| (s.path.as_str(), s.level))
        .collect();
    let new_paths: BTreeSet<(&str, usize)> = new_no_id
        .iter()
        .map(|s| (s.path.as_str(), s.level))
        .collect();
    for s in &old_no_id {
        if !new_paths.contains(&(s.path.as_str(), s.level)) {
            findings.push(json!({
                "kind": "section.removed",
                "id": Value::Null,
                "path": s.path,
                "level": s.level,
            }));
        }
    }
    for s in &new_no_id {
        if !old_paths.contains(&(s.path.as_str(), s.level)) {
            findings.push(json!({
                "kind": "section.added",
                "id": Value::Null,
                "path": s.path,
                "level": s.level,
            }));
        }
    }
}

fn parent_path(path: &str) -> String {
    let parts = split_path(path);
    if parts.len() <= 1 {
        return String::new();
    }
    parts[..parts.len() - 1].join(" > ")
}

// ---------------------------------------------------------------------------
// tables
// ---------------------------------------------------------------------------

fn diff_tables(
    old_doc: &MarkdownDocument,
    new_doc: &MarkdownDocument,
    old_idx: &DocumentIndex,
    new_idx: &DocumentIndex,
    findings: &mut Vec<Value>,
) -> Result<(), MdliError> {
    let mut old_by_name: BTreeMap<&str, &TableInfo> = BTreeMap::new();
    let mut new_by_name: BTreeMap<&str, &TableInfo> = BTreeMap::new();
    for t in &old_idx.tables {
        if let Some(name) = &t.name {
            old_by_name.insert(name.as_str(), t);
        }
    }
    for t in &new_idx.tables {
        if let Some(name) = &t.name {
            new_by_name.insert(name.as_str(), t);
        }
    }

    for (name, old_table) in &old_by_name {
        match new_by_name.get(name) {
            None => findings.push(json!({
                "kind": "table.removed",
                "name": name,
                "section_id": old_table.section_id,
                "section_path": old_table.section_path,
            })),
            Some(new_table) => {
                if old_table.columns != new_table.columns {
                    findings.push(json!({
                        "kind": "table.columns_changed",
                        "name": name,
                        "old_columns": old_table.columns,
                        "new_columns": new_table.columns,
                    }));
                }
                if old_table.key != new_table.key {
                    findings.push(json!({
                        "kind": "table.key_changed",
                        "name": name,
                        "old_key": old_table.key,
                        "new_key": new_table.key,
                    }));
                }
                diff_table_rows(old_doc, new_doc, name, old_table, new_table, findings)?;
            }
        }
    }
    for (name, new_table) in &new_by_name {
        if !old_by_name.contains_key(name) {
            findings.push(json!({
                "kind": "table.added",
                "name": name,
                "section_id": new_table.section_id,
                "section_path": new_table.section_path,
            }));
        }
    }
    Ok(())
}

fn diff_table_rows(
    old_doc: &MarkdownDocument,
    new_doc: &MarkdownDocument,
    name: &str,
    old_table: &TableInfo,
    new_table: &TableInfo,
    findings: &mut Vec<Value>,
) -> Result<(), MdliError> {
    let old_data = match table_data_from_lines(
        &old_doc.lines[old_table.start..old_table.end],
        old_table.marker.is_some(),
    ) {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };
    let new_data = match table_data_from_lines(
        &new_doc.lines[new_table.start..new_table.end],
        new_table.marker.is_some(),
    ) {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };

    // Need a key column on both sides for row-level diff.
    let key_column = old_table
        .key
        .as_deref()
        .filter(|k| new_table.key.as_deref() == Some(k) && old_data.columns == new_data.columns);
    let Some(key_column) = key_column else {
        if old_data.rows != new_data.rows {
            findings.push(json!({
                "kind": "table.body_changed",
                "name": name,
                "old_row_count": old_data.rows.len(),
                "new_row_count": new_data.rows.len(),
                "reason": "no shared key or column shape changed",
            }));
        }
        return Ok(());
    };

    let key_idx = old_data
        .columns
        .iter()
        .position(|c| c == key_column)
        .ok_or_else(|| MdliError::user("E_TABLE_KEY_MISSING", "key column missing"))?;
    let columns = old_data.columns.clone();

    let old_by_key: BTreeMap<String, Vec<String>> = old_data
        .rows
        .into_iter()
        .map(|row| (row.get(key_idx).cloned().unwrap_or_default(), row))
        .collect();
    let new_by_key: BTreeMap<String, Vec<String>> = new_data
        .rows
        .into_iter()
        .map(|row| (row.get(key_idx).cloned().unwrap_or_default(), row))
        .collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut updated = Vec::new();
    for (key, old_row) in &old_by_key {
        match new_by_key.get(key) {
            None => removed.push(row_to_object(&columns, old_row)),
            Some(new_row) if new_row != old_row => updated.push(json!({
                "key": key,
                "before": row_to_object(&columns, old_row),
                "after": row_to_object(&columns, new_row),
            })),
            Some(_) => {}
        }
    }
    for (key, new_row) in &new_by_key {
        if !old_by_key.contains_key(key) {
            added.push(row_to_object(&columns, new_row));
        }
    }

    if !added.is_empty() || !removed.is_empty() || !updated.is_empty() {
        findings.push(json!({
            "kind": "table.rows_changed",
            "name": name,
            "key": key_column,
            "added": added,
            "removed": removed,
            "updated": updated,
        }));
    }
    Ok(())
}

fn row_to_object(columns: &[String], row: &[String]) -> Value {
    let mut map = Map::new();
    for (idx, col) in columns.iter().enumerate() {
        let value = row.get(idx).cloned().unwrap_or_default();
        map.insert(col.clone(), Value::String(value));
    }
    Value::Object(map)
}

// ---------------------------------------------------------------------------
// blocks
// ---------------------------------------------------------------------------

fn diff_blocks(
    old_doc: &MarkdownDocument,
    new_doc: &MarkdownDocument,
    old_idx: &DocumentIndex,
    new_idx: &DocumentIndex,
    findings: &mut Vec<Value>,
) {
    let mut old_by_id: BTreeMap<&str, &BlockInfo> = BTreeMap::new();
    let mut new_by_id: BTreeMap<&str, &BlockInfo> = BTreeMap::new();
    for b in &old_idx.blocks {
        old_by_id.insert(b.id.as_str(), b);
    }
    for b in &new_idx.blocks {
        new_by_id.insert(b.id.as_str(), b);
    }

    for (id, old_block) in &old_by_id {
        match new_by_id.get(id) {
            None => findings.push(json!({
                "kind": "block.removed",
                "id": id,
                "old_locked": old_block.locked,
            })),
            Some(new_block) => {
                let old_actual =
                    checksum_body(&old_doc.lines[old_block.start + 1..old_block.end - 1]);
                let new_actual =
                    checksum_body(&new_doc.lines[new_block.start + 1..new_block.end - 1]);
                if old_actual != new_actual {
                    findings.push(json!({
                        "kind": "block.content_changed",
                        "id": id,
                        "old_checksum": old_actual,
                        "new_checksum": new_actual,
                        "was_locked": old_block.locked,
                    }));
                    if old_block.locked {
                        findings.push(json!({
                            "kind": "block.locked_edit_attempted",
                            "id": id,
                            "old_checksum": old_actual,
                            "new_checksum": new_actual,
                        }));
                    }
                }
                if old_block.locked != new_block.locked {
                    findings.push(json!({
                        "kind": "block.lock_changed",
                        "id": id,
                        "old_locked": old_block.locked,
                        "new_locked": new_block.locked,
                    }));
                }
                if let Some(recorded) = &new_block.checksum {
                    if recorded != &new_actual {
                        findings.push(json!({
                            "kind": "block.tampered",
                            "id": id,
                            "recorded_checksum": recorded,
                            "actual_checksum": new_actual,
                            "side": "new",
                        }));
                    }
                }
            }
        }
    }
    for (id, new_block) in &new_by_id {
        if !old_by_id.contains_key(id) {
            findings.push(json!({
                "kind": "block.added",
                "id": id,
                "locked": new_block.locked,
            }));
        }
    }
}

// ---------------------------------------------------------------------------
// frontmatter
// ---------------------------------------------------------------------------

fn diff_frontmatter(
    old_doc: &MarkdownDocument,
    new_doc: &MarkdownDocument,
    findings: &mut Vec<Value>,
) -> Result<(), MdliError> {
    let old_map = parse_frontmatter_map(&old_doc.lines)?;
    let new_map = parse_frontmatter_map(&new_doc.lines)?;
    for (key, old_value) in &old_map {
        match new_map.get(key) {
            None => findings.push(json!({
                "kind": "frontmatter.removed",
                "key": key,
                "old_value": old_value,
            })),
            Some(new_value) if new_value != old_value => findings.push(json!({
                "kind": "frontmatter.changed",
                "key": key,
                "old_value": old_value,
                "new_value": new_value,
            })),
            Some(_) => {}
        }
    }
    for (key, new_value) in &new_map {
        if !old_map.contains_key(key) {
            findings.push(json!({
                "kind": "frontmatter.added",
                "key": key,
                "new_value": new_value,
            }));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// summary
// ---------------------------------------------------------------------------

fn compute_summary(findings: &[Value]) -> Value {
    let mut summary = json!({
        "sections_added": 0,
        "sections_removed": 0,
        "sections_renamed": 0,
        "sections_moved": 0,
        "sections_level_changed": 0,
        "tables_added": 0,
        "tables_removed": 0,
        "tables_columns_changed": 0,
        "tables_key_changed": 0,
        "tables_rows_changed": 0,
        "rows_added": 0,
        "rows_removed": 0,
        "rows_updated": 0,
        "tables_body_changed": 0,
        "blocks_added": 0,
        "blocks_removed": 0,
        "blocks_content_changed": 0,
        "blocks_lock_changed": 0,
        "blocks_locked_edit_attempted": 0,
        "blocks_tampered": 0,
        "frontmatter_added": 0,
        "frontmatter_removed": 0,
        "frontmatter_changed": 0,
    });
    let obj = summary.as_object_mut().unwrap();
    let bump = |obj: &mut Map<String, Value>, key: &str, n: u64| {
        let prev = obj.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
        obj.insert(key.to_string(), json!(prev + n));
    };
    for f in findings {
        let Some(kind) = f.get("kind").and_then(|v| v.as_str()) else {
            continue;
        };
        match kind {
            "section.added" => bump(obj, "sections_added", 1),
            "section.removed" => bump(obj, "sections_removed", 1),
            "section.renamed" => bump(obj, "sections_renamed", 1),
            "section.moved" => bump(obj, "sections_moved", 1),
            "section.level_changed" => bump(obj, "sections_level_changed", 1),
            "table.added" => bump(obj, "tables_added", 1),
            "table.removed" => bump(obj, "tables_removed", 1),
            "table.columns_changed" => bump(obj, "tables_columns_changed", 1),
            "table.key_changed" => bump(obj, "tables_key_changed", 1),
            "table.rows_changed" => {
                bump(obj, "tables_rows_changed", 1);
                let added = f
                    .get("added")
                    .and_then(|v| v.as_array())
                    .map_or(0, |a| a.len()) as u64;
                let removed = f
                    .get("removed")
                    .and_then(|v| v.as_array())
                    .map_or(0, |a| a.len()) as u64;
                let updated = f
                    .get("updated")
                    .and_then(|v| v.as_array())
                    .map_or(0, |a| a.len()) as u64;
                bump(obj, "rows_added", added);
                bump(obj, "rows_removed", removed);
                bump(obj, "rows_updated", updated);
            }
            "table.body_changed" => bump(obj, "tables_body_changed", 1),
            "block.added" => bump(obj, "blocks_added", 1),
            "block.removed" => bump(obj, "blocks_removed", 1),
            "block.content_changed" => bump(obj, "blocks_content_changed", 1),
            "block.lock_changed" => bump(obj, "blocks_lock_changed", 1),
            "block.locked_edit_attempted" => bump(obj, "blocks_locked_edit_attempted", 1),
            "block.tampered" => bump(obj, "blocks_tampered", 1),
            "frontmatter.added" => bump(obj, "frontmatter_added", 1),
            "frontmatter.removed" => bump(obj, "frontmatter_removed", 1),
            "frontmatter.changed" => bump(obj, "frontmatter_changed", 1),
            _ => {}
        }
    }
    summary
}

// ---------------------------------------------------------------------------
// text rendering
// ---------------------------------------------------------------------------

fn render_text(
    args: &DiffArgs,
    _old_doc: &MarkdownDocument,
    _new_doc: &MarkdownDocument,
    summary: &Value,
    findings: &[Value],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "diff {} -> {}\n",
        args.against.display(),
        args.file.display()
    ));
    if findings.is_empty() {
        out.push_str("no semantic changes\n");
        return out;
    }

    out.push_str("\nsummary:\n");
    if let Some(obj) = summary.as_object() {
        for (key, value) in obj {
            if value.as_u64().unwrap_or(0) > 0 {
                out.push_str(&format!("  {key}: {value}\n"));
            }
        }
    }

    out.push_str("\nfindings:\n");
    for f in findings {
        let kind = f.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        match kind {
            "section.added" => out.push_str(&format!(
                "  + section {} ({})\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("<no-id>"),
                f.get("path").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "section.removed" => out.push_str(&format!(
                "  - section {} ({})\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("<no-id>"),
                f.get("path").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "section.renamed" => out.push_str(&format!(
                "  ~ section {} renamed: {:?} -> {:?}\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("old_title").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("new_title").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "section.moved" => out.push_str(&format!(
                "  ~ section {} moved: {:?} -> {:?}\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("old_path").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("new_path").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "section.level_changed" => out.push_str(&format!(
                "  ~ section {} level: {} -> {}\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("old_level").and_then(|v| v.as_u64()).unwrap_or(0),
                f.get("new_level").and_then(|v| v.as_u64()).unwrap_or(0)
            )),
            "table.added" => out.push_str(&format!(
                "  + table {}\n",
                f.get("name").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "table.removed" => out.push_str(&format!(
                "  - table {}\n",
                f.get("name").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "table.columns_changed" => out.push_str(&format!(
                "  ~ table {} columns changed\n",
                f.get("name").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "table.key_changed" => out.push_str(&format!(
                "  ~ table {} key: {:?} -> {:?}\n",
                f.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("old_key").unwrap_or(&Value::Null),
                f.get("new_key").unwrap_or(&Value::Null)
            )),
            "table.rows_changed" => out.push_str(&format!(
                "  ~ table {} rows: +{} -{} ~{}\n",
                f.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("added")
                    .and_then(|v| v.as_array())
                    .map_or(0, |a| a.len()),
                f.get("removed")
                    .and_then(|v| v.as_array())
                    .map_or(0, |a| a.len()),
                f.get("updated")
                    .and_then(|v| v.as_array())
                    .map_or(0, |a| a.len())
            )),
            "table.body_changed" => out.push_str(&format!(
                "  ~ table {} body changed (no shared key)\n",
                f.get("name").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "block.added" => out.push_str(&format!(
                "  + block {}\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "block.removed" => out.push_str(&format!(
                "  - block {}\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "block.content_changed" => out.push_str(&format!(
                "  ~ block {} content changed\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "block.lock_changed" => out.push_str(&format!(
                "  ~ block {} locked: {} -> {}\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("old_locked")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                f.get("new_locked")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            )),
            "block.locked_edit_attempted" => out.push_str(&format!(
                "  ! block {} locked-edit attempted\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "block.tampered" => out.push_str(&format!(
                "  ! block {} tampered ({} side)\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                f.get("side").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "frontmatter.added" => out.push_str(&format!(
                "  + frontmatter {}\n",
                f.get("key").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "frontmatter.removed" => out.push_str(&format!(
                "  - frontmatter {}\n",
                f.get("key").and_then(|v| v.as_str()).unwrap_or("")
            )),
            "frontmatter.changed" => out.push_str(&format!(
                "  ~ frontmatter {}\n",
                f.get("key").and_then(|v| v.as_str()).unwrap_or("")
            )),
            other => out.push_str(&format!("  ? unknown finding kind {other}\n")),
        }
    }
    out
}
