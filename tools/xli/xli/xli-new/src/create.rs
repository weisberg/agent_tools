use rust_xlsxwriter::Workbook;
use serde_json::Value;
use std::path::Path;
use xli_core::XliError;

pub fn create_blank(path: &Path, sheets: &[String]) -> Result<(), XliError> {
    let mut workbook = Workbook::new();

    if sheets.is_empty() {
        workbook.add_worksheet();
    } else {
        for sheet_name in sheets {
            let worksheet = workbook.add_worksheet();
            worksheet
                .set_name(sheet_name)
                .map_err(|error| XliError::WriteConflict {
                    target: sheet_name.clone(),
                    details: Some(error.to_string()),
                })?;
        }
    }

    workbook.save(path).map_err(|error| XliError::OoxmlCorrupt {
        details: error.to_string(),
    })
}

pub fn create_from_csv(csv_path: &Path, out_path: &Path, sheet_name: &str) -> Result<(), XliError> {
    // Use the csv crate for RFC 4180-compliant parsing. The previous
    // line.split(',') approach silently broke quoted fields containing commas
    // (e.g. "Smith, John" would be split into two cells). (Issue #24)
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(csv_path)
        .map_err(|error| match error.kind() {
            csv::ErrorKind::Io(io_err) if io_err.kind() == std::io::ErrorKind::NotFound => {
                XliError::FileNotFound {
                    path: csv_path.display().to_string(),
                }
            }
            _ => XliError::OoxmlCorrupt {
                details: error.to_string(),
            },
        })?;

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sheet_name)
        .map_err(|error| XliError::WriteConflict {
            target: sheet_name.to_string(),
            details: Some(error.to_string()),
        })?;

    for (row_idx, record) in reader.records().enumerate() {
        let record = record.map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })?;
        for (col_idx, field) in record.iter().enumerate() {
            worksheet
                .write_string(row_idx as u32, col_idx as u16, field)
                .map_err(|error| XliError::OoxmlCorrupt {
                    details: error.to_string(),
                })?;
        }
    }

    workbook
        .save(out_path)
        .map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })
}

pub fn create_from_markdown(
    md_path: &Path,
    out_path: &Path,
    sheet_name: &str,
) -> Result<(), XliError> {
    let content = std::fs::read_to_string(md_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            XliError::FileNotFound {
                path: md_path.display().to_string(),
            }
        } else {
            XliError::OoxmlCorrupt {
                details: error.to_string(),
            }
        }
    })?;

    let rows = parse_markdown_table(&content)?;
    if rows.is_empty() {
        return create_blank(out_path, &[sheet_name.to_string()]);
    }

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name(sheet_name)
        .map_err(|error| XliError::WriteConflict {
            target: sheet_name.to_string(),
            details: Some(error.to_string()),
        })?;

    for (row_idx, row) in rows.iter().enumerate() {
        for (col_idx, cell) in row.iter().enumerate() {
            // Try to write as number first, fall back to string
            if let Ok(num) = cell.parse::<f64>() {
                worksheet
                    .write_number(row_idx as u32, col_idx as u16, num)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            } else if cell.eq_ignore_ascii_case("true") {
                worksheet
                    .write_boolean(row_idx as u32, col_idx as u16, true)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            } else if cell.eq_ignore_ascii_case("false") {
                worksheet
                    .write_boolean(row_idx as u32, col_idx as u16, false)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            } else if !cell.is_empty() {
                worksheet
                    .write_string(row_idx as u32, col_idx as u16, cell)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            }
        }
    }

    workbook
        .save(out_path)
        .map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })
}

