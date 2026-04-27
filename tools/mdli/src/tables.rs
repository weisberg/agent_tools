use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::*;

pub(crate) fn run_table(cmd: TableCommand) -> Result<Outcome, MdliError> {
    match cmd {
        TableCommand::List(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            Ok(Outcome::Json(json!({"tables": index.tables})))
        }
        TableCommand::Get(args) => {
            let doc = MarkdownDocument::read(&args.file)?;
            let index = index_document(&doc);
            let table = if let Some(name) = args.name.as_deref() {
                resolve_table_by_name(&index, name)?
            } else {
                let section_sel = args.section.as_deref().ok_or_else(|| {
                    MdliError::user("E_SELECTOR_REQUIRED", "--section or --name is required")
                })?;
                let section = resolve_section(&index, section_sel)?;
                let matches = index
                    .tables
                    .iter()
                    .filter(|t| t.start >= section.heading && t.end <= section.end)
                    .cloned()
                    .collect::<Vec<_>>();
                one_match(
                    matches,
                    "E_SELECTOR_NOT_FOUND",
                    "no table in selected section",
                    |t| {
                        json!({
                            "name": t.name,
                            "section_id": t.section_id,
                            "section_path": t.section_path,
                            "line": t.line,
                        })
                    },
                )?
            };
            Ok(Outcome::Text(
                doc.lines[table.start..table.end].join(&doc.line_ending),
            ))
        }
        TableCommand::Replace(args) => table_replace(args),
        TableCommand::Upsert(args) => table_upsert(args),
        TableCommand::DeleteRow(args) => table_delete_row(args),
        TableCommand::Sort(args) => table_sort(args),
        TableCommand::Fmt(args) => table_fmt(args),
    }
}

