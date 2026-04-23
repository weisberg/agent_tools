use serde_json::Value;
use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;

fn xli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(args)
        .output()
        .expect("xli command")
}

fn xli_json(args: &[&str]) -> Value {
    let out = xli(args);
    assert!(
        out.status.success(),
        "xli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("valid json")
}

#[test]
fn csv_simple_import() {
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("data.csv");
    let xlsx_path = dir.path().join("output.xlsx");

    fs::write(&csv_path, "name,age\nAlice,30\nBob,25\n").expect("write csv");

    let create = xli_json(&[
        "create",
        xlsx_path.to_str().expect("path"),
        "--from-csv",
        csv_path.to_str().expect("csv path"),
    ]);
    assert_eq!(create["status"], "ok");

    // Inspect: 1 sheet
    let inspect = xli_json(&["inspect", xlsx_path.to_str().expect("path")]);
    assert_eq!(inspect["status"], "ok");
    let sheets = inspect["output"]["sheets"].as_array().expect("sheets array");
    assert_eq!(sheets.len(), 1);

    // Read range: 3 rows (header + 2 data)
    let read = xli_json(&["read", xlsx_path.to_str().expect("path"), "Sheet1!A1:B3"]);
    assert_eq!(read["status"], "ok");
    let rows = read["output"]["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 3);

    // Verify header row
    assert_eq!(rows[0]["A"], "name");
    assert_eq!(rows[0]["B"], "age");

    // Verify data rows
    assert_eq!(rows[1]["A"], "Alice");
    assert_eq!(rows[2]["A"], "Bob");
}

#[test]
fn csv_quoted_fields_with_commas() {
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("data.csv");
    let xlsx_path = dir.path().join("output.xlsx");

    fs::write(&csv_path, "name,city\n\"Smith, John\",\"New York\"\n").expect("write csv");

    let create = xli_json(&[
        "create",
        xlsx_path.to_str().expect("path"),
        "--from-csv",
        csv_path.to_str().expect("csv path"),
    ]);
    assert_eq!(create["status"], "ok");

    // Read cell A2 — should contain the full "Smith, John", not split on the comma
    let read = xli_json(&["read", xlsx_path.to_str().expect("path"), "Sheet1!A2"]);
    assert_eq!(read["status"], "ok");
    assert_eq!(read["output"]["value"], "Smith, John");
}

#[test]
fn csv_empty_file() {
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("empty.csv");
    let xlsx_path = dir.path().join("output.xlsx");

    fs::write(&csv_path, "").expect("write csv");

    let create = xli_json(&[
        "create",
        xlsx_path.to_str().expect("path"),
        "--from-csv",
        csv_path.to_str().expect("csv path"),
    ]);
    assert_eq!(create["status"], "ok");

    // Inspect: sheet exists with 0 rows
    let inspect = xli_json(&["inspect", xlsx_path.to_str().expect("path")]);
    assert_eq!(inspect["status"], "ok");
    let sheets = inspect["output"]["sheets"].as_array().expect("sheets array");
    assert!(!sheets.is_empty(), "sheet should exist");
    assert_eq!(sheets[0]["rows"], 0);
}

#[test]
fn csv_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("nonexistent.csv");
    let xlsx_path = dir.path().join("output.xlsx");

    let out = xli(&[
        "create",
        xlsx_path.to_str().expect("path"),
        "--from-csv",
        csv_path.to_str().expect("csv path"),
    ]);

    assert!(!out.status.success());
    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FILE_NOT_FOUND");
}

#[test]
fn csv_single_column() {
    let dir = tempdir().expect("tempdir");
    let csv_path = dir.path().join("data.csv");
    let xlsx_path = dir.path().join("output.xlsx");

    fs::write(&csv_path, "value\n1\n2\n3\n").expect("write csv");

    let create = xli_json(&[
        "create",
        xlsx_path.to_str().expect("path"),
        "--from-csv",
        csv_path.to_str().expect("csv path"),
    ]);
    assert_eq!(create["status"], "ok");

    // Read range: 4 rows (header + 3 data)
    let read = xli_json(&["read", xlsx_path.to_str().expect("path"), "Sheet1!A1:A4"]);
    assert_eq!(read["status"], "ok");
    let rows = read["output"]["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 4);

    // Verify header
    assert_eq!(rows[0]["A"], "value");
}
