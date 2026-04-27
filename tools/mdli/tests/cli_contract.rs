use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn bin() -> Command {
    Command::cargo_bin("mdli").expect("mdli binary")
}

#[test]
fn section_ensure_creates_id_marked_section_and_is_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    fs::write(&path, "# Report\n\nIntro.\n").unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "cashplus.analytics",
            "--path",
            "Report > 4. Campaign & Product Analytics",
            "--level",
            "2",
            "--write",
        ])
        .assert()
        .success();

    let once = fs::read_to_string(&path).unwrap();
    assert!(once.contains("<!-- mdli:id v=1 id=cashplus.analytics -->"));
    assert!(once.contains("## 4. Campaign & Product Analytics"));

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "cashplus.analytics",
            "--path",
            "Report > 4. Campaign & Product Analytics",
            "--level",
            "2",
            "--write",
        ])
        .assert()
        .success();

    assert_eq!(once, fs::read_to_string(&path).unwrap());
}

#[test]
fn table_replace_renders_named_table_from_ndjson() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(
        &report,
        "# Report\n\n<!-- mdli:id v=1 id=cashplus.analytics -->\n## Analytics\n",
    )
    .unwrap();
    fs::write(
        &rows,
        "{\"key\":\"CP-2\",\"summary\":\"Second\",\"status\":\"Done\"}\n{\"key\":\"CP-1\",\"summary\":\"First | escaped\",\"status\":\"Open\"}\n",
    )
    .unwrap();

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "cashplus.analytics",
            "--name",
            "analytics-tickets",
            "--columns",
            "Ticket=key,Summary=summary,Status=status",
            "--from-rows",
            rows.to_str().unwrap(),
            "--key",
            "Ticket",
            "--sort",
            "Ticket:asc",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(out.contains("<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->"));
    assert!(out.contains("| CP-1   | First \\| escaped | Open   |"));
    assert!(out.contains("| CP-2   | Second           | Done   |"));
}

#[test]
fn block_replace_refuses_modified_content_by_default() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let body = dir.path().join("body.md");
    fs::write(
        &report,
        "# Report\n\n<!-- mdli:id v=1 id=report.analytics -->\n## Analytics\n",
    )
    .unwrap();
    fs::write(&body, "Generated.\n").unwrap();

    bin()
        .args([
            "block",
            "ensure",
            report.to_str().unwrap(),
            "--parent-section",
            "report.analytics",
            "--id",
            "report.analytics.generated",
            "--body-from-file",
            body.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    let mut text = fs::read_to_string(&report).unwrap();
    text = text.replace("Generated.", "Human edit.");
    fs::write(&report, text).unwrap();
    fs::write(&body, "New generated.\n").unwrap();

    bin()
        .args([
            "block",
            "replace",
            report.to_str().unwrap(),
            "--id",
            "report.analytics.generated",
            "--body-from-file",
            body.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_BLOCK_MODIFIED"));
}

#[test]
fn lint_reports_duplicate_stable_ids() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    fs::write(
        &path,
        "<!-- mdli:id v=1 id=dup.id -->\n# One\n\n<!-- mdli:id v=1 id=dup.id -->\n# Two\n",
    )
    .unwrap();

    bin()
        .args(["--json", "lint", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("unique-stable-ids"))
        .stdout(predicate::str::contains("E_DUPLICATE_ID"));
}
