use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_plan(args: PlanArgs) -> Result<Outcome, MdliError> {
    let doc = MarkdownDocument::read(&args.file)?;
    let recipe = load_recipe(&args.recipe)?;
    recipe.validate_schema()?;
    let datasets = parse_data_args(&args.data)?;
    let recipe_dir = args
        .recipe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let recipe_hash = sha256_prefixed(&fs::read(&args.recipe).unwrap_or_default());

    // Replay the recipe on a clone to capture ops.
    let mut clone = doc.clone();
    let mut ops = Vec::new();
    apply_recipe(&mut clone, &recipe, &datasets, &recipe_dir, &recipe_hash, &mut ops)?;
    let preimage_hash = doc.preimage_hash.clone();
    let postimage_hash = sha256_prefixed(clone.render().as_bytes());

    let mut sections_changed: Vec<String> = Vec::new();
    for op in &ops {
        if let Some(s) = op
            .get("section")
            .or_else(|| op.get("id"))
            .and_then(|v| v.as_str())
        {
            let s = s.to_string();
            if !sections_changed.contains(&s) {
                sections_changed.push(s);
            }
        }
    }
    Ok(Outcome::Json(json!({
        "preimage_hash": preimage_hash,
        "postimage_hash": postimage_hash,
        "recipe_hash": recipe_hash,
        "ops": ops,
        "sections_changed": sections_changed,
    })))
}

pub(crate) fn run_apply_plan(args: ApplyPlanArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;

    let plan_text = fs::read_to_string(&args.plan).map_err(|e| {
        MdliError::io(
            "E_READ_FAILED",
            format!("failed to read plan {}", args.plan.display()),
            e,
        )
    })?;
    let plan: Value = serde_json::from_str(&plan_text)
        .map_err(|e| MdliError::user("E_RECIPE_INVALID", format!("invalid plan: {e}")))?;

    let plan_root = plan
        .get("result")
        .cloned()
        .unwrap_or_else(|| plan.clone());
    if let Some(expected) = plan_root.get("preimage_hash").and_then(|v| v.as_str()) {
        if expected != doc.preimage_hash {
            return Err(MdliError::io(
                "E_STALE_PREIMAGE",
                "plan preimage_hash does not match document",
                std::io::Error::other("stale preimage"),
            ));
        }
    }
    let ops = plan_root
        .get("ops")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let before = doc.render();
    let mut applied = Vec::new();
    for op in &ops {
        apply_plan_op(&mut doc, op)?;
        applied.push(op.clone());
    }
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: applied,
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn run_patch(args: PatchArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let edits_text = fs::read_to_string(&args.edits).map_err(|e| {
        MdliError::io(
            "E_READ_FAILED",
            format!("failed to read edits {}", args.edits.display()),
            e,
        )
    })?;
    let edits: Value = serde_json::from_str(&edits_text)
        .map_err(|e| MdliError::user("E_RECIPE_INVALID", format!("invalid edits JSON: {e}")))?;
    let ops = edits.as_array().cloned().ok_or_else(|| {
        MdliError::user("E_RECIPE_INVALID", "edits file must be a JSON array")
    })?;
    let before = doc.render();
    let mut applied = Vec::new();
    for op in &ops {
        apply_plan_op(&mut doc, op)?;
        applied.push(op.clone());
    }
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: applied,
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

fn apply_plan_op(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let kind = op
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MdliError::user("E_RECIPE_INVALID", "plan op missing 'op' field"))?;
    match kind {
        "ensure_section" => apply_ensure_section(doc, op),
        "assign_id" => apply_assign_id(doc, op),
        "set_frontmatter" => apply_set_frontmatter(doc, op),
        "delete_frontmatter" => apply_delete_frontmatter(doc, op),
        "rename_section" => apply_rename_section(doc, op),
        "delete_section" => apply_delete_section(doc, op),
        "replace_section_body" => apply_replace_section_body(doc, op),
        "replace_block" | "replace_managed_section_body" => apply_replace_block(doc, op),
        "ensure_block" => apply_ensure_block(doc, op),
        "lock_block" | "unlock_block" => apply_set_block_lock(doc, op, kind == "lock_block"),
        "replace_table" => apply_replace_table(doc, op),
        other => Err(MdliError::user(
            "E_RECIPE_INVALID",
            format!("unknown plan op {other}"),
        )),
    }
}

fn require_str<'a>(op: &'a Value, key: &str) -> Result<&'a str, MdliError> {
    op.get(key).and_then(|v| v.as_str()).ok_or_else(|| {
        MdliError::user(
            "E_RECIPE_INVALID",
            format!("plan op missing required field {key}"),
        )
    })
}

