use serde_json::json;

use crate::{
    ensure_or_replace_block_in_section, id_marker, index_document, parse_heading, parse_marker,
    read_text_path, resolve_section, selector_from_id_path, split_body_lines, validate_id,
    validate_write_emit, MarkdownDocument, MdliError, MutationOutcome, Outcome, SectionCommand,
    SectionEnsureArgs, SectionMoveArgs, SectionRenameArgs, SectionReplaceArgs,
    SectionSelectMutateArgs,
};

pub(crate) fn run_section(cmd: SectionCommand) -> Result<Outcome, MdliError> {
    match cmd {
        SectionCommand::List(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            Ok(Outcome::Json(json!({"sections": index.sections})))
        }
        SectionCommand::Get(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            let selector = selector_from_id_path(args.id.as_deref(), args.path.as_deref())?;
            let section = resolve_section(&index, &selector)?;
            Ok(Outcome::Text(
                doc.lines[section.start..section.end].join(&doc.line_ending),
            ))
        }
        SectionCommand::Ensure(args) => section_ensure(args),
        SectionCommand::Replace(args) => section_replace(args),
        SectionCommand::Delete(args) => section_delete(args),
        SectionCommand::Move(args) => section_move(args),
        SectionCommand::Rename(args) => section_rename(args),
    }
}

pub(crate) fn section_ensure(args: SectionEnsureArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let mut warnings = Vec::new();
    let op = apply_section_ensure(
        &mut doc,
        SectionEnsureInput {
            id: args.id,
            path: args.path,
            level: args.level,
            after: args.after,
            before: args.before,
            enforce_path: args.enforce_path,
        },
    )?;
    let changed = before != doc.render();
    if let Some(warning) = op.get("warning") {
        warnings.push(warning.clone());
    }
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![op],
        warnings,
        flags: args.mutate,
    }))
}

#[derive(Debug)]
pub(crate) struct SectionEnsureInput {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) level: usize,
    pub(crate) after: Option<String>,
    pub(crate) before: Option<String>,
    pub(crate) enforce_path: bool,
}

pub(crate) fn apply_section_ensure(
    doc: &mut MarkdownDocument,
    input: SectionEnsureInput,
) -> Result<serde_json::Value, MdliError> {
    validate_id(&input.id)?;
    if !(1..=6).contains(&input.level) {
        return Err(MdliError::user(
            "E_INVALID_LEVEL",
            "--level must be 1 through 6",
        ));
    }
    if input.after.is_some() && input.before.is_some() {
        return Err(MdliError::user(
            "E_INVALID_POSITION",
            "--after and --before are mutually exclusive",
        ));
    }
    let index = index_document(doc);

    if let Some(section) = index
        .sections
        .iter()
        .find(|s| s.id.as_deref() == Some(input.id.as_str()))
    {
        if section.level != input.level {
            return Err(MdliError::user(
                "E_SECTION_LEVEL",
                format!(
                    "section {} is level {}, not {}",
                    input.id, section.level, input.level
                ),
            ));
        }
        if section.path != input.path {
            if input.enforce_path {
                return Err(MdliError::user(
                    "E_SECTION_PATH",
                    format!(
                        "section {} path is {}, not {}",
                        input.id, section.path, input.path
                    ),
                ));
            }
            let warning = json!({
                "code": "W_PATH_MISMATCH",
                "message": "stable ID matched a section with a different visible path",
                "id": input.id,
                "actual_path": section.path,
                "requested_path": input.path,
            });
            return Ok(json!({
                "op": "ensure_section",
                "action": "reuse",
                "id": input.id,
                "path": section.path,
                "level": section.level,
                "line": section.line,
                "warning": warning
            }));
        }
        Ok(json!({
            "op": "ensure_section",
            "action": "reuse",
            "id": input.id,
            "path": section.path,
            "level": section.level,
            "line": section.line
        }))
    } else if let Ok(section) = resolve_section(&index, &input.path) {
        if section.level != input.level {
            return Err(MdliError::user(
                "E_SECTION_LEVEL",
                format!(
                    "section path is level {}, not {}",
                    section.level, input.level
                ),
            ));
        }
        doc.lines.insert(section.heading, id_marker(&input.id));
        Ok(json!({
            "op": "ensure_section",
            "action": "assign_id",
            "id": input.id,
            "path": section.path,
            "level": section.level,
            "line": section.line
        }))
    } else {
        let title = input
            .path
            .split('>')
            .next_back()
            .map(|s| s.trim().replace("\\>", ">"))
            .filter(|s| !s.is_empty())
            .ok_or_else(|| MdliError::user("E_INVALID_PATH", "--path cannot be empty"))?;
        let insert_at = if let Some(sel) = input.after.as_deref() {
            resolve_section(&index, sel)?.end
        } else if let Some(sel) = input.before.as_deref() {
            resolve_section(&index, sel)?.start
        } else {
            doc.lines.len()
        };
        let mut new_lines = Vec::new();
        if insert_at > 0
            && doc
                .lines
                .get(insert_at.saturating_sub(1))
                .map(|l| !l.trim().is_empty())
                .unwrap_or(false)
        {
            new_lines.push(String::new());
        }
        new_lines.push(id_marker(&input.id));
        new_lines.push(format!("{} {}", "#".repeat(input.level), title));
        new_lines.push(String::new());
        doc.lines.splice(insert_at..insert_at, new_lines);
        doc.trailing_newline = true;
        Ok(json!({
            "op": "ensure_section",
            "action": "create",
            "id": input.id,
            "path": input.path,
            "level": input.level,
            "insert_line": insert_at + 1
        }))
    }
}

