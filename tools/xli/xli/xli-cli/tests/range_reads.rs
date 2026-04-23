use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};
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

fn create_and_populate(path: &std::path::Path) {
    let out = xli(&["create", path.to_str().unwrap()]);
    assert!(out.status.success());
    // Write 5 rows of data via batch
    let batch = r#"{"op":"write","address":"Sheet1!A1","value":"Name"}
{"op":"write","address":"Sheet1!B1","value":"Score"}
{"op":"write","address":"Sheet1!A2","value":"Alice"}
{"op":"write","address":"Sheet1!B2","value":95}
{"op":"write","address":"Sheet1!A3","value":"Bob"}
{"op":"write","address":"Sheet1!B3","value":87}
{"op":"write","address":"Sheet1!A4","value":"Carol"}
{"op":"write","address":"Sheet1!B4","value":92}
{"op":"write","address":"Sheet1!A5","value":"Dave"}
{"op":"write","address":"Sheet1!B5","value":78}"#;
    let mut child = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(["batch", path.to_str().unwrap(), "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("batch");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(batch.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
}

#[test]
fn read_single_cell() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let json = xli_json(&["read", path.to_str().unwrap(), "Sheet1!A2"]);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["value"], "Alice");
}

#[test]
fn read_range_returns_rows() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let json = xli_json(&["read", path.to_str().unwrap(), "Sheet1!A1:B5"]);
    assert_eq!(json["status"], "ok");
    let rows = json["output"]["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 5);
}

#[test]
fn read_range_with_headers() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let json = xli_json(&[
        "read",
        path.to_str().unwrap(),
        "Sheet1!A1:B5",
        "--headers",
    ]);
    assert_eq!(json["status"], "ok");
    let headers = json["output"]["headers"].as_array().expect("headers array");
    assert_eq!(headers, &[Value::String("Name".into()), Value::String("Score".into())]);
}

#[test]
fn read_range_with_limit() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let json = xli_json(&[
        "read",
        path.to_str().unwrap(),
        "Sheet1!A1:B5",
        "--limit",
        "2",
    ]);
    assert_eq!(json["status"], "ok");
    let rows = json["output"]["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 2);
    assert_eq!(json["output"]["truncated"], true);
}

#[test]
fn read_range_with_offset() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let json = xli_json(&[
        "read",
        path.to_str().unwrap(),
        "Sheet1!A1:B5",
        "--offset",
        "2",
        "--limit",
        "2",
    ]);
    assert_eq!(json["status"], "ok");
    let rows = json["output"]["rows"].as_array().expect("rows array");
    assert_eq!(rows.len(), 2);
    // Offset 2 skips first 2 rows (Name, Alice), returns Bob and Carol
    assert_eq!(rows[0]["A"], "Bob");
    assert_eq!(rows[1]["A"], "Carol");
}

#[test]
fn read_missing_cell_returns_null() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let json = xli_json(&["read", path.to_str().unwrap(), "Sheet1!Z99"]);
    assert_eq!(json["status"], "ok");
    assert!(json["output"]["value"].is_null());
}

#[test]
fn read_nonexistent_sheet_returns_error() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_and_populate(&path);

    let out = xli(&["read", path.to_str().unwrap(), "Missing!A1"]);
    let json: Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "SHEET_NOT_FOUND");
}
