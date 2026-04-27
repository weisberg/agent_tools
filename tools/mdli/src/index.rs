use crate::*;

pub(crate) fn index_document(doc: &MarkdownDocument) -> DocumentIndex {
    let markers = doc
        .lines
        .iter()
        .enumerate()
        .filter_map(|(line, text)| {
            parse_marker(text).map(|m| MarkerAt {
                kind: m.kind,
                fields: m.fields,
                line,
            })
        })
        .collect::<Vec<_>>();
    let sections = index_sections(&doc.lines);
    let tables = index_tables(&doc.lines, &sections);
    let blocks = index_blocks(&doc.lines);
    DocumentIndex {
        sections,
        tables,
        blocks,
        markers,
    }
}

pub(crate) fn index_sections(lines: &[String]) -> Vec<SectionInfo> {
    let mut sections = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let skip_until = frontmatter_range(lines).map(|(_, end)| end).unwrap_or(0);

    for (idx, line) in lines.iter().enumerate().skip(skip_until) {
        if let Some((level, title)) = parse_heading(line) {
            while stack.last().map(|(lvl, _)| *lvl >= level).unwrap_or(false) {
                stack.pop();
            }
            stack.push((level, title.clone()));
            let path = stack
                .iter()
                .map(|(_, t)| normalize_heading(t))
                .collect::<Vec<_>>()
                .join(" > ");
            let (id, marker_line) = bound_id_marker(lines, idx);
            sections.push(SectionInfo {
                id,
                path,
                title,
                level,
                line: idx + 1,
                start: marker_line.unwrap_or(idx),
                heading: idx,
                end: lines.len(),
            });
        }
    }

    for i in 0..sections.len() {
        let level = sections[i].level;
        let mut end = lines.len();
        for next in sections.iter().skip(i + 1) {
            if next.level <= level {
                end = next.start;
                break;
            }
        }
        sections[i].end = end;
    }

    sections
}

pub(crate) fn index_tables(lines: &[String], sections: &[SectionInfo]) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if is_table_header(&lines[i]) && is_table_separator(&lines[i + 1]) {
            let mut end = i + 2;
            while end < lines.len() && is_table_row(&lines[end]) {
                end += 1;
            }
            let (name, key, marker) = bound_table_marker(lines, i);
            let section = sections
                .iter()
                .rev()
                .find(|s| s.heading < i && i < s.end)
                .cloned();
            tables.push(TableInfo {
                name,
                key,
                columns: split_table_row(&lines[i]),
                line: i + 1,
                marker,
                start: marker.unwrap_or(i),
                end,
                section_id: section.as_ref().and_then(|s| s.id.clone()),
                section_path: section.map(|s| s.path),
            });
            i = end;
        } else {
            i += 1;
        }
    }
    tables
}

pub(crate) fn index_blocks(lines: &[String]) -> Vec<BlockInfo> {
    let mut blocks = Vec::new();
    let mut open: Vec<(usize, Marker)> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if let Some(marker) = parse_marker(line) {
            match marker.kind.as_str() {
                "begin" => open.push((idx, marker)),
                "end" => {
                    if let Some(pos) = open
                        .iter()
                        .rposition(|(_, m)| m.fields.get("id") == marker.fields.get("id"))
                    {
                        let (start, begin) = open.remove(pos);
                        if let Some(id) = begin.fields.get("id") {
                            blocks.push(BlockInfo {
                                id: id.clone(),
                                line: start + 1,
                                start,
                                end: idx + 1,
                                checksum: begin.fields.get("checksum").cloned(),
                                locked: begin
                                    .fields
                                    .get("locked")
                                    .map(|v| v == "true")
                                    .unwrap_or(false),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    blocks
}

pub(crate) fn parse_heading(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let mut level = 0;
    while level < bytes.len() && level < 6 && bytes[level] == b'#' {
        level += 1;
    }
    if level == 0 || bytes.get(level) != Some(&b' ') {
        return None;
    }
    let mut title = line[level..].trim().to_string();
    if let Some(stripped) = strip_closing_hashes(&title) {
        title = stripped;
    }
    Some((level, title))
}

pub(crate) fn strip_closing_hashes(title: &str) -> Option<String> {
    let trimmed = title.trim_end();
    if !trimmed.ends_with('#') {
        return None;
    }
    let without = trimmed.trim_end_matches('#').trim_end();
    if without.is_empty() || without == trimmed {
        None
    } else {
        Some(without.to_string())
    }
}

pub(crate) fn normalize_heading(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn bound_id_marker(lines: &[String], heading: usize) -> (Option<String>, Option<usize>) {
    let mut idx = heading;
    while idx > 0 {
        idx -= 1;
        if lines[idx].trim().is_empty() {
            continue;
        }
        if let Some(marker) = parse_marker(&lines[idx]) {
            if marker.kind == "id" {
                return (marker.fields.get("id").cloned(), Some(idx));
            }
        }
        break;
    }
    (None, None)
}

pub(crate) fn bound_table_marker(
    lines: &[String],
    table_start: usize,
) -> (Option<String>, Option<String>, Option<usize>) {
    let mut idx = table_start;
    while idx > 0 {
        idx -= 1;
        if lines[idx].trim().is_empty() {
            continue;
        }
        if let Some(marker) = parse_marker(&lines[idx]) {
            if marker.kind == "table" {
                return (
                    marker.fields.get("name").cloned(),
                    marker.fields.get("key").cloned(),
                    Some(idx),
                );
            }
        }
        break;
    }
    (None, None, None)
}

pub(crate) fn frontmatter_range(lines: &[String]) -> Option<(usize, usize)> {
    let first = lines.first()?.trim();
    if first != "---" && first != "+++" {
        return None;
    }
    for (idx, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == first {
            return Some((0, idx + 1));
        }
    }
    None
}