pub(crate) fn section_replace(args: SectionReplaceArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let selector = selector_from_id_path(args.id.as_deref(), args.path.as_deref())?;
    let mut ops = Vec::new();

    if args.body_from_file.is_none() && args.section_from_file.is_none() {
        return Err(MdliError::user(
            "E_REPLACEMENT_REQUIRED",
            "--body-from-file or --section-from-file is required",
        ));
    }

    if let Some(path) = args.body_from_file {
        let body = read_text_path(&path)?;
        let body_lines = split_body_lines(&body);
        if args.managed {
            let block_id = format!(
                "{}.generated",
                args.id.clone().unwrap_or_else(|| selector.clone())
            );
            ensure_or_replace_block_in_section(
                &mut doc, &selector, &block_id, body_lines, "end", true,
            )?;
            ops.push(json!({"op": "replace_managed_section_body", "section": selector, "block": block_id}));
        } else {
            let index = index_document(&doc);
            let section = resolve_section(&index, &selector)?;
            let body_start = section.heading + 1;
            doc.lines.splice(body_start..section.end, body_lines);
            ops.push(json!({"op": "replace_section_body", "section": selector}));
        }
    } else if let Some(path) = args.section_from_file {
        let replacement = read_text_path(&path)?;
        let mut replacement_lines = split_body_lines(&replacement);
        let index = index_document(&doc);
        let section = resolve_section(&index, &selector)?;
        let id = section.id.clone().or(args.id.clone()).ok_or_else(|| {
            MdliError::user(
                "E_ID_REQUIRED",
                "whole-section replacement requires an id-selected or already-id-marked section",
            )
        })?;
        if replacement_lines
            .iter()
            .take_while(|l| l.trim().is_empty())
            .all(|l| parse_marker(l).map(|m| m.kind != "id").unwrap_or(true))
        {
            let first_heading = replacement_lines
                .iter()
                .position(|l| parse_heading(l).is_some())
                .ok_or_else(|| {
                    MdliError::user(
                        "E_SECTION_INVALID",
                        "replacement section must contain a heading",
                    )
                })?;
            replacement_lines.insert(first_heading, id_marker(&id));
        }
        doc.lines
            .splice(section.start..section.end, replacement_lines);
        ops.push(json!({"op": "replace_section", "id": id}));
    }

    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops,
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn section_delete(args: SectionSelectMutateArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let selector = selector_from_id_path(args.id.as_deref(), args.path.as_deref())?;
    let index = index_document(&doc);
    let section = resolve_section(&index, &selector)?;
    doc.lines.drain(section.start..section.end);
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": "delete_section", "selector": selector})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn section_move(args: SectionMoveArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    if args.after.is_none() && args.before.is_none() {
        return Err(MdliError::user(
            "E_POSITION_REQUIRED",
            "--after or --before is required",
        ));
    }
    let before_render = doc.render();
    let index = index_document(&doc);
    let section = resolve_section(&index, &args.id)?;
    let target = resolve_section(
        &index,
        args.after.as_deref().or(args.before.as_deref()).unwrap(),
    )?;
    if target.start >= section.start && target.start < section.end {
        return Err(MdliError::user(
            "E_INVALID_MOVE",
            "cannot move a section relative to itself or its child",
        ));
    }
    let moved = doc.lines[section.start..section.end].to_vec();
    doc.lines.drain(section.start..section.end);
    let adjusted_index = index_document(&doc);
    let target = resolve_section(
        &adjusted_index,
        args.after.as_deref().or(args.before.as_deref()).unwrap(),
    )?;
    let insert_at = if args.after.is_some() {
        target.end
    } else {
        target.start
    };
    doc.lines.splice(insert_at..insert_at, moved);
    let changed = before_render != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": "move_section", "id": args.id})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn section_rename(args: SectionRenameArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let index = index_document(&doc);
    let section = resolve_section(&index, &args.id)?;
    doc.lines[section.heading] = format!("{} {}", "#".repeat(section.level), args.to);
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": "rename_section", "id": args.id, "title": args.to})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}
