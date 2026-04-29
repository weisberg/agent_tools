use std::collections::HashMap;

use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_lint(args: LintArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    let issues = lint_document(&doc);
    if args.fix.as_deref() == Some("safe") {
        doc.assert_preimage(&args.mutate.preimage_hash)?;
        validate_write_emit(&args.mutate)?;
        let before = doc.render();
        table_fmt_all_safe(&mut doc)?;
        let changed = before != doc.render();
        return Ok(Outcome::Mutated(MutationOutcome {
            document: doc,
            changed,
            ops: vec![json!({"op": "lint_fix_safe"})],
            warnings: issues,
            flags: args.mutate,
        }));
    }
    Ok(Outcome::Json(json!({
        "issues": issues,
        "ok": !issues.iter().any(|i| i.get("severity") == Some(&json!("error")))
    })))
}

pub(crate) fn run_inspect(args: FileArgs) -> Result<Outcome, MdliError> {
    let doc = MarkdownDocument::read(&args.file)?;
    let index = index_document(&doc);
    Ok(Outcome::Json(json!({
        "preimage_hash": doc.preimage_hash,
        "sections": index.sections,
        "tables": index.tables,
        "blocks": index.blocks,
        "issues": lint_document(&doc)
    })))
}

pub(crate) fn lint_document(doc: &MarkdownDocument) -> Vec<Value> {
    let index = index_document(doc);
    let mut issues = Vec::new();
    let mut paths: HashMap<String, Vec<&SectionInfo>> = HashMap::new();
    let mut ids: HashMap<String, Vec<&SectionInfo>> = HashMap::new();
    for section in &index.sections {
        paths.entry(section.path.clone()).or_default().push(section);
        if let Some(id) = &section.id {
            ids.entry(id.clone()).or_default().push(section);
        }
    }
    for (path, matches) in paths {
        if matches.len() > 1 {
            issues.push(json!({
                "rule": "no-duplicate-headings",
                "severity": "warn",
                "message": format!("duplicate heading path {path}"),
                "line": matches[0].line
            }));
        }
    }
    for (id, matches) in ids {
        if matches.len() > 1 {
            issues.push(json!({
                "rule": "unique-stable-ids",
                "severity": "error",
                "code": "E_DUPLICATE_ID",
                "message": format!("duplicate stable ID {id}"),
                "line": matches[0].line
            }));
        }
    }
    for marker in &index.markers {
        if marker
            .fields
            .get("v")
            .map(|v| v != MARKER_VERSION)
            .unwrap_or(true)
        {
            issues.push(json!({
                "rule": "wire-format",
                "severity": "error",
                "message": "mdli marker missing supported v=1 field",
                "line": marker.line + 1
            }));
        }
        if marker.kind == "id" && !index.sections.iter().any(|s| s.start == marker.line) {
            issues.push(json!({
                "rule": "stable-id-binding",
                "severity": "error",
                "code": "E_ORPHAN_MARKER",
                "message": "id marker does not bind to a heading",
                "line": marker.line + 1
            }));
        }
        if marker.kind == "table" && !index.tables.iter().any(|t| t.marker == Some(marker.line)) {
            issues.push(json!({
                "rule": "table-marker-binding",
                "severity": "error",
                "code": "E_ORPHAN_MARKER",
                "message": "table marker does not bind to a table",
                "line": marker.line + 1
            }));
        }
    }
    for table in &index.tables {
        let rows = &doc.lines[table.start..table.end];
        let has_marker = table.marker.is_some();
        if let Err(err) = table_data_from_lines(rows, has_marker) {
            issues.push(json!({
                "rule": "valid-tables",
                "severity": "error",
                "code": err.code(),
                "message": err.message(),
                "line": table.line
            }));
        }
    }
    let mut open_blocks = Vec::new();
    for (idx, line) in doc.lines.iter().enumerate() {
        if let Some(marker) = parse_marker(line) {
            if marker.kind == "begin" {
                open_blocks.push((idx, marker));
            } else if marker.kind == "end" {
                let end_id = marker.fields.get("id").cloned();
                if let Some(pos) = open_blocks
                    .iter()
                    .rposition(|(_, m)| m.fields.get("id") == end_id.as_ref())
                {
                    open_blocks.remove(pos);
                } else {
                    issues.push(json!({
                        "rule": "managed-blocks-balanced",
                        "severity": "error",
                        "message": "managed block end marker has no matching begin marker",
                        "line": idx + 1
                    }));
                }
            }
        }
    }
    for (idx, _) in open_blocks {
        issues.push(json!({
            "rule": "managed-blocks-balanced",
            "severity": "error",
            "message": "managed block begin marker has no matching end marker",
            "line": idx + 1
        }));
    }
    for block in &index.blocks {
        if let Some(expected) = &block.checksum {
            let actual = checksum_body(&doc.lines[block.start + 1..block.end - 1]);
            if &actual != expected {
                issues.push(json!({
                    "rule": "managed-blocks-checksum",
                    "severity": "error",
                    "code": "E_BLOCK_MODIFIED",
                    "message": format!("managed block {} checksum mismatch", block.id),
                    "line": block.line
                }));
            }
        }
    }
    let mut previous = 0;
    for section in &index.sections {
        if previous > 0 && section.level > previous + 1 {
            issues.push(json!({
                "rule": "heading-hierarchy",
                "severity": "warn",
                "message": "heading level skips a level",
                "line": section.line
            }));
        }
        previous = section.level;
    }
    issues
}

pub(crate) fn table_fmt_all_safe(doc: &mut MarkdownDocument) -> Result<(), MdliError> {
    let index = index_document(doc);
    for table in index.tables.into_iter().rev() {
        let data =
            table_data_from_lines(&doc.lines[table.start..table.end], table.marker.is_some())?;
        let rendered = render_existing_table(&data, table.name.as_deref(), table.key.as_ref());
        doc.lines.splice(table.start..table.end, rendered);
    }
    Ok(())
}
