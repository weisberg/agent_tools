//! Integration tests for the `inspect` CLI command.

mod helpers;

use std::process::Command;

fn docli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_docli"))
}

#[test]
fn inspect_simple_returns_valid_json() {
    let fixture = helpers::build_simple_docx();

    let output = docli()
        .args(["inspect", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run docli");

    assert!(output.status.success(), "exit code was not 0: stderr={}", String::from_utf8_lossy(&output.stderr));

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is not valid JSON");

    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "inspect");
    assert!(json["data"]["paragraphs"].is_array(), "paragraphs should be an array");
    assert!(json["elapsed_ms"].is_number(), "elapsed_ms should be a number");

    // Verify paragraph count: 4 body paragraphs + 4 table-cell paragraphs = 8
    let paragraphs = json["data"]["paragraphs"].as_array().unwrap();
    assert!(paragraphs.len() >= 4, "expected at least 4 paragraphs, got {}", paragraphs.len());

    // Verify headings
    let headings = json["data"]["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 2, "expected 2 headings");
    assert_eq!(headings[0]["text"], "Introduction");
    assert_eq!(headings[0]["level"], 1);
    assert_eq!(headings[1]["text"], "Details");
    assert_eq!(headings[1]["level"], 2);

    // Verify table
    let tables = json["data"]["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 1, "expected 1 table");
    assert_eq!(tables[0]["rows"], 2);
    assert_eq!(tables[0]["cols"], 2);

    // Verify bookmark
    let bookmarks = &json["data"]["bookmarks"];
    assert!(bookmarks.is_object(), "bookmarks should be an object");
    assert!(bookmarks["important_section"].is_number(), "bookmark 'important_section' should exist");
}

#[test]
fn inspect_with_sections_filter() {
    let fixture = helpers::build_simple_docx();

    let output = docli()
        .args(["inspect", fixture.to_str().unwrap(), "--sections", "headings,tables"])
        .output()
        .expect("failed to run docli");

    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);

    // Only requested sections should be present
    assert!(json["data"]["headings"].is_array());
    assert!(json["data"]["tables"].is_array());

    // Non-requested sections should be absent (null in JSON)
    assert!(json["data"]["paragraphs"].is_null(), "paragraphs should not be present");
    assert!(json["data"]["images"].is_null(), "images should not be present");
    assert!(json["data"]["comments"].is_null(), "comments should not be present");
}

#[test]
fn inspect_reviewed_shows_comments_and_tracked_changes() {
    let fixture = helpers::build_reviewed_docx();

    let output = docli()
        .args(["inspect", fixture.to_str().unwrap()])
        .output()
        .expect("failed to run docli");

    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);

    // Verify comments detected
    let comments = &json["data"]["comments"];
    assert!(comments["count"].as_u64().unwrap() >= 1, "expected at least 1 comment");

    // Verify tracked changes detected
    let tc = &json["data"]["tracked_changes"];
    assert!(tc["count"].as_u64().unwrap() >= 2, "expected at least 2 tracked changes");
    assert!(tc["insertions"].as_u64().unwrap() >= 1, "expected at least 1 insertion");
    assert!(tc["deletions"].as_u64().unwrap() >= 1, "expected at least 1 deletion");

    // Verify authors
    let authors = tc["authors"].as_array().unwrap();
    let author_names: Vec<&str> = authors.iter().map(|a| a.as_str().unwrap()).collect();
    assert!(author_names.contains(&"Alice"), "Alice should be an author");
    assert!(author_names.contains(&"Bob"), "Bob should be an author");
}

#[test]
fn inspect_missing_file_returns_error_envelope() {
    let output = docli()
        .args(["inspect", "/nonexistent/path/missing.docx"])
        .output()
        .expect("failed to run docli");

    assert!(!output.status.success(), "should exit with non-zero status");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("error output should be valid JSON");

    assert_eq!(json["ok"], false);
    assert_eq!(json["command"], "inspect");
    assert!(json["error"].is_object(), "error field should be present");
    assert!(json["error"]["message"].is_string(), "error.message should be a string");
}