pub(crate) fn table_replace(args: TableReplaceArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let columns = parse_columns(&args.columns)?;
    let rows = read_rows(&args.from_rows)?;
    let render_options = RenderTableOptions {
        columns,
        key: args.key.clone(),
        sort: args.sort.clone(),
        missing: args.missing,
        rich_cell: args.on_rich_cell,
        duplicate_key: args.on_duplicate_key,
        empty: args.empty,
        links: parse_assignment_map(&args.links)?,
        truncates: parse_usize_map(&args.truncates)?,
        escape_markdown: args.escape_markdown,
    };
    let rendered = render_table_from_rows(&rows, &render_options)?;
    let index = index_document(&doc);
    let section = resolve_section(&index, &args.section)?;
    let existing = args
        .name
        .as_deref()
        .and_then(|name| {
            index
                .tables
                .iter()
                .find(|t| t.name.as_deref() == Some(name))
                .cloned()
        })
        .or_else(|| {
            index
                .tables
                .iter()
                .find(|t| t.start >= section.heading && t.end <= section.end)
                .cloned()
        });
    let mut replacement = Vec::new();
    if let Some(name) = &args.name {
        replacement.push(table_marker(name, args.key.as_deref()));
    }
    replacement.extend(rendered.lines);
    if let Some(table) = existing {
        doc.lines.splice(table.start..table.end, replacement);
    } else {
        let insert_at = section.end;
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
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({
            "op": "replace_table",
            "table": args.name,
            "rows_after": rendered.row_count
        })],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn table_upsert(args: TableUpsertArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let index = index_document(&doc);
    let table = resolve_table_by_name(&index, &args.name)?;
    let mut data =
        table_data_from_lines(&doc.lines[table.start..table.end], table.marker.is_some())?;
    let incoming = if let Some(path) = args.from_rows {
        read_rows(&path)?
    } else if !args.rows.is_empty() {
        vec![row_from_kv(&args.rows)?]
    } else {
        return Err(MdliError::user(
            "E_ROW_INPUT_INVALID",
            "--row or --from-rows is required",
        ));
    };
    let key_idx = data
        .columns
        .iter()
        .position(|c| c == &args.key)
        .ok_or_else(|| MdliError::user("E_TABLE_KEY_MISSING", "key column missing from table"))?;
    let mut by_key: BTreeMap<String, Vec<String>> = data
        .rows
        .into_iter()
        .map(|row| (row.get(key_idx).cloned().unwrap_or_default(), row))
        .collect();
    for row in incoming {
        let mut rendered = Vec::new();
        for col in &data.columns {
            rendered.push(scalar_to_cell(
                row.get(col).unwrap_or(&Value::Null),
                &RichCellMode::Error,
                false,
            )?);
        }
        let key_value = rendered.get(key_idx).cloned().unwrap_or_default();
        by_key.insert(key_value, rendered);
    }
    data.rows = by_key.into_values().collect();
    let rendered = render_existing_table(&data, table.name.as_deref(), table.key.as_ref());
    doc.lines.splice(table.start..table.end, rendered);
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": "upsert_table", "table": args.name})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn table_delete_row(args: TableDeleteRowArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let index = index_document(&doc);
    let table = resolve_table_by_name(&index, &args.name)?;
    let mut data =
        table_data_from_lines(&doc.lines[table.start..table.end], table.marker.is_some())?;
    let key_idx = data
        .columns
        .iter()
        .position(|c| c == &args.key)
        .ok_or_else(|| MdliError::user("E_TABLE_KEY_MISSING", "key column missing from table"))?;
    let before_rows = data.rows.len();
    data.rows
        .retain(|row| row.get(key_idx).map(|v| v != &args.value).unwrap_or(true));
    let rendered = render_existing_table(&data, table.name.as_deref(), table.key.as_ref());
    doc.lines.splice(table.start..table.end, rendered);
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({
            "op": "delete_table_row",
            "table": args.name,
            "rows_removed": before_rows.saturating_sub(data.rows.len())
        })],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn table_sort(args: TableSortArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let index = index_document(&doc);
    let table = resolve_table_by_name(&index, &args.name)?;
    let mut data =
        table_data_from_lines(&doc.lines[table.start..table.end], table.marker.is_some())?;
    sort_rows(&mut data.rows, &data.columns, &args.by)?;
    let rendered = render_existing_table(&data, table.name.as_deref(), table.key.as_ref());
    doc.lines.splice(table.start..table.end, rendered);
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": "sort_table", "table": args.name})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

pub(crate) fn table_fmt(args: TableFmtArgs) -> Result<Outcome, MdliError> {
    let mut doc = MarkdownDocument::read(&args.file)?;
    doc.assert_preimage(&args.mutate.preimage_hash)?;
    validate_write_emit(&args.mutate)?;
    let before = doc.render();
    let index = index_document(&doc);
    let targets = if args.all {
        index.tables.clone()
    } else {
        let name = args
            .name
            .as_deref()
            .ok_or_else(|| MdliError::user("E_SELECTOR_REQUIRED", "--all or --name is required"))?;
        vec![resolve_table_by_name(&index, name)?]
    };
    for table in targets.into_iter().rev() {
        let data =
            table_data_from_lines(&doc.lines[table.start..table.end], table.marker.is_some())?;
        let rendered = render_existing_table(&data, table.name.as_deref(), table.key.as_ref());
        doc.lines.splice(table.start..table.end, rendered);
    }
    let changed = before != doc.render();
    Ok(Outcome::Mutated(MutationOutcome {
        document: doc,
        changed,
        ops: vec![json!({"op": "fmt_table"})],
        warnings: Vec::new(),
        flags: args.mutate,
    }))
}

#[derive(Debug)]
pub(crate) struct TableData {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<String>>,
}

#[derive(Debug)]
pub(crate) struct RenderedTable {
    pub(crate) lines: Vec<String>,
    pub(crate) row_count: usize,
}

#[derive(Debug)]
pub(crate) struct RenderTableOptions {
    pub(crate) columns: Vec<(String, String)>,
    pub(crate) key: Option<String>,
    pub(crate) sort: Option<String>,
    pub(crate) missing: MissingMode,
    pub(crate) rich_cell: RichCellMode,
    pub(crate) duplicate_key: DuplicateKeyMode,
    pub(crate) empty: Option<String>,
    pub(crate) links: BTreeMap<String, String>,
    pub(crate) truncates: BTreeMap<String, usize>,
    pub(crate) escape_markdown: bool,
}

pub(crate) fn render_table_from_rows(
    rows: &[Map<String, Value>],
    opts: &RenderTableOptions,
) -> Result<RenderedTable, MdliError> {
    let mut rendered_rows = Vec::new();
    let mut key_positions = BTreeMap::new();
    for row in rows {
        let mut cells = Vec::new();
        for (header, path) in &opts.columns {
            let value = get_dotted(row, path);
            if value.is_none() && matches!(opts.missing, MissingMode::Error) {
                return Err(MdliError::user(
                    "E_ROW_INPUT_INVALID",
                    format!("missing field {path} for column {header}"),
                ));
            }
            let mut cell = scalar_to_cell(
                value.unwrap_or(&Value::Null),
                &opts.rich_cell,
                opts.escape_markdown,
            )?;
            if let Some(max) = opts.truncates.get(header) {
                cell = truncate_chars(&cell, *max);
            }
            if let Some(pattern) = opts.links.get(header) {
                let url = expand_link(pattern, row)?;
                cell = format!("[{cell}]({url})");
            }
            cells.push(cell);
        }
        if let Some(key) = &opts.key {
            let key_idx = opts
                .columns
                .iter()
                .position(|(header, _)| header == key)
                .ok_or_else(|| {
                    MdliError::user(
                        "E_TABLE_KEY_MISSING",
                        format!("key column {key} is not in --columns"),
                    )
                })?;
            let key_value = cells.get(key_idx).cloned().unwrap_or_default();
            if let Some(existing_idx) = key_positions.get(&key_value).copied() {
                match opts.duplicate_key {
                    DuplicateKeyMode::Error => {
                        return Err(MdliError::user(
                            "E_TABLE_DUPLICATE_KEY",
                            format!("duplicate key {key_value}"),
                        ));
                    }
                    DuplicateKeyMode::First => continue,
                    DuplicateKeyMode::Last => {
                        rendered_rows[existing_idx] = cells.clone();
                        continue;
                    }
                }
            } else {
                key_positions.insert(key_value, rendered_rows.len());
            }
        }
        rendered_rows.push(cells);
    }
    if let Some(sort) = &opts.sort {
        let columns = opts
            .columns
            .iter()
            .map(|(h, _)| h.clone())
            .collect::<Vec<_>>();
        sort_rows(&mut rendered_rows, &columns, sort)?;
    }
    if rendered_rows.is_empty() {
        if let Some(empty) = &opts.empty {
            return Ok(RenderedTable {
                lines: vec![format!("_{}_", escape_markdown_text(empty))],
                row_count: 0,
            });
        }
    }
    let data = TableData {
        columns: opts.columns.iter().map(|(h, _)| h.clone()).collect(),
        rows: rendered_rows,
    };
    Ok(RenderedTable {
        row_count: data.rows.len(),
        lines: render_table_lines(&data),
    })
}

pub(crate) fn scalar_to_cell(
    value: &Value,
    rich_mode: &RichCellMode,
    escape_md: bool,
) -> Result<String, MdliError> {
    let raw = match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) if v.contains('\n') => match rich_mode {
            RichCellMode::Error => {
                return Err(MdliError::user(
                    "E_RICH_CELL",
                    "multiline table cell encountered",
                ))
            }
            RichCellMode::Json => serde_json::to_string(v).unwrap_or_default(),
            RichCellMode::Truncate => v.split_whitespace().collect::<Vec<_>>().join(" "),
            RichCellMode::Html => v.replace('\n', "<br>"),
        },
        Value::String(v) => v.trim().to_string(),
        Value::Array(_) | Value::Object(_) => match rich_mode {
            RichCellMode::Error => {
                return Err(MdliError::user(
                    "E_RICH_CELL",
                    "object or array table cell encountered",
                ))
            }
            RichCellMode::Json => serde_json::to_string(value).unwrap_or_default(),
            RichCellMode::Truncate => {
                truncate_chars(&serde_json::to_string(value).unwrap_or_default(), 80)
            }
            RichCellMode::Html => serde_json::to_string(value).unwrap_or_default(),
        },
    };
    if escape_md {
        Ok(escape_markdown_text(&raw))
    } else {
        Ok(raw)
    }
}

