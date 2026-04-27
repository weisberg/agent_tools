use std::collections::BTreeMap;

use serde_json::json;

use crate::*;

pub(crate) fn run_frontmatter(cmd: FrontmatterCommand) -> Result<Outcome, MdliError> {
    match cmd {
        FrontmatterCommand::Get(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let map = parse_frontmatter_map(&doc.lines)?;
            if let Some(key) = args.key {
                Ok(Outcome::Json(json!({"key": key, "value": map.get(&key)})))
            } else {
                Ok(Outcome::Json(json!({"frontmatter": map})))
            }
        }
        FrontmatterCommand::Set(args) => {
            let mut doc = MarkdownDocument::read(&args.file)?;
            doc.assert_preimage(&args.mutate.preimage_hash)?;
            validate_write_emit(&args.mutate)?;
            let before = doc.render();
            set_frontmatter_key(&mut doc, &args.key, Some(args.value));
            let changed = before != doc.render();
            Ok(Outcome::Mutated(MutationOutcome {
                document: doc,
                changed,
                ops: vec![json!({"op": "set_frontmatter", "key": args.key})],
                warnings: Vec::new(),
                flags: args.mutate,
            }))
        }
        FrontmatterCommand::Delete(args) => {
            let mut doc = MarkdownDocument::read(&args.file)?;
            doc.assert_preimage(&args.mutate.preimage_hash)?;
            validate_write_emit(&args.mutate)?;
            let before = doc.render();
            set_frontmatter_key(&mut doc, &args.key, None);
            let changed = before != doc.render();
            Ok(Outcome::Mutated(MutationOutcome {
                document: doc,
                changed,
                ops: vec![json!({"op": "delete_frontmatter", "key": args.key})],
                warnings: Vec::new(),
                flags: args.mutate,
            }))
        }
    }
}

pub(crate) fn parse_frontmatter_map(
    lines: &[String],
) -> Result<BTreeMap<String, String>, MdliError> {
    let Some((start, end)) = frontmatter_range(lines) else {
        return Ok(BTreeMap::new());
    };
    let delimiter = lines[start].trim();
    let separator = if delimiter == "+++" { '=' } else { ':' };
    let mut map = BTreeMap::new();
    for line in &lines[start + 1..end - 1] {
        if let Some((key, value)) = line.split_once(separator) {
            map.insert(
                key.trim().to_string(),
                value.trim().trim_matches('"').to_string(),
            );
        }
    }
    Ok(map)
}

pub(crate) fn set_frontmatter_key(doc: &mut MarkdownDocument, key: &str, value: Option<String>) {
    let range = frontmatter_range(&doc.lines);
    let (start, end) = if let Some(range) = range {
        range
    } else {
        doc.lines.splice(
            0..0,
            vec!["---".to_string(), "---".to_string(), String::new()],
        );
        (0, 2)
    };
    let mut found = None;
    for idx in start + 1..end - 1 {
        if doc.lines[idx]
            .split_once(':')
            .map(|(k, _)| k.trim() == key)
            .unwrap_or(false)
        {
            found = Some(idx);
            break;
        }
    }
    match (found, value) {
        (Some(idx), Some(value)) => doc.lines[idx] = format!("{key}: {value}"),
        (Some(idx), None) => {
            doc.lines.remove(idx);
        }
        (None, Some(value)) => doc.lines.insert(end - 1, format!("{key}: {value}")),
        (None, None) => {}
    }
}
