use std::path::Path;

use serde_json::json;

use crate::*;

pub(crate) fn run_block(cmd: BlockCommand) -> Result<Outcome, MdliError> {
    match cmd {
        BlockCommand::List(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            Ok(Outcome::Json(json!({"blocks": index.blocks})))
        }
        BlockCommand::Get(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            let block = resolve_block(&index, &args.id)?;
            Ok(Outcome::Text(
                doc.lines[block.start + 1..block.end - 1].join(&doc.line_ending),
            ))
        }
        BlockCommand::Ensure(args) => {
            let mut doc = MarkdownDocument::read(&args.file)?;
            doc.assert_preimage(&args.mutate.preimage_hash)?;
            validate_write_emit(&args.mutate)?;
            let before = doc.render();
            let body = block_body_from_args(args.body_from_file.as_deref(), args.text.as_deref())?;
            ensure_or_replace_block_in_section(
                &mut doc,
                &args.parent_section,
                &args.id,
                body,
                &args.position,
                false,
            )?;
            let changed = before != doc.render();
            Ok(Outcome::Mutated(MutationOutcome {
                document: doc,
                changed,
                ops: vec![json!({"op": "ensure_block", "id": args.id})],
                warnings: Vec::new(),
                flags: args.mutate,
            }))
        }
        BlockCommand::Replace(args) => {
            let mut doc = MarkdownDocument::read(&args.file)?;
            doc.assert_preimage(&args.mutate.preimage_hash)?;
            validate_write_emit(&args.mutate)?;
            let before = doc.render();
            let body = split_body_lines(&read_text_path(&args.body_from_file)?);
            replace_block(&mut doc, &args.id, body, &args.on_modified)?;
            let changed = before != doc.render();
            Ok(Outcome::Mutated(MutationOutcome {
                document: doc,
                changed,
                ops: vec![json!({"op": "replace_block", "id": args.id})],
                warnings: Vec::new(),
                flags: args.mutate,
            }))
        }
        BlockCommand::Lock(args) => block_set_lock(args, true),
        BlockCommand::Unlock(args) => block_set_lock(args, false),
    }
}

pub(crate) fn block_body_from_args(
    body_from_file: Option<&Path>,
    text: Option<&str>,
) -> Result<Vec<String>, MdliError> {
    if let Some(path) = body_from_file {
        Ok(split_body_lines(&read_text_path(path)?))
    } else if let Some(text) = text {
        Ok(split_body_lines(text))
    } else {
        Err(MdliError::user(
            "E_REPLACEMENT_REQUIRED",
            "--body-from-file or --text is required",
        ))
    }
}

pub(crate) fn ensure_or_replace_block_in_section(
    doc: &mut MarkdownDocument,
    parent_section: &str,
    block_id: &str,
    body: Vec<String>,
    position: &str,
    force: bool,
) -> Result<(), MdliError> {
    let index = index_document(doc);
    if index.blocks.iter().any(|b| b.id == block_id) {
        replace_block(
            doc,
            block_id,
            body,
            if force {
                &OnModified::Force
            } else {
                &OnModified::Fail
            },
        )?;
        return Ok(());
    }
    let section = resolve_section(&index, parent_section)?;
    let block_lines = render_block_lines(block_id, body, false);
    let insert_at = if position == "start" {
        section.heading + 1
    } else if let Some(other_id) = position.strip_prefix("before:") {
        resolve_block(&index, other_id)?.start
    } else if let Some(other_id) = position.strip_prefix("after:") {
        resolve_block(&index, other_id)?.end
    } else {
        section.end
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
    insertion.extend(block_lines);
    insertion.push(String::new());
    doc.lines.splice(insert_at..insert_at, insertion);
    Ok(())
}

pub(crate) fn replace_block(
    doc: &mut MarkdownDocument,
    block_id: &str,
    body: Vec<String>,
    on_modified: &OnModified,
) -> Result<(), MdliError> {
    let index = index_document(doc);
    let block = resolve_block(&index, block_id)?;
    if block.locked {
        return Err(MdliError::invariant(
            "E_BLOCK_LOCKED",
            format!("block {block_id} is locked"),
        ));
    }
    if let Some(expected) = &block.checksum {
        let actual = checksum_body(&doc.lines[block.start + 1..block.end - 1]);
        if expected != &actual {
            match on_modified {
                OnModified::Fail => {
                    return Err(MdliError::invariant(
                        "E_BLOCK_MODIFIED",
                        format!("block {block_id} checksum does not match"),
                    ));
                }
                OnModified::ThreeWay => {
                    return Err(MdliError::invariant(
                        "E_BLOCK_MODIFIED",
                        "three-way conflict artifacts are not implemented in this MVP",
                    ));
                }
                OnModified::Force => {}
            }
        }
    }
    let rendered = render_block_lines(block_id, body, false);
    doc.lines.splice(block.start..block.end, rendered);
    Ok(())
}

pub(crate) fn block_set_lock(args: BlockGetMutateArgs, locked: bool) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let index = index_document(&doc);
    let block = resolve_block(&index, &args.id)?;
    let marker = parse_marker(&doc.lines[block.start])
        .ok_or_else(|| MdliError::invariant("E_ORPHAN_MARKER", "block begin marker missing"))?;
    let mut fields = marker.fields;
    if locked {
        fields.insert("locked".to_string(), "true".to_string());
    } else {
        fields.remove("locked");
    }
    doc.lines[block.start] = render_marker("begin", &fields);
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": if locked { "lock_block" } else { "unlock_block" }, "id": args.id})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn render_block_lines(block_id: &str, body: Vec<String>, locked: bool) -> Vec<String> {
    let checksum = checksum_body(&body);
    let mut lines = Vec::new();
    lines.push(begin_marker(block_id, &checksum, locked));
    lines.extend(body);
    lines.push(end_marker(block_id));
    lines
}

pub(crate) fn checksum_body(lines: &[String]) -> String {
    let mut body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    sha256_prefixed(body.as_bytes())
}
