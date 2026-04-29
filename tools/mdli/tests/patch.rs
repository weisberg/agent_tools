mod common;

use common::bin;
use std::fs;
use tempfile::tempdir;

#[test]
fn patch_applies_section_and_table_edits_atomically() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    let patch = dir.path().join("patch.json");
    fs::write(&report, "# Report\n").unwrap();
    fs::write(&rows, "{\"key\":\"CP-1\",\"summary\":\"Patched\"}\n").unwrap();
    fs::write(
        &patch,
        format!(
            r#"[
    {{"op":"ensure_section","id":"report.analytics","path":"Report > Analytics","level":2}},
    {{"op":"replace_table","section":"report.analytics","name":"analytics-tickets","columns":["Ticket=key","Summary=summary"],"rows_from":"{}","key":"Ticket"}}
]"#,
            rows.display()
        ),
    )
    .unwrap();

    bin()
        .args([
            "patch",
            report.to_str().unwrap(),
            "--edits",
            patch.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(out.contains("<!-- mdli:id v=1 id=report.analytics -->"));
    assert!(out.contains("<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->"));
    assert!(out.contains("Patched"));
}

#[test]
fn patch_dry_run_leaves_document_untouched() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let patch = dir.path().join("patch.json");
    fs::write(&report, "# Report\n").unwrap();
    fs::write(
        &patch,
        r#"[{"op":"ensure_section","id":"report.summary","path":"Report > Summary","level":2}]"#,
    )
    .unwrap();

    bin()
        .args([
            "patch",
            report.to_str().unwrap(),
            "--edits",
            patch.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(fs::read_to_string(&report).unwrap(), "# Report\n");
}
