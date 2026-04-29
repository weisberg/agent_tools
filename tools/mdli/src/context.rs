use serde_json::{json, Value};

use crate::*;

pub(crate) fn run_context(args: ContextArgs) -> Result<Outcome, MdliError> {
    let doc = MarkdownDocument::read(&args.file)?;
    let index = index_document(&doc);
    let selector = selector_from_id_path(args.id.as_deref(), args.path.as_deref())?;
    let section = resolve_section(&index, &selector)?;

    let breadcrumbs = build_breadcrumbs(&index.sections, &section);
    let siblings = collect_siblings(&index.sections, &section);
    let children = collect_children(&index.sections, &section);
    let managed = if args.include_managed_blocks {
        collect_managed_blocks(&index.blocks, &section)
    } else {
        Vec::new()
    };

    let body_lines = &doc.lines[section.start..section.end];
    let body_text = body_lines.join(&doc.line_ending);
    let byte_range = byte_range_for_lines(&doc, section.start, section.end);
    let line_range = json!([section.start + 1, section.end]);

    let (body_emitted, truncated, tokens_estimated) = if args.include_body {
        truncate_to_tokens(&body_text, args.max_tokens)
    } else {
        (String::new(), false, 0)
    };

    Ok(Outcome::Json(json!({
        "selected": {
            "id": section.id,
            "title": section.title,
            "level": section.level,
            "path": section.path,
            "line_range": line_range,
            "byte_range": byte_range,
            "tokens_estimated": tokens_estimated,
            "truncated": truncated,
            "body": body_emitted,
        },
        "breadcrumbs": breadcrumbs,
        "siblings": siblings,
        "children": children,
        "managed_blocks": managed,
        "max_tokens": args.max_tokens,
    })))
}

fn build_breadcrumbs(sections: &[SectionInfo], target: &SectionInfo) -> Vec<Value> {
    let mut crumbs = Vec::new();
    for ancestor in sections.iter().take_while(|s| s.start < target.start) {
        if ancestor.level < target.level {
            // Pop entries with level >= ancestor.level so we keep only the
            // active path through the document tree.
            while crumbs
                .last()
                .and_then(|v: &Value| v.get("level").and_then(|l| l.as_u64()))
                .map(|lvl| lvl as usize >= ancestor.level)
                .unwrap_or(false)
            {
                crumbs.pop();
            }
            crumbs.push(json!({
                "id": ancestor.id,
                "title": ancestor.title,
                "level": ancestor.level,
                "path": ancestor.path,
                "line": ancestor.line,
            }));
        }
    }
    crumbs
}

fn collect_siblings(sections: &[SectionInfo], target: &SectionInfo) -> Vec<Value> {
    sections
        .iter()
        .filter(|s| {
            s.level == target.level && s.start != target.start && shares_parent(sections, s, target)
        })
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "level": s.level,
                "path": s.path,
                "line": s.line,
                "position": if s.start < target.start { "before" } else { "after" },
            })
        })
        .collect()
}

fn collect_children(sections: &[SectionInfo], target: &SectionInfo) -> Vec<Value> {
    sections
        .iter()
        .filter(|s| s.start > target.heading && s.start < target.end && s.level == target.level + 1)
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "level": s.level,
                "path": s.path,
                "line": s.line,
            })
        })
        .collect()
}

fn shares_parent(sections: &[SectionInfo], a: &SectionInfo, b: &SectionInfo) -> bool {
    if a.level <= 1 {
        return b.level == a.level;
    }
    let parent_level = a.level - 1;
    let parent_a = nearest_ancestor(sections, a, parent_level);
    let parent_b = nearest_ancestor(sections, b, parent_level);
    match (parent_a, parent_b) {
        (Some(pa), Some(pb)) => pa.start == pb.start,
        (None, None) => true,
        _ => false,
    }
}

fn nearest_ancestor<'a>(
    sections: &'a [SectionInfo],
    s: &SectionInfo,
    level: usize,
) -> Option<&'a SectionInfo> {
    sections
        .iter()
        .filter(|cand| cand.level == level && cand.start < s.start && cand.end >= s.end)
        .next_back()
}

fn collect_managed_blocks(blocks: &[BlockInfo], section: &SectionInfo) -> Vec<Value> {
    blocks
        .iter()
        .filter(|b| b.start >= section.heading && b.end <= section.end)
        .map(|b| {
            json!({
                "id": b.id,
                "locked": b.locked,
                "checksum": b.checksum,
                "line_range": [b.start + 1, b.end],
            })
        })
        .collect()
}

fn byte_range_for_lines(doc: &MarkdownDocument, start: usize, end: usize) -> Value {
    let mut prefix_bytes: usize = 0;
    for line in &doc.lines[..start] {
        prefix_bytes += line.len() + doc.line_ending.len();
    }
    if doc.bom {
        prefix_bytes += 3;
    }
    let mut span_bytes: usize = 0;
    for (idx, line) in doc.lines[start..end].iter().enumerate() {
        span_bytes += line.len();
        if start + idx < end - 1 {
            span_bytes += doc.line_ending.len();
        }
    }
    json!([prefix_bytes, prefix_bytes + span_bytes])
}

/// Approximate `chars / 4` as token count and truncate at a line boundary.
/// Returns (body, truncated, tokens_estimated).
pub(crate) fn truncate_to_tokens(body: &str, max_tokens: usize) -> (String, bool, usize) {
    let tokens = (body.chars().count() + 3) / 4;
    if tokens <= max_tokens {
        return (body.to_string(), false, tokens);
    }
    let target_chars = max_tokens.saturating_mul(4);
    let mut kept = 0usize;
    let mut last_nl: Option<usize> = None;
    let mut idx = 0usize;
    for (i, c) in body.char_indices() {
        if kept >= target_chars {
            break;
        }
        if c == '\n' {
            last_nl = Some(i);
        }
        kept += 1;
        idx = i + c.len_utf8();
    }
    let cut = last_nl.map(|n| n + 1).unwrap_or(idx);
    let mut out = body[..cut].to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("…\n");
    let tokens_emitted = (out.chars().count() + 3) / 4;
    (out, true, tokens_emitted)
}