pub(crate) fn escape_markdown_text(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if matches!(c, '*' | '_' | '[' | ']' | '`') {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let take = max.saturating_sub(1);
    format!("{}…", s.chars().take(take).collect::<String>())
}

pub(crate) fn expand_link(pattern: &str, row: &Map<String, Value>) -> Result<String, MdliError> {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut key = String::new();
            for next in chars.by_ref() {
                if next == '}' {
                    break;
                }
                key.push(next);
            }
            let value = get_dotted(row, &key).ok_or_else(|| {
                MdliError::user(
                    "E_ROW_INPUT_INVALID",
                    format!("missing link placeholder {key}"),
                )
            })?;
            out.push_str(&scalar_to_cell(value, &RichCellMode::Json, false)?);
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

pub(crate) fn render_existing_table(
    data: &TableData,
    name: Option<&str>,
    key: Option<&String>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(name) = name {
        lines.push(table_marker(name, key.map(String::as_str)));
    }
    lines.extend(render_table_lines(data));
    lines
}

pub(crate) fn render_table_lines(data: &TableData) -> Vec<String> {
    let mut widths = data
        .columns
        .iter()
        .map(|c| rendered_width(c))
        .collect::<Vec<_>>();
    for row in &data.rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx >= widths.len() {
                widths.push(0);
            }
            widths[idx] = widths[idx].max(rendered_width(cell));
        }
    }
    let mut lines = Vec::new();
    lines.push(render_table_row(&data.columns, &widths));
    lines.push(format!(
        "|{}|",
        widths
            .iter()
            .map(|w| format!(" {} ", "-".repeat((*w).max(3))))
            .collect::<Vec<_>>()
            .join("|")
    ));
    for row in &data.rows {
        lines.push(render_table_row(row, &widths));
    }
    lines
}

