use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct BatchApplyResult {
    pub summary: BatchSummary,
    pub needs_recalc: bool,
    pub used_fallback: bool,
}

/// Typed return from apply_write so callers cannot accidentally ignore the
/// needs_recalc signal. (Issue #22)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WriteResult {
    pub needs_recalc: bool,
    pub used_fallback: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MutationResult {
    pub used_fallback: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum CellContent {
    Value(Option<Value>),
    Formula(String),
}

#[derive(Clone, Debug, Default)]
struct SheetPartMap {
    ordered: Vec<(String, String)>,
    by_name: HashMap<String, String>,
}

impl SheetPartMap {
    fn insert(&mut self, name: String, path: String) {
        self.ordered.push((name.clone(), path.clone()));
        self.by_name.insert(name, path);
    }

    fn contains_key(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<&String> {
        self.by_name.get(name)
    }

    fn first_name(&self) -> Option<String> {
        self.ordered.first().map(|(name, _)| name.clone())
    }
}

pub fn apply_write(
    src: &Path,
    dst: &Path,
    address: &str,
    value: Option<Value>,
    formula: Option<String>,
) -> Result<WriteResult, XliError> {
    if let Some(formula) = formula {
        return patch_write_cell(src, dst, address, CellContent::Formula(formula)).map(|()| {
            WriteResult {
                needs_recalc: true,
                used_fallback: false,
            }
        });
    }

    patch_write_cell(src, dst, address, CellContent::Value(value)).map(|()| WriteResult {
        needs_recalc: false,
        used_fallback: false,
    })
}

fn patch_write_cell(
    src: &Path,
    dst: &Path,
    address: &str,
    content: CellContent,
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
    let needs_recalc = matches!(content, CellContent::Formula(_));
    let patched = patch_sheet_cell(&sheet_xml, &format!("{}{}", cell.col, cell.row), &content)?;
    patcher.patch_part_bytes(sheet_part, patched);
    if needs_recalc {
        mark_workbook_for_recalc(&mut patcher)?;
    }
    patcher.finalize()
}

fn mark_workbook_for_recalc(patcher: &mut WorkbookPatcher) -> Result<(), XliError> {
    let workbook_xml = patcher.read_part("xl/workbook.xml")?;
    let mut reader = Reader::from_reader(Cursor::new(workbook_xml.as_slice()));
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut saw_calc_pr = false;

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"calcPr" => {
                write_recalc_calc_pr(&mut writer)?;
                skip_until_end(&mut reader, b"calcPr")?;
                saw_calc_pr = true;
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"calcPr" => {
                write_recalc_calc_pr(&mut writer)?;
                saw_calc_pr = true;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"workbook" => {
                if !saw_calc_pr {
                    write_recalc_calc_pr(&mut writer)?;
                }
                writer.write_event(Event::End(end)).map_err(xml_error)?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    patcher.patch_part_bytes("xl/workbook.xml", writer.into_inner());
    Ok(())
}

fn write_recalc_calc_pr(writer: &mut Writer<Vec<u8>>) -> Result<(), XliError> {
    let mut calc_pr = BytesStart::new("calcPr");
    calc_pr.push_attribute(("calcMode", "auto"));
    calc_pr.push_attribute(("fullCalcOnLoad", "1"));
    calc_pr.push_attribute(("forceFullCalc", "1"));
    writer.write_event(Event::Empty(calc_pr)).map_err(xml_error)
}

pub fn apply_format(
    src: &Path,
    dst: &Path,
    range: &str,
    style: &StyleSpec,
) -> Result<MutationResult, XliError> {
    if style.horizontal_align.is_some() || style.vertical_align.is_some() {
        return mutate_workbook(src, dst, |book| {
            format_in_book(book, range, style)?;
            Ok(MutationResult {
                used_fallback: true,
            })
        });
    }

    patch_format(src, dst, range, style).map(|()| MutationResult {
        used_fallback: false,
    })
}

pub fn apply_sheet_action(
    src: &Path,
    dst: &Path,
    action: &SheetAction,
) -> Result<MutationResult, XliError> {
    match action {
        SheetAction::Rename { .. }
        | SheetAction::Hide { .. }
        | SheetAction::Unhide { .. }
        | SheetAction::Reorder { .. } => {
            patch_sheet_action(src, dst, action).map(|()| MutationResult {
                used_fallback: false,
            })
        }
        SheetAction::Add { .. } | SheetAction::Delete { .. } | SheetAction::Copy { .. } => {
            mutate_workbook(src, dst, |book| {
                sheet_action_in_book(book, action)?;
                Ok(MutationResult {
                    used_fallback: true,
                })
            })
        }
    }
}

pub fn apply_batch(src: &Path, dst: &Path, ops: &[BatchOp]) -> Result<BatchApplyResult, XliError> {
    if ops.iter().all(|op| matches!(op, BatchOp::Write { .. })) {
        return patch_write_only_batch(src, dst, ops).map(|(summary, needs_recalc)| {
            BatchApplyResult {
                summary,
                needs_recalc,
                used_fallback: false,
            }
        });
    }

    if ops.iter().all(batch_op_supported_without_fallback) {
        return patch_supported_batch(src, dst, ops);
    }

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

        Ok(BatchApplyResult {
            summary,
            needs_recalc,
            used_fallback: true,
        })
    })
}

fn batch_op_supported_without_fallback(op: &BatchOp) -> bool {
    match op {
        BatchOp::Write { .. } => true,
        BatchOp::Format { style, .. } => {
            style.horizontal_align.is_none() && style.vertical_align.is_none()
        }
        BatchOp::Sheet { action } => matches!(
            action,
            SheetAction::Rename { .. }
                | SheetAction::Hide { .. }
                | SheetAction::Unhide { .. }
                | SheetAction::Reorder { .. }
        ),
    }
}

fn patch_supported_batch(
    src: &Path,
    dst: &Path,
    ops: &[BatchOp],
) -> Result<BatchApplyResult, XliError> {
    if ops.is_empty() {
        WorkbookPatcher::open(src, dst)?.finalize()?;
        return Ok(BatchApplyResult::default());
    }

    let mut summary = BatchSummary::default();
    let mut needs_recalc = false;
    let mut current = src.to_path_buf();
    let mut temps = Vec::new();

    for (index, op) in ops.iter().enumerate() {
        let is_last = index + 1 == ops.len();
        let target = if is_last {
            dst.to_path_buf()
        } else {
            let temp = batch_temp_path(dst, index);
            temps.push(temp.clone());
            temp
        };

        let result = match op {
            BatchOp::Write {
                address,
                value,
                formula,
            } => {
                let write = apply_write(&current, &target, address, value.clone(), formula.clone());
                if let Ok(write) = &write {
                    needs_recalc |= write.needs_recalc;
                    summary.ops_executed += 1;
                    summary.cells_written += 1;
                    if formula.is_some() {
                        summary.formulas_written += 1;
                    }
                }
                write.map(|_| ())
            }
            BatchOp::Format { range, style } => {
                let format = apply_format(&current, &target, range, style);
                if format.is_ok() {
                    summary.ops_executed += 1;
                    summary.cells_formatted += cells_in_range(range)? as usize;
                }
                format.map(|_| ())
            }
            BatchOp::Sheet { action } => {
                let sheet = apply_sheet_action(&current, &target, action);
                if sheet.is_ok() {
                    summary.ops_executed += 1;
                }
                sheet.map(|_| ())
            }
        };

        if let Err(error) = result {
            cleanup_batch_temps(&temps);
            return Err(error);
        }

        current = target;
    }

    cleanup_batch_temps(&temps);
    Ok(BatchApplyResult {
        summary,
        needs_recalc,
        used_fallback: false,
    })
}

fn batch_temp_path(dst: &Path, index: usize) -> PathBuf {
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    parent.join(format!(
        ".xli-batch-{}-{timestamp}-{index}.tmp.xlsx",
        std::process::id()
    ))
}

fn cleanup_batch_temps(paths: &[PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}

fn patch_write_only_batch(
    src: &Path,
    dst: &Path,
    ops: &[BatchOp],
) -> Result<(BatchSummary, bool), XliError> {
    let mut patcher = WorkbookPatcher::open(src, dst)?;
    let sheet_parts = discover_sheet_parts(&mut patcher)?;
    let mut grouped: HashMap<String, Vec<(String, CellContent)>> = HashMap::new();
    let mut summary = BatchSummary::default();
    let mut needs_recalc = false;

    for op in ops {
        let BatchOp::Write {
            address,
            value,
            formula,
        } = op
        else {
            continue;
        };
        let cell = parse_address(address).map_err(XliError::from)?;
        let sheet_name = resolve_sheet_part_name(&sheet_parts, cell.sheet.as_deref())?;
        let sheet_part = sheet_parts
            .get(&sheet_name)
            .ok_or_else(|| XliError::SheetNotFound {
                sheet: sheet_name.clone(),
            })?
            .clone();
        let content = if let Some(formula) = formula {
            needs_recalc = true;
            summary.formulas_written += 1;
            CellContent::Formula(formula.clone())
        } else {
            CellContent::Value(value.clone())
        };
        summary.ops_executed += 1;
        summary.cells_written += 1;
        grouped
            .entry(sheet_part)
            .or_default()
            .push((format!("{}{}", cell.col, cell.row), content));
    }

    for (sheet_part, writes) in grouped {
        let mut sheet_xml = patcher.read_part(&sheet_part)?;
        for (cell_ref, content) in writes {
            sheet_xml = patch_sheet_cell(&sheet_xml, &cell_ref, &content)?;
        }
        patcher.patch_part_bytes(&sheet_part, sheet_xml);
    }

    if needs_recalc {
        mark_workbook_for_recalc(&mut patcher)?;
    }

    patcher.finalize()?;
    Ok((summary, needs_recalc))
}

fn patch_format(src: &Path, dst: &Path, range: &str, style: &StyleSpec) -> Result<(), XliError> {
    let range_ref = parse_range(range).map_err(XliError::from)?;
    let mut patcher = WorkbookPatcher::open(src, dst)?;
    let sheet_parts = discover_sheet_parts(&mut patcher)?;
    let sheet_name = resolve_sheet_part_name(&sheet_parts, range_ref.sheet.as_deref())?;
    let sheet_part = sheet_parts
        .get(&sheet_name)
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: sheet_name.clone(),
        })?
        .clone();

    let style_id = if style_has_cell_changes(style) {
        let styles_xml = patcher.read_part("xl/styles.xml")?;
        let (patched_styles, style_id) = append_style(&styles_xml, style)?;
        patcher.patch_part_bytes("xl/styles.xml", patched_styles);
        Some(style_id)
    } else {
        None
    };

    let mut sheet_xml = patcher.read_part(&sheet_part)?;
    if let Some(style_id) = style_id {
        for row in range_ref.start.row..=range_ref.end.row {
            for col_idx in range_ref.start.col_idx..=range_ref.end.col_idx {
                let cell_ref = format!("{}{}", col_to_letter(col_idx), row);
                sheet_xml = patch_sheet_cell_style(&sheet_xml, &cell_ref, style_id)?;
            }
        }
    }

    if let Some(width) = style.column_width {
        sheet_xml = patch_column_widths(
            &sheet_xml,
            range_ref.start.col_idx,
            range_ref.end.col_idx,
            width,
        )?;
    }

    patcher.patch_part_bytes(&sheet_part, sheet_xml);
    patcher.finalize()
}

fn patch_sheet_action(src: &Path, dst: &Path, action: &SheetAction) -> Result<(), XliError> {
    let mut patcher = WorkbookPatcher::open(src, dst)?;
    let workbook_xml = patcher.read_part("xl/workbook.xml")?;
    let patched = patch_workbook_sheet_action(&workbook_xml, action)?;
    patcher.patch_part_bytes("xl/workbook.xml", patched);
    patcher.finalize()
}

fn patch_workbook_sheet_action(
    workbook_xml: &[u8],
    action: &SheetAction,
) -> Result<Vec<u8>, XliError> {
    let mut reader = Reader::from_reader(Cursor::new(workbook_xml));
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"sheets" => {
                writer.write_event(Event::Start(start)).map_err(xml_error)?;
                patch_sheets_block(&mut reader, &mut writer, action)?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(writer.into_inner())
}

fn patch_sheets_block(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    action: &SheetAction,
) -> Result<(), XliError> {
    let mut buffer = Vec::new();
    let mut sheets = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"sheet" => {
                sheets.push(sheet_element(reader, &start)?);
            }
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"sheet" => {
                let sheet = sheet_element(reader, &start)?;
                skip_until_end(reader, b"sheet")?;
                sheets.push(sheet);
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"sheets" => {
                apply_workbook_sheet_action(&mut sheets, action)?;
                for sheet in &sheets {
                    write_sheet_element(writer, sheet)?;
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

#[derive(Clone, Debug, Default)]
struct WorkbookSheet {
    name: String,
    sheet_id: String,
    rel_id: String,
    state: Option<String>,
}

fn sheet_element(
    reader: &Reader<Cursor<&[u8]>>,
    start: &BytesStart<'_>,
) -> Result<WorkbookSheet, XliError> {
    let mut sheet = WorkbookSheet::default();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        let key = normalize_xml_attr_name(&attribute)?;
        let value = decode_attribute(reader, &attribute)?;
        match key.as_str() {
            "name" => sheet.name = value,
            "sheetid" => sheet.sheet_id = value,
            "id" => sheet.rel_id = value,
            "state" => sheet.state = Some(value),
            _ => {}
        }
    }
    Ok(sheet)
}

fn apply_workbook_sheet_action(
    sheets: &mut Vec<WorkbookSheet>,
    action: &SheetAction,
) -> Result<(), XliError> {
    match action {
        SheetAction::Rename { from, to } => {
            let sheet = sheets
                .iter_mut()
                .find(|sheet| sheet.name == *from)
                .ok_or_else(|| XliError::SheetNotFound {
                    sheet: from.clone(),
                })?;
            sheet.name = to.clone();
        }
        SheetAction::Hide { name } => {
            let sheet = sheets
                .iter_mut()
                .find(|sheet| sheet.name == *name)
                .ok_or_else(|| XliError::SheetNotFound {
                    sheet: name.clone(),
                })?;
            sheet.state = Some("hidden".to_string());
        }
        SheetAction::Unhide { name } => {
            let sheet = sheets
                .iter_mut()
                .find(|sheet| sheet.name == *name)
                .ok_or_else(|| XliError::SheetNotFound {
                    sheet: name.clone(),
                })?;
            sheet.state = None;
        }
        SheetAction::Reorder { sheets: order } => {
            if order.len() != sheets.len() {
                return Err(XliError::SpecValidationError {
                    spec: "sheet reorder".to_string(),
                    details: "Order must list every existing sheet exactly once".to_string(),
                });
            }
            let mut reordered = Vec::with_capacity(sheets.len());
            for name in order {
                let position = sheets
                    .iter()
                    .position(|sheet| sheet.name == *name)
                    .ok_or_else(|| XliError::SheetNotFound {
                        sheet: name.clone(),
                    })?;
                reordered.push(sheets[position].clone());
            }
            *sheets = reordered;
        }
        SheetAction::Add { .. } | SheetAction::Delete { .. } | SheetAction::Copy { .. } => {}
    }
    Ok(())
}

fn write_sheet_element(
    writer: &mut Writer<Vec<u8>>,
    sheet: &WorkbookSheet,
) -> Result<(), XliError> {
    let mut start = BytesStart::new("sheet");
    start.push_attribute(("name", sheet.name.as_str()));
    start.push_attribute(("sheetId", sheet.sheet_id.as_str()));
    start.push_attribute(("r:id", sheet.rel_id.as_str()));
    if let Some(state) = sheet.state.as_deref() {
        start.push_attribute(("state", state));
    }
    writer.write_event(Event::Empty(start)).map_err(xml_error)
}

fn style_has_cell_changes(style: &StyleSpec) -> bool {
    style.bold.is_some()
        || style.italic.is_some()
        || style.font_color.is_some()
        || style.fill.is_some()
        || style.number_format.is_some()
}

#[derive(Clone, Debug, Default)]
struct StyleCounts {
    fonts: u32,
    fills: u32,
    cell_xfs: u32,
    num_fmts: u32,
    max_num_fmt_id: u32,
    has_num_fmts: bool,
}

fn append_style(styles_xml: &[u8], style: &StyleSpec) -> Result<(Vec<u8>, u32), XliError> {
    let counts = style_counts(styles_xml)?;
    let add_font = style.bold.is_some() || style.italic.is_some() || style.font_color.is_some();
    let add_fill = style.fill.is_some();
    let add_num_fmt = style.number_format.is_some();
    let font_id = if add_font { counts.fonts } else { 0 };
    let fill_id = if add_fill { counts.fills } else { 0 };
    let num_fmt_id = if add_num_fmt {
        counts.max_num_fmt_id.max(163) + 1
    } else {
        0
    };
    let style_id = counts.cell_xfs;

    let mut reader = Reader::from_reader(Cursor::new(styles_xml));
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut inserted_num_fmts = false;

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"numFmts" => {
                write_start_with_count(
                    &reader,
                    &mut writer,
                    &start,
                    counts.num_fmts + u32::from(add_num_fmt),
                )?;
                inserted_num_fmts = true;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"numFmts" => {
                if add_num_fmt {
                    write_num_fmt(
                        &mut writer,
                        num_fmt_id,
                        style.number_format.as_deref().unwrap(),
                    )?;
                }
                writer.write_event(Event::End(end)).map_err(xml_error)?;
            }
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"fonts" => {
                if add_num_fmt && !counts.has_num_fmts && !inserted_num_fmts {
                    write_num_fmts_block(
                        &mut writer,
                        num_fmt_id,
                        style.number_format.as_deref().unwrap(),
                    )?;
                    inserted_num_fmts = true;
                }
                write_start_with_count(
                    &reader,
                    &mut writer,
                    &start,
                    counts.fonts + u32::from(add_font),
                )?;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"fonts" => {
                if add_font {
                    write_font(&mut writer, style)?;
                }
                writer.write_event(Event::End(end)).map_err(xml_error)?;
            }
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"fills" => {
                write_start_with_count(
                    &reader,
                    &mut writer,
                    &start,
                    counts.fills + u32::from(add_fill),
                )?;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"fills" => {
                if add_fill {
                    write_fill(&mut writer, style.fill.as_deref().unwrap())?;
                }
                writer.write_event(Event::End(end)).map_err(xml_error)?;
            }
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"cellXfs" => {
                write_start_with_count(&reader, &mut writer, &start, counts.cell_xfs + 1)?;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"cellXfs" => {
                write_cell_xf(
                    &mut writer,
                    font_id,
                    fill_id,
                    num_fmt_id,
                    add_font,
                    add_fill,
                    add_num_fmt,
                )?;
                writer.write_event(Event::End(end)).map_err(xml_error)?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok((writer.into_inner(), style_id))
}

fn style_counts(styles_xml: &[u8]) -> Result<StyleCounts, XliError> {
    let mut reader = Reader::from_reader(Cursor::new(styles_xml));
    let mut buffer = Vec::new();
    let mut counts = StyleCounts::default();
    let mut in_fonts = false;
    let mut in_fills = false;
    let mut in_cell_xfs = false;
    let mut in_num_fmts = false;

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                match start.name().local_name().as_ref() {
                    b"fonts" => in_fonts = true,
                    b"fills" => in_fills = true,
                    b"cellXfs" => in_cell_xfs = true,
                    b"numFmts" => {
                        in_num_fmts = true;
                        counts.has_num_fmts = true;
                    }
                    b"font" if in_fonts => counts.fonts += 1,
                    b"fill" if in_fills => counts.fills += 1,
                    b"xf" if in_cell_xfs => counts.cell_xfs += 1,
                    b"numFmt" if in_num_fmts => {
                        counts.num_fmts += 1;
                        if let Some(id) = attr_value(&reader, &start, b"numFmtId")?
                            .and_then(|value| value.parse::<u32>().ok())
                        {
                            counts.max_num_fmt_id = counts.max_num_fmt_id.max(id);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(end)) => match end.name().local_name().as_ref() {
                b"fonts" => in_fonts = false,
                b"fills" => in_fills = false,
                b"cellXfs" => in_cell_xfs = false,
                b"numFmts" => in_num_fmts = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(counts)
}

fn write_start_with_count(
    reader: &Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    start: &BytesStart<'_>,
    count: u32,
) -> Result<(), XliError> {
    let mut updated = start.to_owned();
    updated.clear_attributes();
    let count_text = count.to_string();
    let mut attrs = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        let key = std::str::from_utf8(attribute.key.as_ref())
            .map_err(xml_error)?
            .to_string();
        if attribute.key.local_name().as_ref() == b"count" {
            continue;
        }
        attrs.push((key, decode_attribute(reader, &attribute)?));
    }
    for (key, value) in &attrs {
        updated.push_attribute((key.as_str(), value.as_str()));
    }
    updated.push_attribute(("count", count_text.as_str()));
    writer.write_event(Event::Start(updated)).map_err(xml_error)
}

fn attr_value(
    reader: &Reader<Cursor<&[u8]>>,
    start: &BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, XliError> {
    for attribute in start.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        if attribute.key.local_name().as_ref() == name {
            return decode_attribute(reader, &attribute).map(Some);
        }
    }
    Ok(None)
}

fn write_num_fmts_block(
    writer: &mut Writer<Vec<u8>>,
    id: u32,
    format: &str,
) -> Result<(), XliError> {
    let mut start = BytesStart::new("numFmts");
    start.push_attribute(("count", "1"));
    writer.write_event(Event::Start(start)).map_err(xml_error)?;
    write_num_fmt(writer, id, format)?;
    writer
        .write_event(Event::End(BytesEnd::new("numFmts")))
        .map_err(xml_error)
}

fn write_num_fmt(writer: &mut Writer<Vec<u8>>, id: u32, format: &str) -> Result<(), XliError> {
    let mut num_fmt = BytesStart::new("numFmt");
    let id_text = id.to_string();
    let resolved = xli_core::resolve_number_format(format);
    num_fmt.push_attribute(("numFmtId", id_text.as_str()));
    num_fmt.push_attribute(("formatCode", resolved.as_str()));
    writer.write_event(Event::Empty(num_fmt)).map_err(xml_error)
}

fn write_font(writer: &mut Writer<Vec<u8>>, style: &StyleSpec) -> Result<(), XliError> {
    writer
        .write_event(Event::Start(BytesStart::new("font")))
        .map_err(xml_error)?;
    if style.bold == Some(true) {
        writer
            .write_event(Event::Empty(BytesStart::new("b")))
            .map_err(xml_error)?;
    }
    if style.italic == Some(true) {
        writer
            .write_event(Event::Empty(BytesStart::new("i")))
            .map_err(xml_error)?;
    }
    if let Some(color) = style.font_color.as_deref() {
        let mut color_start = BytesStart::new("color");
        let rgb = normalize_argb(color);
        color_start.push_attribute(("rgb", rgb.as_str()));
        writer
            .write_event(Event::Empty(color_start))
            .map_err(xml_error)?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("font")))
        .map_err(xml_error)
}

fn write_fill(writer: &mut Writer<Vec<u8>>, color: &str) -> Result<(), XliError> {
    writer
        .write_event(Event::Start(BytesStart::new("fill")))
        .map_err(xml_error)?;
    let mut pattern = BytesStart::new("patternFill");
    pattern.push_attribute(("patternType", "solid"));
    writer
        .write_event(Event::Start(pattern))
        .map_err(xml_error)?;
    let mut fg = BytesStart::new("fgColor");
    let rgb = normalize_argb(color);
    fg.push_attribute(("rgb", rgb.as_str()));
    writer.write_event(Event::Empty(fg)).map_err(xml_error)?;
    writer
        .write_event(Event::Empty(BytesStart::new("bgColor")))
        .map_err(xml_error)?;
    writer
        .write_event(Event::End(BytesEnd::new("patternFill")))
        .map_err(xml_error)?;
    writer
        .write_event(Event::End(BytesEnd::new("fill")))
        .map_err(xml_error)
}

fn write_cell_xf(
    writer: &mut Writer<Vec<u8>>,
    font_id: u32,
    fill_id: u32,
    num_fmt_id: u32,
    apply_font: bool,
    apply_fill: bool,
    apply_num_fmt: bool,
) -> Result<(), XliError> {
    let mut xf = BytesStart::new("xf");
    let font_id = font_id.to_string();
    let fill_id = fill_id.to_string();
    let num_fmt_id = num_fmt_id.to_string();
    xf.push_attribute(("numFmtId", num_fmt_id.as_str()));
    xf.push_attribute(("fontId", font_id.as_str()));
    xf.push_attribute(("fillId", fill_id.as_str()));
    xf.push_attribute(("borderId", "0"));
    xf.push_attribute(("xfId", "0"));
    if apply_font {
        xf.push_attribute(("applyFont", "1"));
    }
    if apply_fill {
        xf.push_attribute(("applyFill", "1"));
    }
    if apply_num_fmt {
        xf.push_attribute(("applyNumberFormat", "1"));
    }
    writer.write_event(Event::Empty(xf)).map_err(xml_error)
}

fn patch_column_widths(
    sheet_xml: &[u8],
    start_col_idx: u32,
    end_col_idx: u32,
    width: f64,
) -> Result<Vec<u8>, XliError> {
    let mut reader = Reader::from_reader(Cursor::new(sheet_xml));
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut patched_cols = false;

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"cols" => {
                writer.write_event(Event::Start(start)).map_err(xml_error)?;
                copy_until_end_with_inserted_col(
                    &mut reader,
                    &mut writer,
                    start_col_idx,
                    end_col_idx,
                    width,
                )?;
                patched_cols = true;
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"cols" => {
                writer
                    .write_event(Event::Start(start.to_owned()))
                    .map_err(xml_error)?;
                write_col_width(&mut writer, start_col_idx, end_col_idx, width)?;
                writer
                    .write_event(Event::End(BytesEnd::new("cols")))
                    .map_err(xml_error)?;
                patched_cols = true;
            }
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"sheetData" => {
                if !patched_cols {
                    write_cols_block(&mut writer, start_col_idx, end_col_idx, width)?;
                    patched_cols = true;
                }
                writer.write_event(Event::Start(start)).map_err(xml_error)?;
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"sheetData" => {
                if !patched_cols {
                    write_cols_block(&mut writer, start_col_idx, end_col_idx, width)?;
                    patched_cols = true;
                }
                writer.write_event(Event::Empty(start)).map_err(xml_error)?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event).map_err(xml_error)?,
            Err(error) => return Err(xml_error(error)),
        }
    }

    Ok(writer.into_inner())
}

fn copy_until_end_with_inserted_col(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    start_col_idx: u32,
    end_col_idx: u32,
    width: f64,
) -> Result<(), XliError> {
    let mut buffer = Vec::new();
    let mut depth = 1_u32;
    while depth > 0 {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"cols" && depth == 1 => {
                write_col_width(writer, start_col_idx, end_col_idx, width)?;
                writer.write_event(Event::End(end)).map_err(xml_error)?;
                depth -= 1;
            }
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"cols" => {
                depth += 1;
                writer.write_event(Event::Start(start)).map_err(xml_error)?;
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"cols" => {
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

fn write_cols_block(
    writer: &mut Writer<Vec<u8>>,
    start_col_idx: u32,
    end_col_idx: u32,
    width: f64,
) -> Result<(), XliError> {
    writer
        .write_event(Event::Start(BytesStart::new("cols")))
        .map_err(xml_error)?;
    write_col_width(writer, start_col_idx, end_col_idx, width)?;
    writer
        .write_event(Event::End(BytesEnd::new("cols")))
        .map_err(xml_error)
}

fn write_col_width(
    writer: &mut Writer<Vec<u8>>,
    start_col_idx: u32,
    end_col_idx: u32,
    width: f64,
) -> Result<(), XliError> {
    let mut col = BytesStart::new("col");
    let min = (start_col_idx + 1).to_string();
    let max = (end_col_idx + 1).to_string();
    let width = width.to_string();
    col.push_attribute(("min", min.as_str()));
    col.push_attribute(("max", max.as_str()));
    col.push_attribute(("width", width.as_str()));
    col.push_attribute(("customWidth", "1"));
    writer.write_event(Event::Empty(col)).map_err(xml_error)
}

fn discover_sheet_parts(patcher: &mut WorkbookPatcher) -> Result<SheetPartMap, XliError> {
    let workbook_xml = patcher.read_part("xl/workbook.xml")?;
    let workbook_rels = patcher.read_part("xl/_rels/workbook.xml.rels")?;
    let sheet_relationships = parse_workbook_sheets(&workbook_xml)?;
    let rel_targets = parse_relationship_targets(&workbook_rels, "xl/workbook.xml")?;

    let mut sheet_parts = SheetPartMap::default();
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
    sheet_parts: &SheetPartMap,
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
        .first_name()
        .ok_or_else(|| XliError::SheetNotFound {
            sheet: "<first>".to_string(),
        })
}

fn patch_sheet_cell(
    sheet_xml: &[u8],
    cell_ref: &str,
    content: &CellContent,
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
                patch_sheet_data(&mut reader, &mut writer, &target, content, &mut inserted)?;
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"sheetData" => {
                writer
                    .write_event(Event::Start(start.to_owned()))
                    .map_err(xml_error)?;
                write_row_with_cell(&mut writer, target.row, cell_ref, content, None)?;
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

fn patch_sheet_cell_style(
    sheet_xml: &[u8],
    cell_ref: &str,
    style_id: u32,
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
                patch_sheet_data_style(&mut reader, &mut writer, &target, style_id, &mut inserted)?;
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"sheetData" => {
                writer
                    .write_event(Event::Start(start.to_owned()))
                    .map_err(xml_error)?;
                write_row_with_styled_blank(&mut writer, target.row, cell_ref, style_id)?;
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

fn patch_sheet_data_style(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    target: &xli_core::CellRef,
    style_id: u32,
    inserted: &mut bool,
) -> Result<(), XliError> {
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"row" => {
                let row_num = row_number(reader, &start)?;
                if !*inserted && row_num > target.row {
                    write_row_with_styled_blank(writer, target.row, &target_ref(target), style_id)?;
                    *inserted = true;
                }
                if row_num == target.row {
                    patch_row_style(reader, writer, start, target, style_id)?;
                    *inserted = true;
                } else {
                    writer.write_event(Event::Start(start)).map_err(xml_error)?;
                    copy_until_end(reader, writer, b"row")?;
                }
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"row" => {
                let row_num = row_number(reader, &start)?;
                if !*inserted && row_num > target.row {
                    write_row_with_styled_blank(writer, target.row, &target_ref(target), style_id)?;
                    *inserted = true;
                }
                if row_num == target.row {
                    write_row_with_styled_blank(writer, target.row, &target_ref(target), style_id)?;
                    *inserted = true;
                } else {
                    writer.write_event(Event::Empty(start)).map_err(xml_error)?;
                }
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"sheetData" => {
                if !*inserted {
                    write_row_with_styled_blank(writer, target.row, &target_ref(target), style_id)?;
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

fn patch_row_style(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    row_start: BytesStart<'_>,
    target: &xli_core::CellRef,
    style_id: u32,
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
                let (cell_ref, _) = cell_reference_and_style(reader, &start)?;
                let cell_col = cell_ref
                    .as_deref()
                    .and_then(|value| parse_address(value).ok())
                    .map(|cell| cell.col_idx);
                if !inserted && cell_col.is_some_and(|col| col > target.col_idx) {
                    write_styled_blank_cell(writer, &target_ref(target), style_id)?;
                    inserted = true;
                }
                if cell_col == Some(target.col_idx) {
                    write_styled_cell_start(writer, &start, style_id)?;
                    copy_until_end(reader, writer, b"c")?;
                    inserted = true;
                } else {
                    writer.write_event(Event::Start(start)).map_err(xml_error)?;
                    copy_until_end(reader, writer, b"c")?;
                }
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"c" => {
                let (cell_ref, _) = cell_reference_and_style(reader, &start)?;
                let cell_col = cell_ref
                    .as_deref()
                    .and_then(|value| parse_address(value).ok())
                    .map(|cell| cell.col_idx);
                if !inserted && cell_col.is_some_and(|col| col > target.col_idx) {
                    write_styled_blank_cell(writer, &target_ref(target), style_id)?;
                    inserted = true;
                }
                if cell_col == Some(target.col_idx) {
                    write_styled_cell_empty(writer, &start, style_id)?;
                    inserted = true;
                } else {
                    writer.write_event(Event::Empty(start)).map_err(xml_error)?;
                }
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"row" => {
                if !inserted {
                    write_styled_blank_cell(writer, &target_ref(target), style_id)?;
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

fn write_row_with_styled_blank(
    writer: &mut Writer<Vec<u8>>,
    row: u32,
    cell_ref: &str,
    style_id: u32,
) -> Result<(), XliError> {
    let mut row_start = BytesStart::new("row");
    let row_text = row.to_string();
    row_start.push_attribute(("r", row_text.as_str()));
    writer
        .write_event(Event::Start(row_start))
        .map_err(xml_error)?;
    write_styled_blank_cell(writer, cell_ref, style_id)?;
    writer
        .write_event(Event::End(BytesEnd::new("row")))
        .map_err(xml_error)
}

fn write_styled_blank_cell(
    writer: &mut Writer<Vec<u8>>,
    cell_ref: &str,
    style_id: u32,
) -> Result<(), XliError> {
    let mut cell = BytesStart::new("c");
    let style_text = style_id.to_string();
    cell.push_attribute(("r", cell_ref));
    cell.push_attribute(("s", style_text.as_str()));
    writer.write_event(Event::Empty(cell)).map_err(xml_error)
}

fn write_styled_cell_start(
    writer: &mut Writer<Vec<u8>>,
    start: &BytesStart<'_>,
    style_id: u32,
) -> Result<(), XliError> {
    let updated = cell_with_style(start, style_id)?;
    writer.write_event(Event::Start(updated)).map_err(xml_error)
}

fn write_styled_cell_empty(
    writer: &mut Writer<Vec<u8>>,
    start: &BytesStart<'_>,
    style_id: u32,
) -> Result<(), XliError> {
    let updated = cell_with_style(start, style_id)?;
    writer.write_event(Event::Empty(updated)).map_err(xml_error)
}

fn cell_with_style(start: &BytesStart<'_>, style_id: u32) -> Result<BytesStart<'static>, XliError> {
    let mut updated = BytesStart::new("c");
    let style_text = style_id.to_string();
    let mut attrs = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(xml_error)?;
        if attribute.key.local_name().as_ref() == b"s" {
            continue;
        }
        let key = std::str::from_utf8(attribute.key.as_ref())
            .map_err(xml_error)?
            .to_string();
        let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
        attrs.push((key, value));
    }
    for (key, value) in &attrs {
        updated.push_attribute((key.as_str(), value.as_str()));
    }
    updated.push_attribute(("s", style_text.as_str()));
    Ok(updated)
}

fn patch_sheet_data(
    reader: &mut Reader<Cursor<&[u8]>>,
    writer: &mut Writer<Vec<u8>>,
    target: &xli_core::CellRef,
    content: &CellContent,
    inserted: &mut bool,
) -> Result<(), XliError> {
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) if start.name().local_name().as_ref() == b"row" => {
                let row_num = row_number(reader, &start)?;
                if !*inserted && row_num > target.row {
                    write_row_with_cell(writer, target.row, &target_ref(target), content, None)?;
                    *inserted = true;
                }
                if row_num == target.row {
                    patch_row(reader, writer, start, target, content)?;
                    *inserted = true;
                } else {
                    writer.write_event(Event::Start(start)).map_err(xml_error)?;
                    copy_until_end(reader, writer, b"row")?;
                }
            }
            Ok(Event::Empty(start)) if start.name().local_name().as_ref() == b"row" => {
                let row_num = row_number(reader, &start)?;
                if !*inserted && row_num > target.row {
                    write_row_with_cell(writer, target.row, &target_ref(target), content, None)?;
                    *inserted = true;
                }
                if row_num == target.row {
                    write_row_with_cell(writer, target.row, &target_ref(target), content, None)?;
                    *inserted = true;
                } else {
                    writer.write_event(Event::Empty(start)).map_err(xml_error)?;
                }
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"sheetData" => {
                if !*inserted {
                    write_row_with_cell(writer, target.row, &target_ref(target), content, None)?;
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
    content: &CellContent,
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
                    write_cell(writer, &target_ref(target), content, None)?;
                    inserted = true;
                }
                if cell_col == Some(target.col_idx) {
                    write_cell(writer, &target_ref(target), content, style.as_deref())?;
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
                    write_cell(writer, &target_ref(target), content, None)?;
                    inserted = true;
                }
                if cell_col == Some(target.col_idx) {
                    write_cell(writer, &target_ref(target), content, style.as_deref())?;
                    inserted = true;
                } else {
                    writer.write_event(Event::Empty(start)).map_err(xml_error)?;
                }
            }
            Ok(Event::End(end)) if end.name().local_name().as_ref() == b"row" => {
                if !inserted {
                    write_cell(writer, &target_ref(target), content, None)?;
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
    content: &CellContent,
    style: Option<&str>,
) -> Result<(), XliError> {
    let mut row_start = BytesStart::new("row");
    let row_text = row.to_string();
    row_start.push_attribute(("r", row_text.as_str()));
    writer
        .write_event(Event::Start(row_start))
        .map_err(xml_error)?;
    write_cell(writer, cell_ref, content, style)?;
    writer
        .write_event(Event::End(BytesEnd::new("row")))
        .map_err(xml_error)
}

fn write_cell(
    writer: &mut Writer<Vec<u8>>,
    cell_ref: &str,
    content: &CellContent,
    style: Option<&str>,
) -> Result<(), XliError> {
    let mut cell = BytesStart::new("c");
    cell.push_attribute(("r", cell_ref));
    if let Some(style) = style {
        cell.push_attribute(("s", style));
    }

    match content {
        CellContent::Value(Some(Value::Null) | None) => {
            writer.write_event(Event::Empty(cell)).map_err(xml_error)?;
        }
        CellContent::Value(Some(Value::Bool(value))) => {
            cell.push_attribute(("t", "b"));
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            write_text_element(writer, "v", if *value { "1" } else { "0" })?;
            writer
                .write_event(Event::End(BytesEnd::new("c")))
                .map_err(xml_error)?;
        }
        CellContent::Value(Some(Value::Number(number))) => {
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            write_text_element(writer, "v", &number.to_string())?;
            writer
                .write_event(Event::End(BytesEnd::new("c")))
                .map_err(xml_error)?;
        }
        CellContent::Value(Some(Value::String(value))) => {
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
        CellContent::Value(Some(other)) => {
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
        CellContent::Formula(formula) => {
            writer.write_event(Event::Start(cell)).map_err(xml_error)?;
            write_text_element(writer, "f", normalize_formula(formula))?;
            write_text_element(writer, "v", "0")?;
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

fn normalize_formula(formula: &str) -> &str {
    formula.strip_prefix('=').unwrap_or(formula)
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
