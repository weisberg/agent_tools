use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use umya_spreadsheet::{self, NumberingFormat, SheetStateValues, Spreadsheet, Style};
use xli_core::{
    col_to_letter, parse_address, parse_range, BatchOp, SheetAction, StyleSpec, XliError,
};

use crate::WorkbookPatcher;

pub const UMYA_FALLBACK_WARNING: &str =
    "Used umya-spreadsheet fallback for workbook mutation. Some workbook artifacts may have been modified.";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct BatchSummary {
    pub ops_executed: usize,
    pub cells_written: usize,
    pub formulas_written: usize,
    pub cells_formatted: usize,
}

/// Typed return from apply_write so callers cannot accidentally ignore the
/// needs_recalc signal. (Issue #22)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WriteResult {
    pub needs_recalc: bool,
    pub used_fallback: bool,
}

pub fn apply_write(
    src: &Path,
    dst: &Path,
    address: &str,
    value: Option<Value>,
    formula: Option<String>,
) -> Result<WriteResult, XliError> {
    if formula.is_none() {
        return patch_write_value(src, dst, address, value).map(|()| WriteResult {
            needs_recalc: false,
            used_fallback: false,
        });
    }

    mutate_workbook(src, dst, |book| {
        let needs_recalc = write_into_book(book, address, value, formula)?;
        Ok(WriteResult {
            needs_recalc,
            used_fallback: true,
        })
    })
}

fn patch_write_value(
    src: &Path,
    dst: &Path,
    address: &str,
    value: Option<Value>,
) -> Result<(), XliError> {
    let cell = parse_address(address).map_err(XliError::from)?;
    let mut patcher = WorkbookPatcher::open(src, dst)?;
    let sheet_parts = discover_sheet_parts(&mut patcher)?;
    let sheet_name = resolve_sheet_part_name(&sheet_parts, cell.sheet.as_deref())?;
    let sheet_part = sheet_parts
        .get(&sheet_name)
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: sheet_name.clone(),
        })?;
    let sheet_xml = patcher.read_part(sheet_part)?;
    let patched = patch_sheet_cell(&sheet_xml, &format!("{}{}", cell.col, cell.row), &value)?;
    patcher.patch_part_bytes(sheet_part, patched);
    patcher.finalize()
}

pub fn apply_format(
    src: &Path,
    dst: &Path,
    range: &str,
    style: &StyleSpec,
) -> Result<(), XliError> {
    mutate_workbook(src, dst, |book| {
        format_in_book(book, range, style)?;
        Ok(())
    })
}

pub fn apply_sheet_action(src: &Path, dst: &Path, action: &SheetAction) -> Result<(), XliError> {
    mutate_workbook(src, dst, |book| {
        sheet_action_in_book(book, action)?;
        Ok(())
    })
}

pub fn apply_batch(
    src: &Path,
    dst: &Path,
    ops: &[BatchOp],
) -> Result<(BatchSummary, bool), XliError> {
    mutate_workbook(src, dst, |book| {
        let mut summary = BatchSummary::default();
        let mut needs_recalc = false;

        for op in ops {
            match op {
                BatchOp::Write {
                    address,
                    value,
                    formula,
                } => {
                    let wrote_formula =
                        write_into_book(book, address, value.clone(), formula.clone())?;
                    summary.ops_executed += 1;
                    summary.cells_written += 1;
                    if wrote_formula {
                        summary.formulas_written += 1;
                        needs_recalc = true;
                    }
                }
                BatchOp::Format { range, style } => {
                    format_in_book(book, range, style)?;
                    summary.ops_executed += 1;
                    summary.cells_formatted += cells_in_range(range)? as usize;
                }
                BatchOp::Sheet { action } => {
                    sheet_action_in_book(book, action)?;
                    summary.ops_executed += 1;
                }
            }
        }

        Ok((summary, needs_recalc))
    })
}

fn discover_sheet_parts(
    patcher: &mut WorkbookPatcher,
) -> Result<HashMap<String, String>, XliError> {
    let workbook_xml = patcher.read_part("xl/workbook.xml")?;
    let workbook_rels = patcher.read_part("xl/_rels/workbook.xml.rels")?;
    let sheet_relationships = parse_workbook_sheets(&workbook_xml)?;
    let rel_targets = parse_relationship_targets(&workbook_rels, "xl/workbook.xml")?;

    let mut sheet_parts = HashMap::new();
    for (sheet_name, rel_id) in sheet_relationships {
        if let Some(sheet_path) = rel_targets.get(&rel_id) {
            sheet_parts.insert(sheet_name, sheet_path.to_owned());
        }
    }
    Ok(sheet_parts)
}

