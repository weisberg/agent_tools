//! Fixture-driven round-trip and idempotency tests.
//!
//! Each test in this file exercises a documented PRD edge case from
//! `mdli-prd-final.md` section 30.1 (fixture corpus).

mod common;

use common::bin;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn copy_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let src = fixtures_dir().join(name);
    let dir = tempdir().unwrap();
    let dst = dir.path().join(name);
    fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {name}: {e}"));
    (dir, dst)
}

// ---------------------------------------------------------------------------
// Round-trip: fmt --all on each fixture is idempotent.
//
// We run `table fmt --all --emit document` twice and assert the second run
// produces zero diff against the first. This is the PRD's "zero-diff after
// canonicalization" contract from section 11.4.
// ---------------------------------------------------------------------------

fn assert_zero_diff_idempotent(name: &str) {
    let (_dir, path) = copy_fixture(name);
    let path_str = path.to_str().unwrap();

    let first = bin()
        .args([
            "table",
            "fmt",
            path_str,
            "--all",
            "--emit",
            "document",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let tmp = path.with_extension("md.canonical");
    fs::write(&tmp, &first).unwrap();

    let second = bin()
        .args([
            "table",
            "fmt",
            tmp.to_str().unwrap(),
            "--all",
            "--emit",
            "document",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(
        first, second,
        "fixture {name} does not zero-diff after canonicalization"
    );
}

#[test]
fn duplicate_headings_round_trip() {
    assert_zero_diff_idempotent("duplicate-headings.md");
}

#[test]
fn escaped_gt_round_trip() {
    assert_zero_diff_idempotent("escaped-gt.md");
}

#[test]
fn unicode_headings_round_trip() {
    assert_zero_diff_idempotent("unicode-headings.md");
}

#[test]
fn empty_round_trip() {
    assert_zero_diff_idempotent("empty.md");
}

#[test]
fn no_h1_round_trip() {
    assert_zero_diff_idempotent("no-h1.md");
}

#[test]
fn nested_sections_round_trip() {
    assert_zero_diff_idempotent("nested-sections.md");
}

#[test]
fn code_fence_content_round_trip() {
    assert_zero_diff_idempotent("code-fence-content.md");
}

#[test]
fn table_with_pipes_round_trip() {
    assert_zero_diff_idempotent("table-with-pipes.md");
}

#[test]
fn yaml_frontmatter_round_trip() {
    assert_zero_diff_idempotent("yaml-frontmatter.md");
}

#[test]
fn toml_frontmatter_round_trip() {
    assert_zero_diff_idempotent("toml-frontmatter.md");
}

#[test]
fn inline_html_round_trip() {
    assert_zero_diff_idempotent("inline-html.md");
}

#[test]
fn cash_plus_mini_round_trip() {
    assert_zero_diff_idempotent("cash-plus-mini.md");
}

// ---------------------------------------------------------------------------
// Selector and lint behavior on fixtures
// ---------------------------------------------------------------------------

#[test]
fn duplicate_headings_lint_warns() {
    let (_dir, path) = copy_fixture("duplicate-headings.md");
    bin()
        .args(["--json", "lint", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("no-duplicate-headings"));
}

#[test]
fn duplicate_path_selector_is_ambiguous() {
    let (_dir, path) = copy_fixture("duplicate-headings.md");
    bin()
        .args([
            "section",
            "get",
            path.to_str().unwrap(),
            "--path",
            "Top > Same",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_AMBIGUOUS_SELECTOR"));
}

#[test]
fn escaped_gt_path_selector_resolves() {
    let (_dir, path) = copy_fixture("escaped-gt.md");
    bin()
        .args([
            "section",
            "get",
            path.to_str().unwrap(),
            "--path",
            "Top > A \\> B",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("## A \\> B"));
}

#[test]
fn no_h1_inspect_lists_top_level_h2s() {
    let (_dir, path) = copy_fixture("no-h1.md");
    bin()
        .args(["inspect", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Section A"))
        .stdout(predicate::str::contains("Section B"));
}

#[test]
fn malformed_table_lint_errors() {
    let (_dir, path) = copy_fixture("malformed-table.md");
    bin()
        .args(["--json", "lint", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("E_TABLE_INVALID"));
}

#[test]
fn locked_block_replace_is_blocked() {
    let (_dir, path) = copy_fixture("locked-block.md");
    let body = path.parent().unwrap().join("body.md");
    fs::write(&body, "Should not pass.\n").unwrap();
    bin()
        .args([
            "block",
            "replace",
            path.to_str().unwrap(),
            "--id",
            "section.gen",
            "--body-from-file",
            body.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_BLOCK_LOCKED"));
}

#[test]
fn tampered_block_replace_fails_by_default() {
    let (_dir, path) = copy_fixture("tampered-block.md");
    let body = path.parent().unwrap().join("body.md");
    fs::write(&body, "Replacement.\n").unwrap();
    bin()
        .args([
            "block",
            "replace",
            path.to_str().unwrap(),
            "--id",
            "section.gen",
            "--body-from-file",
            body.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_BLOCK_MODIFIED"));
}

#[test]
fn orphan_id_marker_lint_errors() {
    let (_dir, path) = copy_fixture("orphan-id-marker.md");
    bin()
        .args(["--json", "lint", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("stable-id-binding"));
}

#[test]
fn newer_marker_version_lint_warns() {
    let (_dir, path) = copy_fixture("newer-marker.md");
    bin()
        .args(["--json", "lint", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("wire-format"));
}

#[test]
fn yaml_frontmatter_get_returns_keys() {
    let (_dir, path) = copy_fixture("yaml-frontmatter.md");
    bin()
        .args(["frontmatter", "get", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("title"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn toml_frontmatter_get_returns_keys() {
    let (_dir, path) = copy_fixture("toml-frontmatter.md");
    bin()
        .args(["frontmatter", "get", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("title"));
}

// ---------------------------------------------------------------------------
// CRLF and UTF-8 BOM are generated inline so source-control normalization
// doesn't strip them.
// ---------------------------------------------------------------------------

#[test]
fn crlf_line_endings_preserved_through_inspect() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("crlf.md");
    fs::write(&path, "# Title\r\n\r\n## Sub\r\n\r\nBody.\r\n").unwrap();

    bin()
        .args(["inspect", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Title"))
        .stdout(predicate::str::contains("Sub"));
}

#[test]
fn crlf_round_trip_preserves_line_endings() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("crlf.md");
    let original = b"# CRLF\r\n\r\n## Sub\r\n\r\nBody.\r\n";
    fs::write(&path, original).unwrap();

    bin()
        .args([
            "section",
            "rename",
            path.to_str().unwrap(),
            "--id",
            "missing.id",
            "--to",
            "X",
        ])
        .assert()
        .failure();

    let after = fs::read(&path).unwrap();
    assert_eq!(
        original.to_vec(),
        after,
        "failed mutation must not change the file"
    );

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "crlf.sub",
            "--path",
            "CRLF > Sub",
            "--level",
            "2",
            "--write",
        ])
        .assert()
        .success();

    let after_write = fs::read(&path).unwrap();
    let crlf_count = after_write.windows(2).filter(|w| w == b"\r\n").count();
    assert!(
        crlf_count > 0,
        "writes must preserve the dominant CRLF line ending"
    );
}

#[test]
fn utf8_bom_round_trip_preserves_bom() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bom.md");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(b"# Title\n\n## Sub\n\nBody.\n");
    fs::write(&path, &bytes).unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "bom.sub",
            "--path",
            "Title > Sub",
            "--level",
            "2",
            "--write",
        ])
        .assert()
        .success();

    let after = fs::read(&path).unwrap();
    assert_eq!(
        &after[..3],
        &[0xEF, 0xBB, 0xBF],
        "BOM must be preserved across writes"
    );
}
