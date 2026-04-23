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

#[test]
fn sequential_writes_all_succeed() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();

    for i in 1..=10 {
        let cell = format!("Sheet1!A{}", i);
        let value = format!("{}", i * 10);
        let write = xli_json(&["write", p, &cell, "--value", &value]);
        assert_eq!(write["status"], "ok");
    }

    for i in 1..=10 {
        let cell = format!("Sheet1!A{}", i);
        let read = xli_json(&["read", p, &cell]);
        assert_eq!(read["status"], "ok");
        assert_eq!(
            read["output"]["value"],
            (i * 10) as f64,
            "A{} should be {}",
            i,
            i * 10
        );
    }
}

#[test]
fn fingerprint_tracks_each_mutation() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();

    let w1 = xli_json(&["write", p, "Sheet1!A1", "--value", "1"]);
    let fp1 = w1["fingerprint_after"].as_str().unwrap().to_string();

    let w2 = xli_json(&["write", p, "Sheet1!A2", "--value", "2"]);
    let fp2 = w2["fingerprint_after"].as_str().unwrap().to_string();

    let w3 = xli_json(&["write", p, "Sheet1!A3", "--value", "3"]);
    let fp3 = w3["fingerprint_after"].as_str().unwrap().to_string();

    assert_ne!(fp1, fp2, "fingerprint must change between write A1 and A2");
    assert_ne!(fp2, fp3, "fingerprint must change between write A2 and A3");
    assert_ne!(fp1, fp3, "fingerprint must change between write A1 and A3");
}

#[test]
fn write_after_inspect_preserves_data() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();

    let w1 = xli_json(&["write", p, "Sheet1!A1", "--value", "100"]);
    assert_eq!(w1["status"], "ok");

    // inspect is read-only; it should not corrupt the workbook
    let inspect = xli_json(&["inspect", p]);
    assert_eq!(inspect["status"], "ok");

    let w2 = xli_json(&["write", p, "Sheet1!B1", "--value", "200"]);
    assert_eq!(w2["status"], "ok");

    let read_a1 = xli_json(&["read", p, "Sheet1!A1"]);
    assert_eq!(read_a1["output"]["value"], 100.0, "A1 should still be 100");

    let read_b1 = xli_json(&["read", p, "Sheet1!B1"]);
    assert_eq!(read_b1["output"]["value"], 200.0, "B1 should be 200");
}

#[test]
fn multiple_format_passes_dont_corrupt() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let p = path.to_str().unwrap();

    let write = xli_json(&["write", p, "Sheet1!A1", "--value", "test"]);
    assert_eq!(write["status"], "ok");

    let fmt_bold = xli_json(&["format", p, "Sheet1!A1:A1", "--bold"]);
    assert_eq!(fmt_bold["status"], "ok");

    let fmt_italic = xli_json(&["format", p, "Sheet1!A1:A1", "--italic"]);
    assert_eq!(fmt_italic["status"], "ok");

    let fmt_fill = xli_json(&["format", p, "Sheet1!A1:A1", "--fill", "FF0000"]);
    assert_eq!(fmt_fill["status"], "ok");

    // Workbook must still be valid after repeated format mutations
    let inspect = xli_json(&["inspect", p]);
    assert_eq!(inspect["status"], "ok");
}