fn parse_workbook_sheets(workbook_xml: &[u8]) -> Result<Vec<(String, String)>, XliError> {
    let mut reader = Reader::from_reader(Cursor::new(workbook_xml));
    let mut buffer = Vec::new();
    let mut sheets = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                if start.name().local_name().as_ref() != b"sheet" {
                    continue;
                }

                let mut sheet_name = None;
                let mut rel_id = None;
                for attribute in start.attributes() {
                    let attribute = attribute.map_err(xml_error)?;
                    let key = normalize_xml_attr_name(&attribute)?;
                    match key.as_str() {
                        "name" => sheet_name = Some(decode_attribute(&reader, &attribute)?),
                        "id" => rel_id = Some(decode_attribute(&reader, &attribute)?),
                        _ => {}
                    }
                }
                if let (Some(name), Some(rel_id)) = (sheet_name, rel_id) {
                    sheets.push((name, rel_id));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(sheets)
}

fn parse_relationship_targets(
    xml: &[u8],
    base_part_path: &str,
) -> Result<HashMap<String, String>, XliError> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    let mut rel_targets = HashMap::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                if start.name().local_name().as_ref() != b"Relationship" {
                    continue;
                }

                let mut rel_id = None;
                let mut rel_target = None;
                for attribute in start.attributes() {
                    let attribute = attribute.map_err(xml_error)?;
                    let key = normalize_xml_attr_name(&attribute)?;
                    match key.as_str() {
                        "id" => rel_id = Some(decode_attribute(&reader, &attribute)?),
                        "target" => rel_target = Some(decode_attribute(&reader, &attribute)?),
                        _ => {}
                    }
                }
                if let (Some(rel_id), Some(rel_target)) = (rel_id, rel_target) {
                    rel_targets.insert(rel_id, resolve_ooxml_path(base_part_path, &rel_target));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(rel_targets)
}

fn resolve_ooxml_path(base_part_path: &str, target: &str) -> String {
    let mut parts = base_part_path.split('/').collect::<Vec<_>>();
    let _ = parts.pop();

    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                let _ = parts.pop();
            }
            _ => parts.push(segment),
        }
    }

    parts.join("/")
}

fn resolve_sheet_part_name(
    sheet_parts: &HashMap<String, String>,
    requested: Option<&str>,
) -> Result<String, XliError> {
    if let Some(name) = requested {
        if sheet_parts.contains_key(name) {
            return Ok(name.to_string());
        }
        return Err(XliError::SheetNotFound {
            sheet: name.to_string(),
        });
    }

    sheet_parts
        .keys()
        .next()
        .cloned()
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: "<first>".to_string(),
        })
}

