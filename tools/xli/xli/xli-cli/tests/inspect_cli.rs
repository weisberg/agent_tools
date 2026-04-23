use rust_xlsxwriter::Workbook;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn inspect_returns_json_envelope() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("simple.xlsx");
    create_fixture(&path);

    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(["inspect", path.to_str().expect("path")])
        .output()
        .expect("run inspect");

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid json output");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["sheets"][0]["name"], "Summary");
}

#[test]
fn inspect_missing_file_returns_structured_error() {
    let dir = tempdir().expect("tempdir");
    let missing = dir.path().join("missing.xlsx");

    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(["inspect", missing.to_str().expect("path")])
        .output()
        .expect("run inspect");

    // Error envelope → non-zero exit code. (Issue #26)
    assert!(!output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid json output");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FILE_NOT_FOUND");
}

#[test]
fn clap_parse_errors_are_emitted_as_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(["inspect", "--badarg"])
        .output()
        .expect("run inspect");

    assert_eq!(output.status.code(), Some(2));
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid json output");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "CLI_PARSE_ERROR");
}

fn create_fixture(path: &Path) {
    let mut workbook = Workbook::new();

    let summary = workbook.add_worksheet();
    summary.set_name("Summary").expect("name");
    summary.write_string(0, 0, "Metric").expect("write");
    summary.write_number(0, 1, 42.0).expect("write");
    summary.write_formula(1, 1, "=SUM(B1:B1)").expect("write");

    let raw = workbook.add_worksheet();
    raw.set_name("Raw Data").expect("name");
    raw.write_string(0, 0, "Value").expect("write");

    workbook.save(path).expect("save");
}