fn apply_ensure_section(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let id = require_str(op, "id")?.to_string();
    let path = require_str(op, "path")?.to_string();
    let level = op
        .get("level")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| MdliError::user("E_RECIPE_INVALID", "ensure_section needs level"))?
        as usize;
    let after = op
        .get("after_id")
        .or_else(|| op.get("after"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let before = op
        .get("before_id")
        .or_else(|| op.get("before"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    ensure_section_in_doc(doc, &id, &path, level, after.as_deref(), before.as_deref())
}

fn apply_assign_id(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let id = require_str(op, "id")?.to_string();
    let path = require_str(op, "path")?.to_string();
    let mut ops = Vec::new();
    assign_one_id(doc, &path, Some(id), &mut ops)
}

fn apply_set_frontmatter(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let key = require_str(op, "key")?.to_string();
    let value = op.get("value").and_then(|v| v.as_str()).map(String::from);
    set_frontmatter_key(doc, &key, value);
    Ok(())
}

fn apply_delete_frontmatter(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let key = require_str(op, "key")?.to_string();
    set_frontmatter_key(doc, &key, None);
    Ok(())
}

fn apply_rename_section(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let id = require_str(op, "id")?;
    let title = require_str(op, "title")?;
    let index = index_document(doc);
    let section = resolve_section(&index, id)?;
    doc.lines[section.heading] = format!("{} {}", "#".repeat(section.level), title);
    Ok(())
}

fn apply_delete_section(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let selector = op
        .get("selector")
        .or_else(|| op.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            MdliError::user("E_RECIPE_INVALID", "delete_section needs selector or id")
        })?;
    let index = index_document(doc);
    let section = resolve_section(&index, selector)?;
    doc.lines.drain(section.start..section.end);
    Ok(())
}

fn apply_replace_section_body(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let selector = op
        .get("section")
        .or_else(|| op.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            MdliError::user("E_RECIPE_INVALID", "replace_section_body needs section or id")
        })?;
    let body_path = op
        .get("body_from_file")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            MdliError::user(
                "E_RECIPE_INVALID",
                "replace_section_body needs body_from_file",
            )
        })?;
    let body = read_text_path(Path::new(body_path))?;
    let body_lines = split_body_lines(&body);
    let index = index_document(doc);
    let section = resolve_section(&index, selector)?;
    let body_start = section.heading + 1;
    doc.lines.splice(body_start..section.end, body_lines);
    Ok(())
}

fn apply_replace_block(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let id = require_str(op, "id")?.to_string();
    let body = if let Some(file) = op.get("body_from_file").and_then(|v| v.as_str()) {
        read_text_path(Path::new(file))?
    } else if let Some(text) = op.get("text").and_then(|v| v.as_str()) {
        text.to_string()
    } else {
        return Err(MdliError::user(
            "E_RECIPE_INVALID",
            "replace_block needs body_from_file or text",
        ));
    };
    let body_lines = split_body_lines(&body);
    let parent = op
        .get("parent_section")
        .or_else(|| op.get("section"))
        .and_then(|v| v.as_str());
    if let Some(parent) = parent {
        return ensure_or_replace_block_in_section(doc, parent, &id, body_lines, "end", true);
    }
    let on_modified = match op.get("on_modified").and_then(|v| v.as_str()) {
        Some("force") => OnModified::Force,
        Some("three-way") => OnModified::ThreeWay,
        _ => OnModified::Fail,
    };
    replace_block(doc, &id, body_lines, &on_modified)
}

fn apply_ensure_block(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let id = require_str(op, "id")?.to_string();
    let parent_section = require_str(op, "parent_section")?.to_string();
    let body = if let Some(file) = op.get("body_from_file").and_then(|v| v.as_str()) {
        read_text_path(Path::new(file))?
    } else if let Some(text) = op.get("text").and_then(|v| v.as_str()) {
        text.to_string()
    } else {
        return Err(MdliError::user(
            "E_RECIPE_INVALID",
            "ensure_block needs body_from_file or text",
        ));
    };
    let position = op
        .get("position")
        .and_then(|v| v.as_str())
        .unwrap_or("end")
        .to_string();
    ensure_or_replace_block_in_section(
        doc,
        &parent_section,
        &id,
        split_body_lines(&body),
        &position,
        false,
    )
}

