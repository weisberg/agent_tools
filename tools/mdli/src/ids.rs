use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_id(cmd: IdCommand) -> Result<Outcome, MdliError> {
    match cmd {
        IdCommand::List(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            Ok(Outcome::Json(json!({
                "ids": index.sections.iter().filter_map(|s| s.id.as_ref().map(|id| json!({
                    "id": id,
                    "path": s.path,
                    "line": s.line,
                    "level": s.level,
                }))).collect::<Vec<_>>()
            })))
        }
        IdCommand::Assign(args) => {
            let mut doc = MarkdownDocument::read(&args.file)?;
            doc.assert_preimage(&args.mutate.preimage_hash)?;
            validate_write_emit(&args.mutate)?;
            let before = doc.render();
            let mut ops = Vec::new();
            if args.all {
                assign_all_ids(&mut doc, &mut ops)?;
            } else {
                let selector = args.section.as_ref().ok_or_else(|| {
                    MdliError::user(
                        "E_SELECTOR_REQUIRED",
                        "--section is required unless --all is passed",
                    )
                })?;
                let id = if args.auto {
                    None
                } else {
                    Some(
                        args.id
                            .as_ref()
                            .ok_or_else(|| {
                                MdliError::user("E_ID_REQUIRED", "--id or --auto is required")
                            })?
                            .clone(),
                    )
                };
                assign_one_id(&mut doc, selector, id, &mut ops)?;
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
    }
}

pub(crate) fn assign_all_ids(
    doc: &mut MarkdownDocument,
    ops: &mut Vec<Value>,
) -> Result<(), MdliError> {
    let index = index_document(doc);
    let mut existing = index
        .sections
        .iter()
        .filter_map(|s| s.id.clone())
        .collect::<BTreeSet<_>>();
    let mut inserts = Vec::new();
    for section in index.sections.iter().filter(|s| s.id.is_none()) {
        let id = unique_slug(&section.title, &mut existing);
        inserts.push((section.heading, id, section.path.clone()));
    }
    for (heading, id, path) in inserts.into_iter().rev() {
        doc.lines.insert(heading, id_marker(&id));
        ops.push(json!({"op": "assign_id", "id": id, "path": path}));
    }
    Ok(())
}

pub(crate) fn assign_one_id(
    doc: &mut MarkdownDocument,
    selector: &str,
    id: Option<String>,
    ops: &mut Vec<Value>,
) -> Result<(), MdliError> {
    let index = index_document(doc);
    let section = resolve_section(&index, selector)?;
    let mut existing = index
        .sections
        .iter()
        .filter_map(|s| s.id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(current) = &section.id {
        if id.as_ref().map(|v| v == current).unwrap_or(false) {
            return Ok(());
        }
        return Err(MdliError::invariant(
            "E_DUPLICATE_ID",
            format!("section already has id {current}"),
        ));
    }
    let new_id = id.unwrap_or_else(|| unique_slug(&section.title, &mut existing));
    validate_id(&new_id)?;
    if existing.contains(&new_id) {
        return Err(MdliError::invariant(
            "E_DUPLICATE_ID",
            format!("stable ID {new_id} already exists"),
        ));
    }
    doc.lines.insert(section.heading, id_marker(&new_id));
    ops.push(json!({"op": "assign_id", "id": new_id, "path": section.path}));
    Ok(())
}
