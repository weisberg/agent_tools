use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn bin() -> Command {
    Command::cargo_bin("mdli").expect("mdli binary")
}

#[test]
fn table_fmt_matches_golden_fixture() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("table.md");
    fs::copy("tests/fixtures/table_uncanonical.md", &path).unwrap();

    bin()
        .args([
            "table",
            "fmt",
            path.to_str().unwrap(),
            "--all",
            "--emit",
            "document",
        ])
        .assert()
        .success()
        .stdout(fs::read_to_string("tests/golden/table_canonical.md").unwrap());
}

#[test]
fn document_io_preserves_bom_and_crlf_on_mutation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("crlf.md");
    fs::write(
        &path,
        b"\xef\xbb\xbf# Report\r\n\r\n<!-- mdli:id v=1 id=report.analytics -->\r\n## Analytics\r\n",
    )
    .unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "report.summary",
            "--path",
            "Report > Summary",
            "--level",
            "2",
            "--write",
        ])
        .assert()
        .success();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"\xef\xbb\xbf"));
    assert!(String::from_utf8(bytes)
        .unwrap()
        .contains("\r\n## Summary\r\n"));
}

#[test]
fn lint_reports_malformed_table_fixture() {
    bin()
        .args(["--json", "lint", "tests/fixtures/malformed_table.md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("valid-tables"))
        .stdout(predicate::str::contains("E_TABLE_INVALID"));
}

#[test]
fn duplicate_path_selector_is_ambiguous() {
    bin()
        .args([
            "section",
            "get",
            "tests/fixtures/duplicate_paths.md",
            "--path",
            "Report > Analytics",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_AMBIGUOUS_SELECTOR"));
}
