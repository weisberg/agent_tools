use std::collections::HashMap;

use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_tree(args: FileArgs) -> Result<Outcome, MdliError> {
    let doc = MarkdownDocument::read(&args.file)?;
    let index = index_document(&doc);
    Ok(Outcome::Json(json!({
        "tree": build_tree(&index.sections)
    })))
}

pub(crate) fn build_tree(sections: &[SectionInfo]) -> Vec<Value> {
    let mut roots: Vec<usize> = Vec::new();
    let mut children_of: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut stack: Vec<usize> = Vec::new();

    for (idx, section) in sections.iter().enumerate() {
        while let Some(&top) = stack.last() {
            if sections[top].level >= section.level {
                stack.pop();
            } else {
                break;
            }
        }
        if let Some(&parent) = stack.last() {
            children_of.entry(parent).or_default().push(idx);
        } else {
            roots.push(idx);
        }
        stack.push(idx);
    }

    roots
        .into_iter()
        .map(|i| materialize(i, sections, &children_of))
        .collect()
}

fn materialize(
    idx: usize,
    sections: &[SectionInfo],
    children_of: &HashMap<usize, Vec<usize>>,
) -> Value {
    let section = &sections[idx];
    let children: Vec<Value> = children_of
        .get(&idx)
        .map(|kids| {
            kids.iter()
                .map(|&c| materialize(c, sections, children_of))
                .collect()
        })
        .unwrap_or_default();
    json!({
        "id": section.id,
        "title": section.title,
        "level": section.level,
        "line": section.line,
        "path": section.path,
        "children": children,
    })
}
