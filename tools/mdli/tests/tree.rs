mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn tree_emits_nested_heading_hierarchy() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n## A\n### A1\n### A2\n## B\n").unwrap();

    bin()
        .args(["tree", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"Top\""))
        .stdout(predicate::str::contains("\"title\": \"A\""))
        .stdout(predicate::str::contains("\"title\": \"A1\""))
        .stdout(predicate::str::contains("\"title\": \"A2\""))
        .stdout(predicate::str::contains("\"title\": \"B\""));
}

#[test]
fn tree_skipped_levels_attach_to_nearest_ancestor() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    // H1 then H3 (skipping H2). The H3 should still appear as a descendant.
    fs::write(&path, "# Top\n### Deep\n").unwrap();

    bin()
        .args(["tree", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\": \"Top\""))
        .stdout(predicate::str::contains("\"title\": \"Deep\""));
}

#[test]
fn tree_includes_stable_id_when_present() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "<!-- mdli:id v=1 id=top.id -->\n# Top\n").unwrap();

    bin()
        .args(["tree", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"top.id\""));
}

#[test]
fn tree_handles_empty_document() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "").unwrap();

    bin()
        .args(["tree", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tree\": []"));
}
