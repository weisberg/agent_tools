use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use umya_spreadsheet::{self, NumberingFormat, SheetStateValues, Spreadsheet, Style};
use xli_core::{
    col_to_letter, parse_address, parse_range, BatchOp, SheetAction, StyleSpec, XliError,
};

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
}

pub fn apply_write(
    src: &Path,
    dst: &Path,
    address: &str,
    value: Option<Value>,
    formula: Option<String>,
) -> Result<WriteResult, XliError> {
    mutate_workbook(src, dst, |book| {
        let needs_recalc = write_into_book(book, address, value, formula)?;
        Ok(WriteResult { needs_recalc })
    })
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
        format.set_format_code(number_format);
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