fn apply_set_block_lock(
    doc: &mut MarkdownDocument,
    op: &Value,
    locked: bool,
) -> Result<(), MdliError> {
    let id = require_str(op, "id")?.to_string();
    let index = index_document(doc);
    let block = resolve_block(&index, &id)?;
    let marker = parse_marker(&doc.lines[block.start])
        .ok_or_else(|| MdliError::invariant("E_ORPHAN_MARKER", "block begin marker missing"))?;
    let mut fields = marker.fields;
    if locked {
        fields.insert("locked".to_string(), "true".to_string());
    } else {
        fields.remove("locked");
    }
    doc.lines[block.start] = render_marker("begin", &fields);
    Ok(())
}

fn apply_replace_table(doc: &mut MarkdownDocument, op: &Value) -> Result<(), MdliError> {
    let section = require_str(op, "section_id").or_else(|_| require_str(op, "section"))?;
    let name = op.get("name").and_then(|v| v.as_str()).map(String::from);
    let column_specs = op
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MdliError::user("E_RECIPE_INVALID", "replace_table needs columns"))?
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let from_rows = require_str(op, "rows_from")?.to_string();
    let key = op.get("key").and_then(|v| v.as_str()).map(String::from);
    let columns = parse_columns(&column_specs)?;
    let rows = read_rows(Path::new(&from_rows))?;
    let render_options = RenderTableOptions {
        columns,
        key: key.clone(),
        sort: op.get("sort").and_then(|v| v.as_str()).map(String::from),
        missing: MissingMode::Empty,
        rich_cell: RichCellMode::Error,
        duplicate_key: DuplicateKeyMode::Error,
        empty: op.get("empty").and_then(|v| v.as_str()).map(String::from),
        links: std::collections::BTreeMap::new(),
        truncates: std::collections::BTreeMap::new(),
        escape_markdown: false,
    };
    let rendered = render_table_from_rows(&rows, &render_options)?;
    let index = index_document(doc);
    let parent = resolve_section(&index, section)?;
    let existing = name
        .as_deref()
        .and_then(|n| {
            index
                .tables
                .iter()
                .find(|t| t.name.as_deref() == Some(n))
                .cloned()
        })
        .or_else(|| {
            index
                .tables
                .iter()
                .find(|t| t.start >= parent.heading && t.end <= parent.end)
                .cloned()
        });
    let mut replacement = Vec::new();
    if let Some(name) = &name {
        replacement.push(table_marker(name, key.as_deref()));
    }
    replacement.extend(rendered.lines);
    if let Some(table) = existing {
        doc.lines.splice(table.start..table.end, replacement);
    } else {
        let insert_at = parent.end;
        let mut insertion = Vec::new();
        if insert_at > 0
            && doc
                .lines
                .get(insert_at - 1)
                .map(|l| !l.trim().is_empty())
                .unwrap_or(false)
        {
            insertion.push(String::new());
        }
        insertion.extend(replacement);
        insertion.push(String::new());
        doc.lines.splice(insert_at..insert_at, insertion);
    }
    Ok(())
}

pub(crate) fn ensure_section_in_doc(
    doc: &mut MarkdownDocument,
    id: &str,
    path: &str,
    level: usize,
    after: Option<&str>,
    before: Option<&str>,
) -> Result<(), MdliError> {
    validate_id(id)?;
    if !(1..=6).contains(&level) {
        return Err(MdliError::user(
            "E_INVALID_LEVEL",
            "level must be 1 through 6",
        ));
    }
    let index = index_document(doc);
    if index.sections.iter().any(|s| s.id.as_deref() == Some(id)) {
        return Ok(());
    }
    if let Ok(found) = resolve_section(&index, path) {
        doc.lines.insert(found.heading, id_marker(id));
        return Ok(());
    }
    let title = path
        .split('>')
        .next_back()
        .map(|s| s.trim().replace("\\>", ">"))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| MdliError::user("E_INVALID_PATH", "path cannot be empty"))?;
    let insert_at = if let Some(sel) = after {
        resolve_section(&index, sel)?.end
    } else if let Some(sel) = before {
        resolve_section(&index, sel)?.start
    } else {
        doc.lines.len()
    };
    let mut insertion = Vec::new();
    if insert_at > 0
        && doc
            .lines
            .get(insert_at - 1)
            .map(|l| !l.trim().is_empty())
            .unwrap_or(false)
    {
        insertion.push(String::new());
    }
    insertion.push(id_marker(id));
    insertion.push(format!("{} {}", "#".repeat(level), title));
    insertion.push(String::new());
    doc.lines.splice(insert_at..insert_at, insertion);
    doc.trailing_newline = true;
    Ok(())
}
