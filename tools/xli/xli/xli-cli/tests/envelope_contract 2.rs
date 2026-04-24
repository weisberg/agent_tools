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
    serde_json::from_slice(&out.stdout).expect("valid json")
}

fn create_workbook(path: &std::path::Path) {
    let out = xli(&["create", path.to_str().unwrap()]);
    assert!(out.status.success());
}

/// Assert the envelope has all required fields
fn assert_envelope_structure(json: &Value) {
    assert!(json["status"].is_string(), "missing status field");
    assert!(json["command"].is_string(), "missing command field");
    assert!(json["commit_mode"].is_string(), "missing commit_mode field");
    assert!(
        json["needs_recalc"].is_boolean(),
        "missing needs_recalc field"
    );
    // warnings and errors should be arrays
    assert!(json["warnings"].is_array(), "warnings should be array");
    assert!(json["errors"].is_array(), "errors should be array");
}

#[test]
fn inspect_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&["inspect", path.to_str().unwrap()]);
    assert_envelope_structure(&json);
}

#[test]
fn create_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");

    let json = xli_json(&["create", path.to_str().unwrap()]);
    assert_envelope_structure(&json);
}

#[test]
fn write_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "42",
    ]);
    assert_envelope_structure(&json);
}

#[test]
fn format_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    xli(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "42",
    ]);

    let json = xli_json(&["format", path.to_str().unwrap(), "Sheet1!A1:A1", "--bold"]);
    assert_envelope_structure(&json);
}

#[test]
fn read_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&["read", path.to_str().unwrap(), "Sheet1!A1"]);
    assert_envelope_structure(&json);
}

#[test]
fn sheet_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&["sheet", path.to_str().unwrap(), "add", "NewSheet"]);
    assert_envelope_structure(&json);
}

#[test]
fn error_envelope_structure() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("does_not_exist.xlsx");

    let out = xli(&["inspect", missing.to_str().unwrap()]);
    assert!(!out.status.success());

    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(json["status"], "error", "status should be error");
    assert!(
        json["errors"].as_array().map_or(false, |a| !a.is_empty()),
        "errors array should be non-empty"
    );
    assert!(json["command"].is_string(), "command field should be set");
}

#[test]
fn write_includes_umya_warning() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "hello",
    ]);
    let warnings = json["warnings"].as_array().expect("warnings array");
    let has_umya_warning = warnings.iter().any(|w| {
        let msg = w.as_str().unwrap_or_default();
        msg.contains("umya")
    });
    assert!(
        has_umya_warning,
        "expected umya fallback warning in warnings array, got: {:?}",
        warnings
    );
}

#[test]
fn write_response_has_fingerprints() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "42",
    ]);

    let fp_before = json["fingerprint_before"]
        .as_str()
        .expect("fingerprint_before should be a string");
    let fp_after = json["fingerprint_after"]
        .as_str()
        .expect("fingerprint_after should be a string");

    assert!(
        fp_before.starts_with("sha256:"),
        "fingerprint_before should start with sha256:, got: {}",
        fp_before
    );
    assert!(
        fp_after.starts_with("sha256:"),
        "fingerprint_after should start with sha256:, got: {}",
        fp_after
    );
}

#[test]
fn create_has_commit_mode_none() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");

    let json = xli_json(&["create", path.to_str().unwrap()]);
    assert_eq!(
        json["commit_mode"], "none",
        "create should have commit_mode=none, got: {}",
        json["commit_mode"]
    );
}

#[test]
fn write_has_commit_mode_atomic() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "42",
    ]);
    assert_eq!(
        json["commit_mode"], "atomic",
        "write should have commit_mode=atomic, got: {}",
        json["commit_mode"]
    );
}

#[test]
fn human_mode_is_not_json() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let out = xli(&["inspect", path.to_str().unwrap(), "--human"]);
    assert!(out.status.success());

    let result: Result<Value, _> = serde_json::from_slice(&out.stdout);
    assert!(
        result.is_err(),
        "human mode output should NOT be valid JSON, but it parsed successfully"
    );
}
