// Excel HTML editor — modify individual cells in clipboard HTML by A1 reference.
//
// Reads existing Excel HTML from the clipboard, applies cell-level edits
// (value, style, formula), and writes the result back.

use regex::Regex;

// ---------------------------------------------------------------------------
// Cell reference parsing (A1-style)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CellRef {
    pub col: usize, // 0-based
    pub row: usize, // 0-based (0 = first row in HTML, including header)
}

/// Parse "A1" → (col=0, row=0), "B2" → (col=1, row=1), "AA5" → (col=26, row=4)
pub fn parse_cell_ref(s: &str) -> Result<CellRef, String> {
    let s = s.trim();
    let mut col_part = String::new();
    let mut row_part = String::new();

    for ch in s.chars() {
        if ch.is_ascii_alphabetic() {
            col_part.push(ch.to_ascii_uppercase());
        } else if ch.is_ascii_digit() {
            row_part.push(ch);
        } else {
            return Err(format!("invalid cell reference: '{s}'"));
        }
    }

    if col_part.is_empty() || row_part.is_empty() {
        return Err(format!("invalid cell reference: '{s}'"));
    }

    let col = col_letter_to_index(&col_part)?;
    let row: usize = row_part
        .parse::<usize>()
        .map_err(|_| format!("invalid row number in '{s}'"))?;
    if row == 0 {
        return Err("row numbers start at 1".to_string());
    }

    Ok(CellRef { col, row: row - 1 })
}

fn col_letter_to_index(letters: &str) -> Result<usize, String> {
    let mut idx = 0usize;
    for ch in letters.chars() {
        idx = idx * 26 + (ch as usize - 'A' as usize + 1);
    }
    Ok(idx - 1)
}

// ---------------------------------------------------------------------------
// Edit operations
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct EditOp {
    pub cell: CellRef,
    pub kind: EditKind,
}

#[derive(Debug)]
pub enum EditKind {
    SetValue(String),
    SetBold,
    SetItalic,
    #[allow(dead_code)]
    ClearBold,
    #[allow(dead_code)]
    ClearItalic,
    SetFgColor(String),
    SetBgColor(String),
    SetNumberFormat(String),
    SetFormula(String),
    SetAlignment(String),
    SetWrap,
}

// ---------------------------------------------------------------------------
// HTML modification
// ---------------------------------------------------------------------------

/// Apply edits to Excel HTML. Returns the modified HTML.
pub fn apply_edits(html: &str, edits: &[EditOp]) -> String {
    if edits.is_empty() {
        return html.to_string();
    }

    let mut cells = find_all_cells(html);

    for edit in edits {
        if let Some(cell) = cells
            .iter_mut()
            .find(|c| c.row == edit.cell.row && c.col == edit.cell.col)
        {
            apply_edit_to_cell(cell, &edit.kind);
        } else {
            eprintln!(
                "warning: cell row={} col={} not found in clipboard HTML",
                edit.cell.row + 1,
                edit.cell.col
            );
        }
    }

    rebuild_html(html, &cells)
}

#[derive(Debug)]
struct CellInfo {
    row: usize,
    col: usize,
    start: usize,
    end: usize,
    style: String,
    has_fmla: bool,
    fmla_value: String,
    new_value: Option<String>,
}

