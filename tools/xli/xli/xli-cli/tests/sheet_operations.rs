use serde_json::Value;
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

fn create_workbook(path: &std::path::Path, sheets: &str) {
    let out = xli(&["create", path.to_str().unwrap(), "--sheets", sheets]);
    assert!(out.status.success());
}

fn sheet_names(path: &std::path::Path) -> Vec<String> {
    let json = xli_json(&["inspect", path.to_str().unwrap()]);
    json["output"]["sheets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn sheet_copy() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "Data");

    let out = xli_json(&[
        "sheet",
        path.to_str().unwrap(),
        "copy",
        "Data",
        "--to",
        "DataBackup",
    ]);
    assert_eq!(out["status"], "ok");

    let names = sheet_names(&path);
    assert!(names.contains(&"Data".to_string()));
    assert!(names.contains(&"DataBackup".to_string()));
    assert_eq!(names.len(), 2);
}

#[test]
fn sheet_remove() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "A,B,C");

    let out = xli_json(&["sheet", path.to_str().unwrap(), "remove", "B"]);
    assert_eq!(out["status"], "ok");

    let names = sheet_names(&path);
    assert_eq!(names, vec!["A", "C"]);
}

#[test]
fn sheet_reorder() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "A,B,C");

    let out = xli_json(&[
        "sheet",
        path.to_str().unwrap(),
        "reorder",
        "--order",
        "C,A,B",
    ]);
    assert_eq!(out["status"], "ok");

    let names = sheet_names(&path);
    assert_eq!(names, vec!["C", "A", "B"]);
}

#[test]
fn sheet_add_after() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "A,B");

    let out = xli_json(&["sheet", path.to_str().unwrap(), "add", "X", "--after", "A"]);
    assert_eq!(out["status"], "ok");

    let names = sheet_names(&path);
    assert_eq!(names, vec!["A", "X", "B"]);
}

#[test]
fn sheet_unhide() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "A,B");

    let hide = xli_json(&["sheet", path.to_str().unwrap(), "hide", "B"]);
    assert_eq!(hide["status"], "ok");

    let unhide = xli_json(&["sheet", path.to_str().unwrap(), "unhide", "B"]);
    assert_eq!(unhide["status"], "ok");

    let inspect = xli_json(&["inspect", path.to_str().unwrap()]);
    let sheets = inspect["output"]["sheets"].as_array().unwrap();
    let sheet_b = sheets.iter().find(|s| s["name"] == "B").expect("sheet B");
    assert_ne!(sheet_b.get("hidden"), Some(&Value::Bool(true)));
}

#[test]
fn sheet_remove_nonexistent_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "A");

    let out = xli(&["sheet", path.to_str().unwrap(), "remove", "Missing"]);
    assert!(!out.status.success());

    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(json["status"], "error");
    // umya-spreadsheet returns WRITE_CONFLICT for missing sheet removal
    let code = json["errors"][0]["code"].as_str().unwrap();
    assert!(
        code == "SHEET_NOT_FOUND" || code == "WRITE_CONFLICT",
        "expected SHEET_NOT_FOUND or WRITE_CONFLICT, got {code}"
    );
}

#[test]
fn sheet_rename() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path, "OldName");

    let out = xli_json(&[
        "sheet",
        path.to_str().unwrap(),
        "rename",
        "OldName",
        "--to",
        "NewName",
    ]);
    assert_eq!(out["status"], "ok");

    let names = sheet_names(&path);
    assert!(!names.contains(&"OldName".to_string()));
    assert!(names.contains(&"NewName".to_string()));
}