pub(crate) fn render_table_row(cells: &[String], widths: &[usize]) -> String {
    format!(
        "|{}|",
        cells
            .iter()
            .enumerate()
            .map(|(idx, cell)| {
                let escaped = escape_pipe_for_render(cell);
                format!(
                    " {:width$} ",
                    escaped,
                    width = widths.get(idx).copied().unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    )
}

fn escape_pipe_for_render(cell: &str) -> String {
    // Re-escape literal pipes that survived split_table_row's unescape pass.
    // We intentionally do not double-escape `\|` (already escaped).
    let mut out = String::with_capacity(cell.len());
    let mut prev_backslash = false;
    for c in cell.chars() {
        if c == '|' && !prev_backslash {
            out.push('\\');
        }
        out.push(c);
        prev_backslash = c == '\\' && !prev_backslash;
    }
    out
}

pub(crate) fn visible_width(s: &str) -> usize {
    s.chars().count()
}

pub(crate) fn rendered_width(s: &str) -> usize {
    visible_width(&escape_pipe_for_render(s))
}

pub(crate) fn table_data_from_lines(
    lines: &[String],
    _has_marker: bool,
) -> Result<TableData, MdliError> {
    let offset = lines
        .iter()
        .position(|line| is_table_header(line))
        .ok_or_else(|| MdliError::user("E_TABLE_INVALID", "table header missing"))?;
    if lines.len() < offset + 2 {
        return Err(MdliError::user("E_TABLE_INVALID", "table is incomplete"));
    }
    if !is_table_separator(&lines[offset + 1]) {
        return Err(MdliError::user(
            "E_TABLE_INVALID",
            "table separator missing",
        ));
    }
    let columns = split_table_row(&lines[offset]);
    let rows = lines
        .iter()
        .skip(offset + 2)
        .filter(|line| is_table_row(line))
        .map(|line| split_table_row(line))
        .collect::<Vec<_>>();
    Ok(TableData { columns, rows })
}

pub(crate) fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim().trim_matches('|');
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in trimmed.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            current.push(c);
            escaped = true;
        } else if c == '|' {
            cells.push(current.trim().replace("\\|", "|"));
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().replace("\\|", "|"));
    cells
}

pub(crate) fn is_table_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

pub(crate) fn is_table_separator(line: &str) -> bool {
    let cells = split_table_row(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let c = cell.trim();
            c.len() >= 3 && c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
        })
}