fn find_all_cells(html: &str) -> Vec<CellInfo> {
    let mut cells = Vec::new();
    let tr_re = Regex::new(r"(?i)<tr[^>]*>").unwrap();
    let td_re = Regex::new(r"(?is)<td([^>]*)>(.*?)</td>").unwrap();

    let mut row_idx = 0usize;

    for tr_match in tr_re.find_iter(html) {
        let tr_start = tr_match.start();
        let tr_end_tag = match html[tr_start..].find("</tr>") {
            Some(offset) => tr_start + offset + 5,
            None => continue,
        };
        let tr_content = &html[tr_start..tr_end_tag];

        let mut col_idx = 0usize;
        for td_match in td_re.captures_iter(tr_content) {
            let full = td_match.get(0).unwrap();
            let attrs = td_match.get(1).unwrap().as_str();

            let abs_start = tr_start + full.start();
            let abs_end = tr_start + full.end();

            let style = extract_attr(attrs, "style").unwrap_or_default();
            let fmla = extract_attr(attrs, "x:fmla");

            cells.push(CellInfo {
                row: row_idx,
                col: col_idx,
                start: abs_start,
                end: abs_end,
                style,
                has_fmla: fmla.is_some(),
                fmla_value: fmla.unwrap_or_default(),
                new_value: None,
            });
            col_idx += 1;
        }
        row_idx += 1;
    }

    cells
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    // Find name= then determine the quote character and match to the SAME closing quote.
    // Must handle style values containing embedded quotes (e.g. mso-number-format:"\0022...").
    let prefix = format!("{}=", name);
    let start = attrs.find(&prefix)?;
    let after_eq = start + prefix.len();
    let bytes = attrs.as_bytes();
    if after_eq >= bytes.len() {
        return None;
    }
    let quote = bytes[after_eq] as char;
    if quote != '\'' && quote != '"' {
        return None;
    }
    // For style attributes that use single quotes, the value can contain double quotes freely.
    // Find the matching closing quote by scanning for the same quote character.
    let value_start = after_eq + 1;
    let rest = &attrs[value_start..];
    // The closing quote must be the same type as the opening one
    let end = if quote == '\'' {
        // Single-quoted: find next unescaped single quote
        // In practice, style values don't have escaped single quotes,
        // so just find the next '
        rest.find('\'')?
    } else {
        rest.find('"')?
    };
    Some(rest[..end].to_string())
}

fn apply_edit_to_cell(cell: &mut CellInfo, kind: &EditKind) {
    match kind {
        EditKind::SetBold => {
            set_style_prop(&mut cell.style, "font-weight", "700");
        }
        EditKind::ClearBold => {
            remove_style_prop(&mut cell.style, "font-weight");
        }
        EditKind::SetItalic => {
            set_style_prop(&mut cell.style, "font-style", "italic");
        }
        EditKind::ClearItalic => {
            remove_style_prop(&mut cell.style, "font-style");
        }
        EditKind::SetFgColor(color) => {
            set_style_prop(&mut cell.style, "color", color);
        }
        EditKind::SetBgColor(color) => {
            set_style_prop(&mut cell.style, "background", color);
            set_style_prop(&mut cell.style, "mso-pattern", &format!("{color} none"));
        }
        EditKind::SetNumberFormat(fmt) => {
            let css = crate::excel::number_format_css_owned(fmt);
            if let Some(val) = css
                .strip_prefix("mso-number-format:")
                .and_then(|v| Some(v.trim_end_matches(';')))
            {
                set_style_prop(&mut cell.style, "mso-number-format", val);
            }
        }
        EditKind::SetAlignment(align) => {
            set_style_prop(&mut cell.style, "text-align", align);
        }
        EditKind::SetWrap => {
            set_style_prop(&mut cell.style, "white-space", "normal");
        }
        EditKind::SetFormula(formula) => {
            cell.has_fmla = true;
            cell.fmla_value = formula.clone();
        }
        EditKind::SetValue(value) => {
            cell.new_value = Some(value.clone());
        }
    }
}

fn set_style_prop(style: &mut String, prop: &str, value: &str) {
    let re = Regex::new(&format!(r"(?i){}\s*:\s*[^;]*;?", regex::escape(prop))).unwrap();
    let cleaned = re.replace_all(style, "").to_string();
    let trimmed = cleaned.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        *style = format!("{prop}:{value};");
    } else {
        *style = format!("{trimmed};{prop}:{value};");
    }
}

fn remove_style_prop(style: &mut String, prop: &str) {
    let re = Regex::new(&format!(r"(?i){}\s*:\s*[^;]*;?", regex::escape(prop))).unwrap();
    *style = re.replace_all(style, "").trim().to_string();
}

fn rebuild_html(original: &str, cells: &[CellInfo]) -> String {
    let mut sorted: Vec<&CellInfo> = cells.iter().collect();
    sorted.sort_by(|a, b| b.start.cmp(&a.start));

    let mut result = original.to_string();

    for cell in &sorted {
        let original_td = &original[cell.start..cell.end];
        let new_td = rebuild_td(original_td, cell);
        result.replace_range(cell.start..cell.end, &new_td);
    }

    result
}

