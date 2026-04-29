mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

const SAMPLE: &str = "# Top\n\n## Intro\n\nIntro body.\n\n<!-- mdli:id v=1 id=top.work -->\n## Work\n\nWork body.\n\n### Sub A\n\nA.\n\n### Sub B\n\nB.\n\n<!-- mdli:begin v=1 id=top.work.gen checksum=sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 -->\n<!-- mdli:end v=1 id=top.work.gen -->\n\n## Tail\n\nEnd.\n";

#[test]
fn context_returns_selected_section_metadata() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, SAMPLE).unwrap();

    bin()
        .args(["context", path.to_str().unwrap(), "--id", "top.work"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"top.work\""))
        .stdout(predicate::str::contains("\"title\": \"Work\""))
        .stdout(predicate::str::contains("\"path\": \"Top > Work\""))
        .stdout(predicate::str::contains("\"truncated\": false"))
        .stdout(predicate::str::contains("\"line_range\""));
}

#[test]
fn context_includes_breadcrumbs_siblings_and_children() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, SAMPLE).unwrap();

    bin()
        .args(["context", path.to_str().unwrap(), "--id", "top.work"])
        .assert()
        .success()
        // Breadcrumb to the H1 ancestor.
        .stdout(predicate::str::contains("\"breadcrumbs\""))
        .stdout(predicate::str::contains("\"title\": \"Top\""))
        // Siblings on either side at the same level under the same parent.
        .stdout(predicate::str::contains("\"position\": \"before\""))
        .stdout(predicate::str::contains("\"position\": \"after\""))
        .stdout(predicate::str::contains("\"title\": \"Intro\""))
        .stdout(predicate::str::contains("\"title\": \"Tail\""))
        // Direct children only (level == target.level + 1).
        .stdout(predicate::str::contains("\"title\": \"Sub A\""))
        .stdout(predicate::str::contains("\"title\": \"Sub B\""));
}

#[test]
fn context_lists_managed_blocks_inside_section() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, SAMPLE).unwrap();

    bin()
        .args(["context", path.to_str().unwrap(), "--id", "top.work"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"managed_blocks\""))
        .stdout(predicate::str::contains("top.work.gen"))
        .stdout(predicate::str::contains("\"locked\": false"));
}

#[test]
fn context_truncates_body_at_line_boundary_when_over_budget() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    let mut body = String::from("<!-- mdli:id v=1 id=big -->\n# Big\n\n");
    for i in 0..200 {
        body.push_str(&format!("Body line {i} with extra padding for chars.\n"));
    }
    fs::write(&path, body).unwrap();

    bin()
        .args([
            "context",
            path.to_str().unwrap(),
            "--id",
            "big",
            "--max-tokens",
            "50",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"truncated\": true"))
        .stdout(predicate::str::contains("…"));
}

#[test]
fn context_resolves_path_selector_when_unambiguous() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, SAMPLE).unwrap();

    bin()
        .args(["context", path.to_str().unwrap(), "--path", "Top > Intro"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"Intro\""));
}

#[test]
fn context_errors_when_selector_missing() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, SAMPLE).unwrap();

    bin()
        .args(["context", path.to_str().unwrap(), "--id", "no.such.section"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_SELECTOR_NOT_FOUND"));
}
