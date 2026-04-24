use serde_json::Value;
use std::process::{Command, Output};
use tempfile::tempdir;

fn xli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(args)
        .output()
        .expect("xli command")
}

fn xli_error(args: &[&str]) -> Value {
    let out = xli(args);
    assert!(!out.status.success(), "expected failure but got success");
    serde_json::from_slice(&out.stdout).expect("valid json error envelope")
}

fn create_workbook(path: &std::path::Path) {
    let out = xli(&["create", path.to_str().unwrap()]);
    assert!(out.status.success());
}

// ---------------------------------------------------------------------------
// 1. write to nonexistent xlsx
// ---------------------------------------------------------------------------
#[test]
fn write_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.xlsx");

    let json = xli_error(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "hello",
    ]);
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FILE_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// 2. read from nonexistent xlsx
// ---------------------------------------------------------------------------
#[test]
fn read_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.xlsx");

    let json = xli_error(&["read", path.to_str().unwrap(), "Sheet1!A1"]);
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FILE_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// 3. format on nonexistent xlsx
// ---------------------------------------------------------------------------
#[test]
fn format_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.xlsx");

    let json = xli_error(&["format", path.to_str().unwrap(), "Sheet1!A1:A1", "--bold"]);
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FILE_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// 4. sheet add on nonexistent xlsx
// ---------------------------------------------------------------------------
#[test]
fn sheet_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.xlsx");

    let json = xli_error(&["sheet", path.to_str().unwrap(), "add", "NewSheet"]);
    assert_eq!(json["status"], "error");
}

// ---------------------------------------------------------------------------
// 5. write to nonexistent sheet
// ---------------------------------------------------------------------------
#[test]
fn write_to_nonexistent_sheet_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_error(&[
        "write",
        path.to_str().unwrap(),
        "NoSheet!A1",
        "--value",
        "hello",
    ]);
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "SHEET_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// 6. read from nonexistent sheet
// ---------------------------------------------------------------------------
#[test]
fn read_from_nonexistent_sheet_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_error(&["read", path.to_str().unwrap(), "NoSheet!A1"]);
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "SHEET_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// 7. error envelope structure: status, command, errors array
// ---------------------------------------------------------------------------
#[test]
fn error_envelope_always_has_status_and_command() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("nonexistent.xlsx");
    let existing = dir.path().join("test.xlsx");
    create_workbook(&existing);

    let cases: Vec<Value> = vec![
        // write missing file
        xli_error(&[
            "write",
            missing.to_str().unwrap(),
            "Sheet1!A1",
            "--value",
            "x",
        ]),
        // read missing file
        xli_error(&["read", missing.to_str().unwrap(), "Sheet1!A1"]),
        // format missing file
        xli_error(&[
            "format",
            missing.to_str().unwrap(),
            "Sheet1!A1:A1",
            "--bold",
        ]),
        // sheet add missing file
        xli_error(&["sheet", missing.to_str().unwrap(), "add", "X"]),
        // write to nonexistent sheet
        xli_error(&[
            "write",
            existing.to_str().unwrap(),
            "NoSheet!A1",
            "--value",
            "x",
        ]),
        // read from nonexistent sheet
        xli_error(&["read", existing.to_str().unwrap(), "NoSheet!A1"]),
    ];

    for (i, json) in cases.iter().enumerate() {
        assert_eq!(
            json["status"], "error",
            "case {i}: status should be \"error\""
        );
        assert!(
            json["command"].as_str().map_or(false, |s| !s.is_empty()),
            "case {i}: command should be a non-empty string"
        );
        let errors = json["errors"]
            .as_array()
            .unwrap_or_else(|| panic!("case {i}: errors should be an array"));
        assert!(
            !errors.is_empty(),
            "case {i}: errors array should be non-empty"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. error exit code is nonzero (specifically 1)
// ---------------------------------------------------------------------------
#[test]
fn error_exit_code_is_nonzero() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.xlsx");

    let out = xli(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "hello",
    ]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(1));
}

// ---------------------------------------------------------------------------
// 9. clap parse error exits with code 2 and CLI_PARSE_ERROR
// ---------------------------------------------------------------------------
#[test]
fn clap_error_exit_code_is_2() {
    let out = xli(&[]);
    assert!(!out.status.success());
    assert_eq!(
        out.status.code(),
        Some(2),
        "clap parse errors should exit with code 2"
    );

    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json for clap error");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "CLI_PARSE_ERROR");
}