fn rebuild_td(original_td: &str, cell: &CellInfo) -> String {
    let td_re = Regex::new(r"(?is)<td([^>]*)>(.*)</td>").unwrap();
    let caps = match td_re.captures(original_td) {
        Some(c) => c,
        None => return original_td.to_string(),
    };

    let original_attrs = caps.get(1).unwrap().as_str();
    let original_content = caps.get(2).unwrap().as_str();

    // Update style attribute
    let mut new_attrs = update_attr(original_attrs, "style", &cell.style);

    // Handle formula
    if cell.has_fmla {
        let escaped = cell
            .fmla_value
            .replace('&', "&amp;")
            .replace('"', "&quot;");
        new_attrs = add_or_update_attr(&new_attrs, "x:fmla", &escaped);
        if !new_attrs.contains("x:num") {
            new_attrs = format!("{new_attrs} x:num");
        }
    }

    // Sync align= attribute with text-align
    let align_re = Regex::new(r"text-align:\s*([^;]+)").unwrap();
    if let Some(caps) = align_re.captures(&cell.style) {
        let align_val = caps[1].trim();
        if align_val != "general" {
            new_attrs = add_or_update_attr(&new_attrs, "align", align_val);
        }
    }

    // Content: use new value if set, otherwise keep original
    let content = cell
        .new_value
        .as_deref()
        .unwrap_or(original_content);

    format!("<td{new_attrs}>{content}</td>")
}

fn update_attr(attrs: &str, name: &str, value: &str) -> String {
    // Find the existing attribute and replace its value, preserving quote type
    let prefix = format!("{name}=");
    if let Some(start) = attrs.find(&prefix) {
        let after_eq = start + prefix.len();
        let bytes = attrs.as_bytes();
        if after_eq < bytes.len() {
            let quote = bytes[after_eq] as char;
            if quote == '\'' || quote == '"' {
                let value_start = after_eq + 1;
                let rest = &attrs[value_start..];
                if let Some(end) = rest.find(quote) {
                    let before = &attrs[..start];
                    let after = &attrs[value_start + end + 1..];
                    return format!("{before}{name}={quote}{value}{quote}{after}");
                }
            }
        }
    }
    // Attribute doesn't exist — add it
    format!("{attrs} {name}='{value}'")
}

fn add_or_update_attr(attrs: &str, name: &str, value: &str) -> String {
    let re = Regex::new(&format!(r#"{}="[^"]*""#, regex::escape(name))).unwrap();
    if re.is_match(attrs) {
        re.replace(attrs, &format!("{name}=\"{value}\"")).to_string()
    } else {
        format!("{attrs} {name}=\"{value}\"")
    }
}

// ---------------------------------------------------------------------------
// CLI parsing helpers
// ---------------------------------------------------------------------------

pub fn parse_set_value(spec: &str) -> Result<EditOp, String> {
    let (cell_str, value) = split_cell_spec(spec)?;
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetValue(value),
    })
}

pub fn parse_set_bg(spec: &str) -> Result<EditOp, String> {
    let (cell_str, value) = split_cell_spec(spec)?;
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetBgColor(value),
    })
}

pub fn parse_set_fg(spec: &str) -> Result<EditOp, String> {
    let (cell_str, value) = split_cell_spec(spec)?;
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetFgColor(value),
    })
}

pub fn parse_set_format(spec: &str) -> Result<EditOp, String> {
    let (cell_str, value) = split_cell_spec(spec)?;
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetNumberFormat(value),
    })
}

pub fn parse_set_formula(spec: &str) -> Result<EditOp, String> {
    let (cell_str, value) = split_cell_spec(spec)?;
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetFormula(value),
    })
}

pub fn parse_set_align(spec: &str) -> Result<EditOp, String> {
    let (cell_str, value) = split_cell_spec(spec)?;
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetAlignment(value),
    })
}

pub fn parse_set_bold(cell_str: &str) -> Result<EditOp, String> {
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetBold,
    })
}

pub fn parse_set_italic(cell_str: &str) -> Result<EditOp, String> {
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetItalic,
    })
}

pub fn parse_set_wrap(cell_str: &str) -> Result<EditOp, String> {
    Ok(EditOp {
        cell: parse_cell_ref(cell_str)?,
        kind: EditKind::SetWrap,
    })
}

fn split_cell_spec(spec: &str) -> Result<(&str, String), String> {
    let pos = spec.find(':').ok_or_else(|| {
        format!("invalid spec '{spec}': expected CELL:VALUE")
    })?;
    Ok((&spec[..pos], spec[pos + 1..].to_string()))
}
