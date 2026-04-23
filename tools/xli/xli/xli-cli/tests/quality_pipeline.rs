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

fn create_workbook(path: &std::path::Path) {
    let out = xli(&["create", path.to_str().unwrap()]);
    assert!(out.status.success());
}

// ---------------------------------------------------------------------------
// schema — outputs raw JSON schema (not wrapped in a ResponseEnvelope)
// ---------------------------------------------------------------------------

#[test]
fn schema_returns_valid_json() {
    let out = xli(&["schema"]);
    assert!(out.status.success());
    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    // Schema output is a raw JSON schema object, not an envelope
    assert!(json.is_object(), "schema output should be a JSON object");
}

#[test]
fn schema_lists_commands() {
    let out = xli(&["schema"]);
    assert!(out.status.success());
    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert!(
        json["commands"].is_object(),
        "expected commands object in schema output"
    );
}

// ---------------------------------------------------------------------------
// lint
// ---------------------------------------------------------------------------

#[test]
fn lint_clean_workbook() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("clean.xlsx");
    create_workbook(&path);

    let json = xli_json(&["lint", path.to_str().unwrap()]);
    assert_eq!(json["status"], "ok");
}

#[test]
fn lint_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("does_not_exist.xlsx");

    let out = xli(&["lint", path.to_str().unwrap()]);
    if out.status.success() {
        let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
        assert_eq!(json["status"], "error");
    }
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

#[test]
fn validate_clean_workbook() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("valid.xlsx");
    create_workbook(&path);

    let write_out = xli(&["write", path.to_str().unwrap(), "Sheet1!A1", "--value", "hello"]);
    assert!(write_out.status.success());

    let json = xli_json(&["validate", path.to_str().unwrap()]);
    assert_eq!(json["status"], "ok");
}

#[test]
fn validate_missing_file_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("ghost.xlsx");

    let out = xli(&["validate", path.to_str().unwrap()]);
    if out.status.success() {
        let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
        assert_eq!(json["status"], "error");
    }
}

// ---------------------------------------------------------------------------
// doctor — may fail if LibreOffice is not installed; that's acceptable
// ---------------------------------------------------------------------------

#[test]
fn doctor_returns_json_envelope() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("checkup.xlsx");
    create_workbook(&path);

    let out = xli(&["doctor", path.to_str().unwrap()]);
    // Doctor may fail (exit 1) if LibreOffice is missing — that's ok.
    // The key contract is: stdout is always valid JSON with a command field.
    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json envelope");
    assert_eq!(json["command"], "doctor");
    assert!(
        json["status"] == "ok" || json["status"] == "error",
        "status must be ok or error"
    );
}
