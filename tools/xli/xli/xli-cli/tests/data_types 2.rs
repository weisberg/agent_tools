use serde_json::Value;
use std::process::{Command, Output};
use tempfile::tempdir;

#[test]
fn write_and_read_integer() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    xli_json(&["write", p, "Sheet1!A1", "--value", "42"]);

    let read = xli_json(&["read", p, "Sheet1!A1"]);
    assert_eq!(read["output"]["value"], 42.0);
}

#[test]
fn write_and_read_float() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    xli_json(&["write", p, "Sheet1!A1", "--value", "3.14"]);

    let read = xli_json(&["read", p, "Sheet1!A1"]);
    assert_eq!(read["output"]["value"], 3.14);
}

#[test]
fn write_and_read_string() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    xli_json(&["write", p, "Sheet1!A1", "--value", "\"hello world\""]);

    let read = xli_json(&["read", p, "Sheet1!A1"]);
    assert_eq!(read["output"]["value"], "hello world");
}

#[test]
fn write_and_read_boolean_true() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    xli_json(&["write", p, "Sheet1!A1", "--value", "true"]);

    let read = xli_json(&["read", p, "Sheet1!A1"]);
    assert_eq!(read["output"]["value"], true);
}

#[test]
fn write_and_read_boolean_false() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    xli_json(&["write", p, "Sheet1!A1", "--value", "false"]);

    let read = xli_json(&["read", p, "Sheet1!A1"]);
    assert_eq!(read["output"]["value"], false);
}

#[test]
fn write_formula_sets_needs_recalc() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    let write = xli_json(&["write", p, "Sheet1!A1", "--formula", "=1+1"]);
    assert_eq!(write["needs_recalc"], true);
}

#[test]
fn write_value_does_not_set_needs_recalc() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();
    let write = xli_json(&["write", p, "Sheet1!A1", "--value", "42"]);
    assert_eq!(write["needs_recalc"], false);
}

#[test]
fn write_to_specific_sheet() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    let p = path.to_str().unwrap();

    let out = xli(&["create", p, "--sheets", "A,B"]);
    assert!(out.status.success());

    xli_json(&["write", p, "B!A1", "--value", "99"]);

    let read = xli_json(&["read", p, "B!A1"]);
    assert_eq!(read["output"]["value"], 99.0);
}

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
