//! Integration test: review / finalize round-trip.
//!
//! Builds a fixture with tracked changes, runs `docli finalize accept` on it,
//! then inspects the result to verify the output is a valid DOCX.

mod helpers;

use std::process::Command;

fn docli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_docli"))
}

#[test]
fn finalize_accept_roundtrip() {
    let fixture = helpers::build_reviewed_docx();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("finalized.docx");

    // Run finalize accept (accept all tracked changes).
    let finalize_result = docli()
        .args([
            "finalize",
            "accept",
            "--in",
            fixture.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run docli finalize accept");

    assert!(
        finalize_result.status.success(),
        "finalize accept failed: stderr={}",
        String::from_utf8_lossy(&finalize_result.stderr)
    );

    let finalize_json: serde_json::Value = serde_json::from_slice(&finalize_result.stdout)
        .expect("finalize output is not valid JSON");
    assert_eq!(finalize_json["ok"], true);
    assert_eq!(finalize_json["data"]["operations"], 1);

    // Inspect the output to verify it is a valid DOCX.
    let inspect_result = docli()
        .args(["inspect", output.to_str().unwrap()])
        .output()
        .expect("failed to run docli inspect");

    assert!(
        inspect_result.status.success(),
        "inspect failed: stderr={}",
        String::from_utf8_lossy(&inspect_result.stderr)
    );

    let inspect_json: serde_json::Value =
        serde_json::from_slice(&inspect_result.stdout).expect("inspect output is not valid JSON");
    assert_eq!(inspect_json["ok"], true);

    // The reviewed fixture has tracked changes. After finalize, the output
    // should still be a valid document with paragraphs. (The actual accept
    // logic will be applied via pipeline hooks in the future; here we verify
    // structural integrity of the round-trip.)
    let paragraphs = inspect_json["data"]["paragraphs"]
        .as_array()
        .expect("paragraphs should be an array");
    assert!(
        !paragraphs.is_empty(),
        "output document should contain paragraphs"
    );
}

#[test]
fn finalize_accept_with_ids() {
    let fixture = helpers::build_reviewed_docx();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("finalized_ids.docx");

    let result = docli()
        .args([
            "finalize",
            "accept",
            "--in",
            fixture.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--ids",
            "2,3",
        ])
        .output()
        .expect("failed to run docli finalize accept --ids");

    assert!(
        result.status.success(),
        "finalize accept --ids failed: stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("output is not valid JSON");
    assert_eq!(json["ok"], true);
}

#[test]
fn finalize_strip_roundtrip() {
    let fixture = helpers::build_reviewed_docx();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("stripped.docx");

    let result = docli()
        .args([
            "finalize",
            "strip",
            "--in",
            fixture.to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run docli finalize strip");

    assert!(
        result.status.success(),
        "finalize strip failed: stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("output is not valid JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["operations"], 1);
}