/// Create a workbook from a JSON template file.
///
/// The JSON format supports multiple sheets, optional headers, and rows as
/// either arrays or objects. See crate-level docs for the full schema.
///
/// Returns the number of sheets created.
pub fn create_from_json(json_path: &Path, out_path: &Path) -> Result<usize, XliError> {
    let content = std::fs::read_to_string(json_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            XliError::FileNotFound {
                path: json_path.display().to_string(),
            }
        } else {
            XliError::OoxmlCorrupt {
                details: error.to_string(),
            }
        }
    })?;

    let root: Value = serde_json::from_str(&content).map_err(|error| XliError::OoxmlCorrupt {
        details: format!("invalid JSON: {error}"),
    })?;

    let sheets = root
        .get("sheets")
        .and_then(Value::as_object)
        .ok_or_else(|| XliError::OoxmlCorrupt {
            details: "JSON must have a \"sheets\" object at the top level".to_string(),
        })?;

    let mut workbook = Workbook::new();
    let sheet_count = sheets.len();

    for (sheet_name, sheet_def) in sheets {
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(sheet_name)
            .map_err(|error| XliError::WriteConflict {
                target: sheet_name.clone(),
                details: Some(error.to_string()),
            })?;

        let rows = sheet_def.get("rows").and_then(Value::as_array);
        let explicit_headers = sheet_def.get("headers").and_then(Value::as_array);

        // Determine headers: explicit, or derived from first object row
        let derived_headers: Option<Vec<String>> = if explicit_headers.is_none() {
            rows.and_then(|r| r.first())
                .and_then(Value::as_object)
                .map(|obj| obj.keys().cloned().collect())
        } else {
            None
        };

        let headers: Option<Vec<String>> = explicit_headers
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .or(derived_headers);

        let mut row_offset: u32 = 0;

        // Write headers if present
        if let Some(ref hdrs) = headers {
            for (col, hdr) in hdrs.iter().enumerate() {
                worksheet
                    .write_string(0, col as u16, hdr)
                    .map_err(|error| XliError::OoxmlCorrupt {
                        details: error.to_string(),
                    })?;
            }
            row_offset = 1;
        }

        // Write rows
        if let Some(rows) = rows {
            for (r, row_val) in rows.iter().enumerate() {
                let row_idx = row_offset + r as u32;
                match row_val {
                    Value::Array(cells) => {
                        for (c, cell) in cells.iter().enumerate() {
                            write_json_cell(worksheet, row_idx, c as u16, cell)?;
                        }
                    }
                    Value::Object(obj) => {
                        if let Some(ref hdrs) = headers {
                            for (c, key) in hdrs.iter().enumerate() {
                                if let Some(cell) = obj.get(key) {
                                    write_json_cell(worksheet, row_idx, c as u16, cell)?;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    workbook
        .save(out_path)
        .map_err(|error| XliError::OoxmlCorrupt {
            details: error.to_string(),
        })?;

    Ok(sheet_count)
}

fn write_json_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: &Value,
) -> Result<(), XliError> {
    match value {
        Value::Number(n) => {
            let num = n.as_f64().unwrap_or(0.0);
            worksheet
                .write_number(row, col, num)
                .map_err(|e| XliError::OoxmlCorrupt {
                    details: e.to_string(),
                })?;
        }
        Value::String(s) => {
            worksheet
                .write_string(row, col, s)
                .map_err(|e| XliError::OoxmlCorrupt {
                    details: e.to_string(),
                })?;
        }
        Value::Bool(b) => {
            worksheet
                .write_boolean(row, col, *b)
                .map_err(|e| XliError::OoxmlCorrupt {
                    details: e.to_string(),
                })?;
        }
        Value::Null | Value::Array(_) | Value::Object(_) => {
            // Skip null and complex types
        }
    }
    Ok(())
}

/// Parse a markdown pipe table into a Vec of rows (each row is a Vec of cell strings).
/// Skips the separator row (contains only dashes/colons/pipes).
fn parse_markdown_table(content: &str) -> Result<Vec<Vec<String>>, XliError> {
    let mut rows = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('|') {
            // Skip non-table lines. Stop if we already have rows and hit a blank.
            if !rows.is_empty() {
                break;
            }
            continue;
        }

        // Skip separator rows like |---|---|---|
        if is_separator_row(trimmed) {
            continue;
        }

        let cells: Vec<String> = trimmed
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();

        // Trim leading/trailing empty strings from leading/trailing pipes
        let cells: Vec<String> = cells
            .into_iter()
            .skip_while(|c| c.is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip_while(|c| c.is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    Ok(rows)
}

fn is_separator_row(line: &str) -> bool {
    line.chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}

#[cfg(test)]
mod tests {
    use super::{create_blank, create_from_csv, create_from_json, create_from_markdown};
    use calamine::{open_workbook, Reader, Xlsx};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn creates_blank_workbook_with_named_sheets() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("blank.xlsx");
        create_blank(&path, &["Summary".to_string(), "Data".to_string()]).expect("create");

        let workbook: Xlsx<_> = open_workbook(&path).expect("open");
        assert_eq!(workbook.sheet_names(), vec!["Summary", "Data"]);
    }

    #[test]
    fn creates_workbook_from_csv() {
        let dir = tempdir().expect("tempdir");
        let csv = dir.path().join("data.csv");
        let out = dir.path().join("data.xlsx");
        fs::write(&csv, "name,value\nfoo,1\nbar,2\n").expect("write");

        create_from_csv(&csv, &out, "Import").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Import").expect("range");
        assert_eq!(
            range
                .get_value((0, 0))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("name".to_string())
        );
        assert_eq!(
            range
                .get_value((1, 0))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("foo".to_string())
        );
    }

    #[test]
    fn csv_quoted_fields_with_commas_are_single_cells() {
        // Regression test for Issue #24: split(',') would break "Smith, John"
        // into two cells. The csv crate handles RFC 4180 quoting correctly.
        let dir = tempdir().expect("tempdir");
        let csv = dir.path().join("quoted.csv");
        let out = dir.path().join("quoted.xlsx");
        fs::write(&csv, "name,city\n\"Smith, John\",\"New York\"\n").expect("write");

        create_from_csv(&csv, &out, "Data").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Data").expect("range");
        assert_eq!(
            range
                .get_value((1, 0))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("Smith, John".to_string()),
            "quoted field with comma should be a single cell"
        );
        assert_eq!(
            range
                .get_value((1, 1))
                .map(|cell: &calamine::Data| cell.to_string()),
            Some("New York".to_string())
        );
    }

    #[test]
    fn creates_workbook_from_markdown_table() {
        let dir = tempdir().expect("tempdir");
        let md = dir.path().join("table.md");
        let out = dir.path().join("table.xlsx");
        fs::write(
            &md,
            "| Name  | Score |\n| ----- | ----- |\n| Alice | 95    |\n| Bob   | 87    |\n",
        )
        .expect("write");

        create_from_markdown(&md, &out, "Data").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Data").expect("range");
        // Row 0: headers
        assert_eq!(
            range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("Name".to_string())
        );
        assert_eq!(
            range.get_value((0, 1)).map(|c: &calamine::Data| c.to_string()),
            Some("Score".to_string())
        );
        // Row 1: Alice, 95 (as number)
        assert_eq!(
            range.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("Alice".to_string())
        );
        assert_eq!(
            range.get_value((1, 1)).map(|c: &calamine::Data| c.to_string()),
            Some("95".to_string())
        );
    }

    #[test]
    fn markdown_missing_file_returns_error() {
        let dir = tempdir().expect("tempdir");
        let md = dir.path().join("nope.md");
        let out = dir.path().join("out.xlsx");
        let err = create_from_markdown(&md, &out, "Sheet1").expect_err("missing");
        assert!(matches!(err, xli_core::XliError::FileNotFound { .. }));
    }

    #[test]
    fn markdown_with_surrounding_text() {
        let dir = tempdir().expect("tempdir");
        let md = dir.path().join("doc.md");
        let out = dir.path().join("doc.xlsx");
        fs::write(
            &md,
            "# Report\n\nSome intro text.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.\n",
        )
        .expect("write");

        create_from_markdown(&md, &out, "Sheet1").expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Sheet1").expect("range");
        assert_eq!(
            range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("A".to_string())
        );
        // "1" should be written as number
        assert_eq!(
            range.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("1".to_string())
        );
    }

    #[test]
    fn creates_workbook_from_json_with_headers() {
        let dir = tempdir().expect("tempdir");
        let json = dir.path().join("data.json");
        let out = dir.path().join("data.xlsx");
        fs::write(
            &json,
            r#"{
                "sheets": {
                    "Summary": {
                        "headers": ["Name", "Score", "Grade"],
                        "rows": [
                            ["Alice", 95, "A"],
                            ["Bob", 87, "B+"]
                        ]
                    }
                }
            }"#,
        )
        .expect("write");

        let count = create_from_json(&json, &out).expect("create");
        assert_eq!(count, 1);

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Summary").expect("range");
        // Headers
        assert_eq!(
            range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("Name".to_string())
        );
        assert_eq!(
            range.get_value((0, 2)).map(|c: &calamine::Data| c.to_string()),
            Some("Grade".to_string())
        );
        // Data row 1
        assert_eq!(
            range.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("Alice".to_string())
        );
        assert_eq!(
            range.get_value((1, 1)).map(|c: &calamine::Data| c.to_string()),
            Some("95".to_string())
        );
        // Data row 2
        assert_eq!(
            range.get_value((2, 2)).map(|c: &calamine::Data| c.to_string()),
            Some("B+".to_string())
        );
    }

    #[test]
    fn creates_workbook_from_json_object_rows() {
        let dir = tempdir().expect("tempdir");
        let json = dir.path().join("obj.json");
        let out = dir.path().join("obj.xlsx");
        fs::write(
            &json,
            r#"{
                "sheets": {
                    "Sheet1": {
                        "rows": [
                            {"Name": "Alice", "Score": 95},
                            {"Name": "Bob", "Score": 87}
                        ]
                    }
                }
            }"#,
        )
        .expect("write");

        create_from_json(&json, &out).expect("create");

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let range = workbook.worksheet_range("Sheet1").expect("range");
        // Headers derived from keys of first object
        let h0 = range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()).unwrap();
        let h1 = range.get_value((0, 1)).map(|c: &calamine::Data| c.to_string()).unwrap();
        // Keys may be in any order, so just check both are present
        let mut headers = vec![h0, h1];
        headers.sort();
        assert_eq!(headers, vec!["Name", "Score"]);
        // Data should be in row 1
        // Find which column is "Name"
        let name_col = if range.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()) == Some("Name".to_string()) {
            0u16
        } else {
            1u16
        };
        assert_eq!(
            range.get_value((1, name_col as u32)).map(|c: &calamine::Data| c.to_string()),
            Some("Alice".to_string())
        );
    }

    #[test]
    fn creates_multi_sheet_from_json() {
        let dir = tempdir().expect("tempdir");
        let json = dir.path().join("multi.json");
        let out = dir.path().join("multi.xlsx");
        fs::write(
            &json,
            r#"{
                "sheets": {
                    "First": {
                        "headers": ["A"],
                        "rows": [["x"]]
                    },
                    "Second": {
                        "headers": ["B"],
                        "rows": [["y"]]
                    }
                }
            }"#,
        )
        .expect("write");

        let count = create_from_json(&json, &out).expect("create");
        assert_eq!(count, 2);

        let mut workbook: Xlsx<_> = open_workbook(&out).expect("open");
        let names = workbook.sheet_names().to_vec();
        assert!(names.contains(&"First".to_string()));
        assert!(names.contains(&"Second".to_string()));

        let r1 = workbook.worksheet_range("First").expect("range");
        assert_eq!(
            r1.get_value((0, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("A".to_string())
        );
        assert_eq!(
            r1.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("x".to_string())
        );

        let r2 = workbook.worksheet_range("Second").expect("range");
        assert_eq!(
            r2.get_value((1, 0)).map(|c: &calamine::Data| c.to_string()),
            Some("y".to_string())
        );
    }

    #[test]
    fn json_missing_file_returns_error() {
        let dir = tempdir().expect("tempdir");
        let json = dir.path().join("nope.json");
        let out = dir.path().join("out.xlsx");
        let err = create_from_json(&json, &out).expect_err("missing");
        assert!(matches!(err, xli_core::XliError::FileNotFound { .. }));
    }
}
