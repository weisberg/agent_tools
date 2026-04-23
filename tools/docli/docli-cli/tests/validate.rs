//! Integration tests for the `validate` CLI command.

mod helpers;

use std::process::Command;

fn docli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_docli"))
}

#[test]
fn validate_simple_returns_valid_envelope() {
    let fixture = helpers::build_simple_docx();

    let output = docli()
        .args(["validate", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run docli");

    // validate may exit 0 or 1 depending on issues found; either way stdout must be valid JSON
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is not valid JSON");

    assert_eq!(json["command"], "validate");
    assert!(json["data"]["issues"].is_array(), "issues should be an array");
    assert!(json["data"]["error_count"].is_number(), "error_count should be a number");
    assert!(json["data"]["warning_count"].is_number(), "warning_count should be a number");
    assert!(json["elapsed_ms"].is_number(), "elapsed_ms should be present");
}

#[test]
fn validate_minimal_reports_issues() {
    // minimal.docx is in the workspace tests/fixtures directory
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/minimal.docx");

    if !fixture.exists() {
        eprintln!("skipping: minimal.docx not found at {:?}", fixture);
        return;
    }

    let output = docli()
        .args(["validate", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run docli");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is not valid JSON");

    assert_eq!(json["command"], "validate");
    assert!(json["data"]["issues"].is_array());
}

#[test]
fn validate_reviewed_returns_valid_envelope() {
    let fixture = helpers::build_reviewed_docx();

    let output = docli()
        .args(["validate", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run docli");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is not valid JSON");

    assert_eq!(json["command"], "validate");
    assert!(json["data"]["issues"].is_array());
}

#[test]
fn validate_missing_file_returns_error() {
    let output = docli()
        .args(["validate", "/nonexistent/path/missing.docx"])
        .output()
        .expect("failed to run docli");

    assert!(!output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("error output should be valid JSON");

    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "validate");
    assert!(json["error"]["message"].is_string());
}