fn patch_sheet_cell(
    sheet_xml: &[u8],
    cell_ref: &str,
    value: &Option<Value>,
) -> Result<Vec<u8>, XliError> {
    let target = parse_address(cell_ref).map_err(XliError::from)?;
    let mut reader = Reader::from_reader(Cursor::new(sheet_xml));
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut inserted = false;

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"sheetData" => {
                writer.write_event(Event::Start(start)).map_err(xml_error)?;
                patch_sheet_data(&mut reader, &mut writer, &target, value, &mut inserted)?;
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"sheetData" => {
                writer
                    .write_event(Event::Start(start.to_owned()))
                    .map_err(xml_error)?;
                write_row_with_cell(&mut writer, target.row, cell_ref, value, None)?;
                writer
                    .write_event(Event::End(BytesEnd::new("sheetData")))
                    .map_err(xml_error)?;
                inserted = true;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(writer.into_inner())
}

fn patch_sheet_data(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    target: &xli_core::CellRef,
    value: &Option<Value>,
    inserted: &mut bool,
) -> Result<(), XliError> {
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"row" => {
                let row_num = row_number(reader, &start)?;
                if !*inserted && row_num > target.row {
                    write_row_with_cell(writer, target.row, &target_ref(target), value, None)?;
                    *inserted = true;
                }
                if row_num == target.row {
                    patch_row(reader, writer, start, target, value)?;
                    *inserted = true;
                } else {
                    writer.write_event(Event::Start(start)).map_err(xml_error)?;
                    copy_until_end(reader, writer, b"row")?;
                }
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"row" => {
                let row_num = row_number(reader, &start)?;
                if !*inserted && row_num > target.row {
                    write_row_with_cell(writer, target.row, &target_ref(target), value, None)?;
                    *inserted = true;
                }
                if row_num == target.row {
                    write_row_with_cell(writer, target.row, &target_ref(target), value, None)?;
                    *inserted = true;
                } else {
                    writer.write_event(Event::Empty(start)).map_err(xml_error)?;
                }
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"sheetData" => {
                if !*inserted {
                    write_row_with_cell(writer, target.row, &target_ref(target), value, None)?;
                    *inserted = true;
                }
                writer.write_event(Event::End(end)).map_err(xml_error)?;
                break;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(())
}

fn patch_row(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    row_start: BytesStart<'_>,
    target: &xli_core::CellRef,
    value: &Option<Value>,
) -> Result<(), XliError> {
    writer
        .write_event(Event::Start(row_start))
        .map_err(xml_error)?;
    let mut buffer = Vec::new();
    let mut inserted = false;

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"c" => {
                let (cell_ref, style) = cell_reference_and_style(reader, &start)?;
                let cell_col = cell_ref
                    .as_deref()
                    .and_then(|value| parse_address(value).ok())
                    .map(|cell| cell.col_idx);
                if !inserted && cell_col.is_some_and(|col| col > target.col_idx) {
                    write_cell(writer, &target_ref(target), value, None)?;
                    inserted = true;
                }
                if cell_col == Some(target.col_idx) {
                    write_cell(writer, &target_ref(target), value, style.as_deref())?;
                    skip_until_end(reader, b"c")?;
                    inserted = true;
                } else {
                    writer.write_event(Event::Start(start)).map_err(xml_error)?;
                    copy_until_end(reader, writer, b"c")?;
                }
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"c" => {
                let (cell_ref, style) = cell_reference_and_style(reader, &start)?;
                let cell_col = cell_ref
                    .as_deref()
                    .and_then(|value| parse_address(value).ok())
                    .map(|cell| cell.col_idx);
                if !inserted && cell_col.is_some_and(|col| col > target.col_idx) {
                    write_cell(writer, &target_ref(target), value, None)?;
                    inserted = true;
                }
                if cell_col == Some(target.col_idx) {
                    write_cell(writer, &target_ref(target), value, style.as_deref())?;
                    inserted = true;
                } else {
                    writer.write_event(Event::Empty(start)).map_err(xml_error)?;
                }
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"row" => {
                if !inserted {
                    write_cell(writer, &target_ref(target), value, None)?;
                }
                writer.write_event(Event::End(end)).map_err(xml_error)?;
                break;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(())
}

fn write_row_with_cell(
    writer: &mut Writer<Vec<u8>>,
    row: u32,
    cell_ref: &str,
    value: &Option<Value>,
    style: Option<&str>,
) -> Result<(), XliError> {
    let mut row_start = BytesStart::new("row");
    let row_text = row.to_string();
    row_start.push_attribute(("r", row_text.as_str()));
    writer
        .write_event(Event::Start(row_start))
        .map_err(xml_error)?;
    write_cell(writer, cell_ref, value, style)?;
    writer
        .write_event(Event::End(BytesEnd::new("row")))
        .map_err(xml_error)
}

fn write_cell(
    writer: &mut Writer<Vec<u8>>,
    cell_ref: &str,
    value: &Option<Value>,
    style: Option<&str>,
) -> Result<(), XliError> {
    let mut cell = BytesStart::new("c");
    cell.push_attribute(("r", cell_ref));
    if let Some(style) = style {
        cell.push_attribute(("s", style));
    }

    match value {
        Some(Value::Null) | None => {
            writer.write_event(Event::Empty(cell)).map_err(xml_error)?;
        }
        Some(Value::Bool(value)) => {
            cell.push_attribute(("t", "b"));
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            write_text_element(writer, "v", if *value { "1" } else { "0" })?;
            writer
                .write_event(Event::End(BytesEnd::new("c")))
                .map_err(xml_error)?;
        }
        Some(Value::Number(number)) => {
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            write_text_element(writer, "v", &number.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new("c")))
                .map_err(xml_error)?;
        }
        Some(Value::String(value)) => {
            cell.push_attribute(("t", "inlineStr"));
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            writer
                .write_event(Event::Start(BytesStart::new("is")))
                .map_err(xml_error)?;
            write_text_element(writer, "t", value)?;
            writer
                .write_event(Event::End(BytesEnd::new("is")))
                .map_err(xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::new("c")))
                .map_err(xml_error)?;
        }
        Some(other) => {
            cell.push_attribute(("t", "inlineStr"));
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            writer
                .write_event(Event::Start(BytesStart::new("is")))
                .map_err(xml_error)?;
            write_text_element(writer, "t", &other.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new("is")))
                .map_err(xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::new("c")))
                .map_err(xml_error)?;
        }
    }

    Ok(())
}

fn write_text_element(
    writer: &mut Writer<Vec<u8>>,
    name: &str,
    text: &str,
) -> Result<(), XliError> {
    writer
        .write_event(Event::Start(BytesStart::new(name)))
        .map_err(xml_error)?;
    writer
        .write_event(Event::Text(BytesText::new(text)))
        .map_err(xml_error)?;
    writer
        .write_event(Event::End(BytesEnd::new(name)))
        .map_err(xml_error)
}

fn copy_until_end(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    local_name: &[u8],
) -> Result<(), XliError> {
    let mut buffer = Vec::new();
    let mut depth = 1_u32;
    while depth > 0 {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == local_name => {
                depth += 1;
                writer.write_event(Event::Start(start)).map_err(xml_error)?;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == local_name => {
                depth -= 1;
                writer.write_event(Event::End(end)).map_err(xml_error)?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }
    Ok(())
}

fn skip_until_end(reader: &mut Reader<Cursor<&[u8]>>, local_name: &[u8]) -> Result<(), XliError> {
    let mut buffer = Vec::new();
    let mut depth = 1_u32;
    while depth > 0 {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == local_name => {
                depth += 1;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == local_name => {
                depth -= 1;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(xml_error(error)),
        }
    }
    Ok(())
}

fn row_number(reader: &Reader<Cursor<&[u8]>>, row: &BytesStart<'_>) -> Result<u32, XliError> {
    for attribute in row.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        if attribute.key.local_name().as_ref() == b"r" {
            return decode_attribute(reader, &attribute)?
                .parse::<u32>()
                .map_err(xml_error);
        }
    }
    Ok(0)
}

fn cell_reference_and_style(
    reader: &Reader<Cursor<&[u8]>>,
    cell: &BytesStart<'_>,
) -> Result<(Option<String>, Option<String>), XliError> {
    let mut cell_ref = None;
    let mut style = None;
    for attribute in cell.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        match attribute.key.local_name().as_ref() {
            b"r" => cell_ref = Some(decode_attribute(reader, &attribute)?),
            b"s" => style = Some(decode_attribute(reader, &attribute)?),
            _ => {}
        }
    }
    Ok((cell_ref, style))
}

fn target_ref(target: &xli_core::CellRef) -> String {
    format!("{}{}", target.col, target.row)
}

fn normalize_xml_attr_name(
    attribute: &quick_xml::events::attributes::Attribute<'_>,
) -> Result<String, XliError> {
    std::str::from_utf8(attribute.key.local_name().as_ref())
        .map(|value| {
            value
                .rsplit(':')
                .next()
                .unwrap_or(value)
                .to_ascii_lowercase()
        })
        .map_err(xml_error)
}

fn decode_attribute(
    reader: &Reader<Cursor<&[u8]>>,
    attribute: &quick_xml::events::attributes::Attribute<'_>,
) -> Result<String, XliError> {
    attribute
        .decode_and_unescape_value(reader.decoder())
        .map(|value| value.into_owned())
        .map_err(xml_error)
}

fn xml_error<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

pub fn write_workbook(book: &Spreadsheet, dst: &Path) -> Result<(), XliError> {
    let file = std::fs::File::create(dst).map_err(|error| XliError::OoxmlCorrupt {
        details: error.to_string(),
    })?;
    umya_spreadsheet::writer::xlsx::write_writer(book, std::io::BufWriter::new(file)).map_err(
        |error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        },
    )
}

fn mutate_workbook<T, F>(src: &Path, dst: &Path, mutate: F) -> Result<T, XliError>
where
    F: FnOnce(&mut Spreadsheet) -> Result<T, XliError>,
{
    let mut book =
        umya_spreadsheet::reader::xlsx::read(src).map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })?;
    let result = mutate(&mut book)?;
    write_workbook(&book, dst)?;
    Ok(result)
}

fn write_into_book(
    book: &mut Spreadsheet,
    address: &str,
    value: Option<Value>,
    formula: Option<String>,
) -> Result<bool, XliError> {
    let cell = parse_address(address).map_err(XliError::from)?;
    let sheet_name = resolve_sheet_name(book, cell.sheet.as_deref())?;
    let worksheet =
        book.get_sheet_by_name_mut(&sheet_name)
            .ok_or_else(|| XliError::SheetNotFound {
                sheet: sheet_name.clone(),
            })?;
    let coordinate = format!("{}{}", cell.col, cell.row);
    let target = worksheet.get_cell_mut(coordinate.as_str());

    if let Some(formula) = formula {
        target.set_formula(formula);
        target.set_formula_result_default("0");
        return Ok(true);
    }

    match value {
        Some(Value::Null) | None => {
            target.set_blank();
        }
        Some(Value::Bool(value)) => {
            target.set_value_bool(value);
        }
        Some(Value::Number(number)) => {
            if let Some(value) = number.as_f64() {
                target.set_value_number(value);
            } else {
                target.set_value(number.to_string());
            }
        }
        Some(Value::String(value)) => {
            target.set_value(value);
        }
        Some(other) => {
            target.set_value(other.to_string());
        }
    }

    Ok(false)
}

fn format_in_book(book: &mut Spreadsheet, range: &str, style: &StyleSpec) -> Result<(), XliError> {
    let range_ref = parse_range(range).map_err(XliError::from)?;
    let sheet_name = resolve_sheet_name(book, range_ref.sheet.as_deref())?;
    let worksheet =
        book.get_sheet_by_name_mut(&sheet_name)
            .ok_or_else(|| XliError::SheetNotFound {
                sheet: sheet_name.clone(),
            })?;
    let plain_range = format!(
        "{}{}:{}{}",
        range_ref.start.col, range_ref.start.row, range_ref.end.col, range_ref.end.row
    );

    let mut umya_style = Style::default();
    let mut has_changes = false;
    if let Some(true) = style.bold {
        umya_style.get_font_mut().set_bold(true);
        has_changes = true;
    }
    if let Some(true) = style.italic {
        umya_style.get_font_mut().set_italic(true);
        has_changes = true;
    }
    if let Some(font_color) = style.font_color.as_ref() {
        umya_style
            .get_font_mut()
            .get_color_mut()
            .set_argb(normalize_argb(font_color));
        has_changes = true;
    }
    if let Some(fill_color) = style.fill.as_ref() {
        umya_style.set_background_color(normalize_argb(fill_color));
        has_changes = true;
    }
    if let Some(number_format) = style.number_format.as_ref() {
        let mut format = NumberingFormat::default();
        format.set_format_code(xli_core::resolve_number_format(number_format));
        umya_style.set_number_format(format);
        has_changes = true;
    }

    if has_changes {
        worksheet.set_style_by_range(&plain_range, umya_style);
    }

    if let Some(width) = style.column_width {
        for col_idx in range_ref.start.col_idx..=range_ref.end.col_idx {
            worksheet
                .get_column_dimension_mut(&col_to_letter(col_idx))
                .set_width(width);
        }
    }

    Ok(())
}

fn sheet_action_in_book(book: &mut Spreadsheet, action: &SheetAction) -> Result<(), XliError> {
    match action {
        SheetAction::Add { name, after } => {
            book.new_sheet(name).map_err(sheet_action_error)?;
            // Respect the `after` positioning parameter. umya always appends at
            // the end, so when `after` is specified we reorder immediately after
            // adding. Previously this field was silently ignored. (Issue #23)
            if let Some(after_name) = after {
                let all_names: Vec<String> = book
                    .get_sheet_collection()
                    .iter()
                    .map(|s| s.get_name().to_string())
                    .collect();
                let after_idx =
                    all_names
                        .iter()
                        .position(|n| n == after_name)
                        .ok_or_else(|| XliError::SheetNotFound {
                            sheet: after_name.clone(),
                        })?;
                // Build the new order: everything up to and including after_idx,
                // then the new sheet, then everything else (excluding the new sheet
                // which was appended at the end).
                let new_sheet_name = name.clone();
                let mut new_order: Vec<String> = all_names
                    .iter()
                    .filter(|n| n.as_str() != new_sheet_name)
                    .cloned()
                    .collect();
                new_order.insert(after_idx + 1, new_sheet_name);
                reorder_sheets(book, &new_order)?;
            }
        }
        SheetAction::Delete { name } => {
            book.remove_sheet_by_name(name)
                .map_err(sheet_action_error)?;
        }
        SheetAction::Rename { from, to } => {
            let index = find_sheet_index(book, from).ok_or_else(|| XliError::SheetNotFound {
                sheet: from.clone(),
            })?;
            book.set_sheet_name(index, to).map_err(sheet_action_error)?;
        }
        SheetAction::Copy { from, to } => {
            let worksheet = book
                .get_sheet_by_name(from)
                .ok_or_else(|| XliError::SheetNotFound {
                    sheet: from.clone(),
                })?
                .clone();
            let mut clone = worksheet;
            clone.set_name(to);
            book.add_sheet(clone).map_err(sheet_action_error)?;
        }
        SheetAction::Reorder { sheets } => reorder_sheets(book, sheets)?,
        SheetAction::Hide { name } => set_sheet_state(book, name, SheetStateValues::Hidden)?,
        SheetAction::Unhide { name } => set_sheet_state(book, name, SheetStateValues::Visible)?,
    }

    Ok(())
}

fn reorder_sheets(book: &mut Spreadsheet, order: &[String]) -> Result<(), XliError> {
    let current = book.get_sheet_collection().to_vec();
    if current.len() != order.len() {
        return Err(XliError::SpecValidationError {
            spec: "sheet reorder".to_string(),
            details: "Order must list every existing sheet exactly once".to_string(),
        });
    }

    let mut reordered = Vec::with_capacity(current.len());
    for name in order {
        let sheet = current
            .iter()
            .find(|sheet| sheet.get_name() == name)
            .ok_or_else(|| XliError::SheetNotFound {
                sheet: name.clone(),
            })?
            .clone();
        reordered.push(sheet);
    }
    let collection = book.get_sheet_collection_mut();
    collection.clear();
    collection.extend(reordered);
    Ok(())
}

fn set_sheet_state(
    book: &mut Spreadsheet,
    name: &str,
    state: SheetStateValues,
) -> Result<(), XliError> {
    let worksheet = book
        .get_sheet_by_name_mut(name)
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: name.to_string(),
        })?;
    worksheet.set_state(state);
    Ok(())
}

