use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum 0-based column index supported by Excel (`XFD`).
pub const MAX_COL_IDX: u32 = 16_383;

/// Maximum 1-based row index supported by Excel.
pub const MAX_ROW: u32 = 1_048_576;

/// A parsed Excel cell reference in normalized form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRef {
    pub sheet: Option<String>,
    pub col: String,
    pub row: u32,
    pub col_idx: u32,
}

/// A parsed Excel range reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeRef {
    pub sheet: Option<String>,
    pub start: CellRef,
    pub end: CellRef,
}

/// Errors returned while parsing Excel addresses.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AddressError {
    #[error("Cell address cannot be empty")]
    EmptyInput,
    #[error("Invalid cell address: {input}")]
    InvalidAddress { input: String },
    #[error("Column {column} is outside Excel bounds (A:XFD)")]
    ColumnOutOfBounds { column: String },
    #[error("Row {row} is outside Excel bounds (1:{max})")]
    RowOutOfBounds { row: u32, max: u32 },
    #[error("Range references must stay on one sheet: {left} vs {right}")]
    SheetMismatch { left: String, right: String },
}

/// Convert a 0-based column index to an Excel column label.
pub fn col_to_letter(col_idx: u32) -> String {
    let mut value = col_idx + 1;
    let mut letters = Vec::new();

    while value > 0 {
        let remainder = (value - 1) % 26;
        letters.push((b'A' + remainder as u8) as char);
        value = (value - 1) / 26;
    }

    letters.iter().rev().collect()
}

/// Convert an Excel column label to a 0-based column index.
pub fn letter_to_col(s: &str) -> Result<u32, AddressError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(AddressError::EmptyInput);
    }

    let mut value = 0_u32;
    let mut normalized = String::with_capacity(trimmed.len());

    for ch in trimmed.chars() {
        if !ch.is_ascii_alphabetic() {
            return Err(AddressError::InvalidAddress {
                input: trimmed.to_string(),
            });
        }

        let upper = ch.to_ascii_uppercase();
        normalized.push(upper);
        value = value
            .checked_mul(26)
            .and_then(|current| current.checked_add((upper as u32) - ('A' as u32) + 1))
            .ok_or_else(|| AddressError::ColumnOutOfBounds {
                column: normalized.clone(),
            })?;
    }

    let col_idx = value - 1;
    if col_idx > MAX_COL_IDX {
        return Err(AddressError::ColumnOutOfBounds { column: normalized });
    }

    Ok(col_idx)
}

/// Parse an Excel A1-style cell reference, optionally including a sheet prefix.
pub fn parse_address(s: &str) -> Result<CellRef, AddressError> {
    let cleaned = normalize_reference(s)?;
    let (sheet, cell) = split_sheet_and_cell(&cleaned)?;
    let (col, row) = split_cell_parts(cell)?;
    let col_idx = letter_to_col(col)?;
    let row_idx = parse_row(row)?;

    Ok(CellRef {
        sheet,
        col: col.to_ascii_uppercase(),
        row: row_idx,
        col_idx,
    })
}

/// Parse an Excel A1-style range reference, optionally including a sheet prefix.
pub fn parse_range(s: &str) -> Result<RangeRef, AddressError> {
    let cleaned = normalize_reference(s)?;
    let (left, right) = cleaned
        .split_once(':')
        .ok_or_else(|| AddressError::InvalidAddress {
            input: cleaned.clone(),
        })?;

    if right.contains(':') {
        return Err(AddressError::InvalidAddress { input: cleaned });
    }

    let mut start = parse_address(left)?;
    let mut end = parse_address(right)?;

    let sheet = match (start.sheet.clone(), end.sheet.clone()) {
        (Some(left_sheet), Some(right_sheet)) if left_sheet == right_sheet => Some(left_sheet),
        (Some(left_sheet), Some(right_sheet)) => {
            return Err(AddressError::SheetMismatch {
                left: left_sheet,
                right: right_sheet,
            });
        }
        (Some(sheet), None) => {
            end.sheet = Some(sheet.clone());
            Some(sheet)
        }
        (None, Some(sheet)) => {
            start.sheet = Some(sheet.clone());
            Some(sheet)
        }
        (None, None) => None,
    };

    Ok(RangeRef { sheet, start, end })
}

fn normalize_reference(input: &str) -> Result<String, AddressError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AddressError::EmptyInput);
    }

    Ok(trimmed.replace('$', ""))
}

fn split_sheet_and_cell(input: &str) -> Result<(Option<String>, &str), AddressError> {
    match input.rsplit_once('!') {
        Some((sheet, cell)) if sheet.is_empty() || cell.is_empty() => {
            Err(AddressError::InvalidAddress {
                input: input.to_string(),
            })
        }
        Some((sheet, cell)) => Ok((Some(normalize_sheet_name(sheet)), cell)),
        None => Ok((None, input)),
    }
}