pub(crate) fn is_table_row(line: &str) -> bool {
    is_table_header(line)
}

pub(crate) fn parse_columns(spec: &str) -> Result<Vec<(String, String)>, MdliError> {
    let mut cols = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (header, path) = part
            .split_once('=')
            .map(|(h, p)| (h.trim(), p.trim()))
            .unwrap_or((part, part));
        if header.is_empty() || path.is_empty() {
            return Err(MdliError::user(
                "E_ROW_INPUT_INVALID",
                "invalid --columns entry",
            ));
        }
        cols.push((header.to_string(), path.to_string()));
    }
    if cols.is_empty() {
        return Err(MdliError::user(
            "E_ROW_INPUT_INVALID",
            "--columns cannot be empty",
        ));
    }
    Ok(cols)
}

pub(crate) fn read_rows(path: &Path) -> Result<Vec<Map<String, Value>>, MdliError> {
    let text = read_text_path(path)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        let values: Vec<Value> = serde_json::from_str(trimmed).map_err(|e| {
            MdliError::user("E_ROW_INPUT_INVALID", format!("invalid JSON array: {e}"))
        })?;
        values
            .into_iter()
            .map(value_to_row)
            .collect::<Result<Vec<_>, _>>()
    } else {
        trimmed
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(idx, line)| {
                let value: Value = serde_json::from_str(line).map_err(|e| {
                    MdliError::user(
                        "E_ROW_INPUT_INVALID",
                        format!("invalid NDJSON at line {}: {e}", idx + 1),
                    )
                })?;
                value_to_row(value)
            })
            .collect()
    }
}

pub(crate) fn value_to_row(value: Value) -> Result<Map<String, Value>, MdliError> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(MdliError::user(
            "E_ROW_INPUT_INVALID",
            "each row must be a JSON object",
        )),
    }
}

pub(crate) fn row_from_kv(entries: &[String]) -> Result<Map<String, Value>, MdliError> {
    let mut map = Map::new();
    for entry in entries {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| MdliError::user("E_ROW_INPUT_INVALID", "--row entries must be K=V"))?;
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
    Ok(map)
}

pub(crate) fn get_dotted<'a>(row: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let first = parts.next()?;
    let mut current = row.get(first)?;
    for part in parts {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

pub(crate) fn sort_rows(
    rows: &mut [Vec<String>],
    columns: &[String],
    spec: &str,
) -> Result<(), MdliError> {
    let keys = spec
        .split(',')
        .map(|part| {
            let (name, dir) = part.trim().split_once(':').unwrap_or((part.trim(), "asc"));
            let idx = columns.iter().position(|c| c == name).ok_or_else(|| {
                MdliError::user("E_TABLE_KEY_MISSING", format!("sort column {name} missing"))
            })?;
            Ok((idx, dir == "desc"))
        })
        .collect::<Result<Vec<_>, MdliError>>()?;
    rows.sort_by(|a, b| {
        for (idx, desc) in &keys {
            let ord = a.get(*idx).cmp(&b.get(*idx));
            if !ord.is_eq() {
                return if *desc { ord.reverse() } else { ord };
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(())
}

pub(crate) fn parse_assignment_map(
    entries: &[String],
) -> Result<BTreeMap<String, String>, MdliError> {
    let mut map = BTreeMap::new();
    for entry in entries {
        let (key, value) = entry.split_once('=').ok_or_else(|| {
            MdliError::user("E_ROW_INPUT_INVALID", "assignment must be KEY=VALUE")
        })?;
        map.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    Ok(map)
}

pub(crate) fn parse_usize_map(entries: &[String]) -> Result<BTreeMap<String, usize>, MdliError> {
    let mut map = BTreeMap::new();
    for entry in entries {
        let (key, value) = entry.split_once('=').ok_or_else(|| {
            MdliError::user("E_ROW_INPUT_INVALID", "assignment must be KEY=VALUE")
        })?;
        map.insert(
            key.to_string(),
            value.parse().map_err(|_| {
                MdliError::user("E_ROW_INPUT_INVALID", "truncate value must be a number")
            })?,
        );
    }
    Ok(map)
}