fn resolve_sheet_name(book: &Spreadsheet, explicit: Option<&str>) -> Result<String, XliError> {
    if let Some(sheet) = explicit {
        return Ok(sheet.to_string());
    }

    book.get_sheet(&0)
        .map(|sheet| sheet.get_name().to_string())
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: "Sheet1".to_string(),
        })
}

fn find_sheet_index(book: &Spreadsheet, name: &str) -> Option<usize> {
    book.get_sheet_collection()
        .iter()
        .enumerate()
        .find_map(|(index, sheet)| (sheet.get_name() == name).then_some(index))
}

fn sheet_action_error(error: &'static str) -> XliError {
    XliError::WriteConflict {
        target: "worksheet".to_string(),
        details: Some(error.to_string()),
    }
}

fn normalize_argb(color: &str) -> String {
    let trimmed = color.trim_start_matches('#');
    if trimmed.len() == 6 {
        format!("FF{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn cells_in_range(range: &str) -> Result<u32, XliError> {
    let range_ref = parse_range(range).map_err(XliError::from)?;
    // Use checked_sub to catch inverted ranges (end < start). Plain u32
    // subtraction panics in debug builds and silently wraps in release,
    // producing a nonsense cell count. (Issue #20)
    let width = range_ref
        .end
        .col_idx
        .checked_sub(range_ref.start.col_idx)
        .ok_or_else(|| XliError::InvalidCellAddress {
            address: range.to_string(),
        })?
        + 1;
    let height = range_ref
        .end
        .row
        .checked_sub(range_ref.start.row)
        .ok_or_else(|| XliError::InvalidCellAddress {
            address: range.to_string(),
        })?
        + 1;
    Ok(width * height)
}