fn normalize_sheet_name(sheet: &str) -> String {
    if sheet.len() >= 2 && sheet.starts_with('\'') && sheet.ends_with('\'') {
        return sheet[1..sheet.len() - 1].replace("''", "'");
    }

    sheet.to_string()
}

fn split_cell_parts(input: &str) -> Result<(&str, &str), AddressError> {
    let mut split_at = None;
    let mut seen_digit = false;

    for (idx, ch) in input.char_indices() {
        if ch.is_ascii_alphabetic() {
            if seen_digit {
                return Err(AddressError::InvalidAddress {
                    input: input.to_string(),
                });
            }
            continue;
        }

        if ch.is_ascii_digit() {
            if split_at.is_none() {
                split_at = Some(idx);
            }
            seen_digit = true;
            continue;
        }

        return Err(AddressError::InvalidAddress {
            input: input.to_string(),
        });
    }

    let split_at = split_at.ok_or_else(|| AddressError::InvalidAddress {
        input: input.to_string(),
    })?;
    let (col, row) = input.split_at(split_at);

    if col.is_empty() || row.is_empty() {
        return Err(AddressError::InvalidAddress {
            input: input.to_string(),
        });
    }

    Ok((col, row))
}

fn parse_row(input: &str) -> Result<u32, AddressError> {
    let row = input
        .parse::<u32>()
        .map_err(|_| AddressError::InvalidAddress {
            input: input.to_string(),
        })?;

    if row == 0 || row > MAX_ROW {
        return Err(AddressError::RowOutOfBounds { row, max: MAX_ROW });
    }

    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::{
        col_to_letter, letter_to_col, parse_address, parse_range, AddressError, MAX_COL_IDX,
    };

    #[test]
    fn converts_known_column_indexes() {
        let cases = [
            (0, "A"),
            (25, "Z"),
            (26, "AA"),
            (51, "AZ"),
            (52, "BA"),
            (63, "BL"),
            (16_383, "XFD"),
        ];

        for (idx, expected) in cases {
            assert_eq!(col_to_letter(idx), expected);
            assert_eq!(letter_to_col(expected), Ok(idx));
        }
    }

    #[test]
    fn round_trips_all_valid_excel_columns() {
        for idx in 0..=MAX_COL_IDX {
            let col = col_to_letter(idx);
            assert_eq!(letter_to_col(&col), Ok(idx));
        }
    }

    #[test]
    fn parses_rows_sheet_prefixes_and_absolute_refs() {
        let simple = parse_address("B10").expect("B10 should parse");
        assert_eq!(simple.sheet, None);
        assert_eq!(simple.col, "B");
        assert_eq!(simple.row, 10);
        assert_eq!(simple.col_idx, 1);

        let with_sheet = parse_address("Summary!$B$10").expect("sheet ref should parse");
        assert_eq!(with_sheet.sheet.as_deref(), Some("Summary"));
        assert_eq!(with_sheet.col, "B");
        assert_eq!(with_sheet.row, 10);
        assert_eq!(with_sheet.col_idx, 1);

        let with_spaces =
            parse_address("Sheet Name With Spaces!A1").expect("sheet with spaces should parse");
        assert_eq!(with_spaces.sheet.as_deref(), Some("Sheet Name With Spaces"));
        assert_eq!(with_spaces.col, "A");
        assert_eq!(with_spaces.row, 1);
        assert_eq!(with_spaces.col_idx, 0);
    }

    #[test]
    fn parses_ranges_and_applies_sheet_prefix_to_both_ends() {
        let range = parse_range("Summary!B2:D5").expect("range should parse");
        assert_eq!(range.sheet.as_deref(), Some("Summary"));
        assert_eq!(range.start.sheet.as_deref(), Some("Summary"));
        assert_eq!(range.end.sheet.as_deref(), Some("Summary"));
        assert_eq!(range.start.col, "B");
        assert_eq!(range.start.row, 2);
        assert_eq!(range.end.col, "D");
        assert_eq!(range.end.row, 5);
    }

    #[test]
    fn rejects_invalid_addresses() {
        assert!(matches!(
            parse_address("0A"),
            Err(AddressError::InvalidAddress { .. })
        ));
        assert!(matches!(
            parse_address("A0"),
            Err(AddressError::RowOutOfBounds { row: 0, .. })
        ));
        assert!(matches!(parse_address(""), Err(AddressError::EmptyInput)));
        assert!(matches!(
            parse_address("ZZZ99999"),
            Err(AddressError::ColumnOutOfBounds { .. })
        ));
    }
}
