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

fn get_fingerprint(path: &std::path::Path) -> String {
    let json = xli_json(&["inspect", path.to_str().unwrap()]);
    json["output"]["fingerprint"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn fingerprint_changes_after_write() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let fp1 = get_fingerprint(&path);

    let write = xli_json(&["write", path.to_str().unwrap(), "Sheet1!A1", "--value", "42"]);
    assert_eq!(write["status"], "ok");

    let fp2 = get_fingerprint(&path);
    assert_ne!(fp1, fp2, "fingerprint must change after a write");
}

#[test]
fn write_with_correct_fingerprint_succeeds() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let fp = get_fingerprint(&path);

    let write = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "hello",
        "--expect-fingerprint",
        &fp,
    ]);
    assert_eq!(write["status"], "ok");
}

#[test]
fn write_with_wrong_fingerprint_fails() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let out = xli(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "hello",
        "--expect-fingerprint",
        "sha256:0000",
    ]);

    assert!(!out.status.success());
    let json: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FINGERPRINT_MISMATCH");
}

#[test]
fn write_dry_run_does_not_change_file() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let fp_before = get_fingerprint(&path);

    let write = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "99",
        "--dry-run",
    ]);
    assert_eq!(write["status"], "ok");

    let fp_after = get_fingerprint(&path);
    assert_eq!(
        fp_before, fp_after,
        "dry-run must not change the file on disk"
    );
}

#[test]
fn write_dry_run_returns_hypothetical_fingerprint() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let write = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "99",
        "--dry-run",
    ]);
    assert_eq!(write["status"], "ok");
    assert_eq!(write["commit_mode"], "dry_run");
    assert!(
        write["fingerprint_before"].is_string(),
        "response must include fingerprint_before"
    );
    assert!(
        write["fingerprint_after"].is_string(),
        "response must include fingerprint_after"
    );
}

#[test]
fn format_dry_run_preserves_workbook() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    // Write a value so the format has a target cell.
    xli_json(&["write", path.to_str().unwrap(), "Sheet1!A1", "--value", "hello"]);

    let fp_before = get_fingerprint(&path);

    let format = xli_json(&[
        "format",
        path.to_str().unwrap(),
        "Sheet1!A1:A1",
        "--bold",
        "--dry-run",
    ]);
    assert_eq!(format["status"], "ok");

    let fp_after = get_fingerprint(&path);
    assert_eq!(
        fp_before, fp_after,
        "format --dry-run must not change the file on disk"
    );
}

#[test]
fn batch_dry_run_reports_ops_without_committing() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let fp_before = get_fingerprint(&path);

    let batch_input = r#"{"op":"write","address":"Sheet1!A1","value":100}
{"op":"write","address":"Sheet1!A2","value":200}"#;

    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args([
            "batch",
            path.to_str().unwrap(),
            "--stdin",
            "--dry-run",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(batch_input.as_bytes())?;
            child.wait_with_output()
        })
        .expect("batch");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(json["status"], "ok");

    let fp_after = get_fingerprint(&path);
    assert_eq!(
        fp_before, fp_after,
        "batch --dry-run must not change the file on disk"
    );
}

#[test]
fn fingerprint_in_response_envelope() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let write = xli_json(&[
        "write",
        path.to_str().unwrap(),
        "Sheet1!A1",
        "--value",
        "42",
    ]);
    assert_eq!(write["status"], "ok");
    assert!(
        write["fingerprint_before"].is_string(),
        "response must include fingerprint_before"
    );
    assert!(
        write["fingerprint_after"].is_string(),
        "response must include fingerprint_after"
    );
    assert_ne!(
        write["fingerprint_before"], write["fingerprint_after"],
        "fingerprint_before and fingerprint_after should differ after a real write"
    );
}
