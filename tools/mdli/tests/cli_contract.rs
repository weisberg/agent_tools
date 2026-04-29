mod common;

use common::bin;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// section ensure / replace / delete / move / rename
// ---------------------------------------------------------------------------

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
fn section_ensure_after_anchors_a_new_section_in_order() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    fs::write(
        &path,
        "# Report\n\n<!-- mdli:id v=1 id=report.first -->\n## First\n\nA\n",
    )
    .unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "report.second",
            "--path",
            "Report > Second",
            "--level",
            "2",
            "--after",
            "report.first",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&path).unwrap();
    let first = out.find("## First").unwrap();
    let second = out.find("## Second").unwrap();
    assert!(first < second, "second should come after first");
}

#[test]
fn section_replace_body_replaces_only_body_lines() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    let body = dir.path().join("body.md");
    fs::write(
        &path,
        "<!-- mdli:id v=1 id=report.intro -->\n## Intro\n\nOld body.\n",
    )
    .unwrap();
    fs::write(&body, "Brand new body.\n").unwrap();

    bin()
        .args([
            "section",
            "replace",
            path.to_str().unwrap(),
            "--id",
            "report.intro",
            "--body-from-file",
            body.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&path).unwrap();
    assert!(out.contains("## Intro"));
    assert!(out.contains("Brand new body."));
    assert!(!out.contains("Old body."));
}

#[test]
fn section_delete_removes_section_and_marker() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    fs::write(
        &path,
        "# Report\n\n<!-- mdli:id v=1 id=report.intro -->\n## Intro\n\nA paragraph.\n\n## Other\n",
    )
    .unwrap();

    bin()
        .args([
            "section",
            "delete",
            path.to_str().unwrap(),
            "--id",
            "report.intro",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&path).unwrap();
    assert!(!out.contains("Intro"));
    assert!(!out.contains("report.intro"));
    assert!(out.contains("## Other"));
}

#[test]
fn section_move_repositions_section() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    fs::write(
        &path,
        "# Report\n\n<!-- mdli:id v=1 id=a -->\n## A\n\n<!-- mdli:id v=1 id=b -->\n## B\n",
    )
    .unwrap();

    bin()
        .args([
            "section",
            "move",
            path.to_str().unwrap(),
            "--id",
            "a",
            "--after",
            "b",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&path).unwrap();
    let pos_a = out.find("## A").unwrap();
    let pos_b = out.find("## B").unwrap();
    assert!(pos_b < pos_a, "section A should move below section B");
}

#[test]
fn section_rename_updates_visible_heading_only() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("report.md");
    fs::write(&path, "<!-- mdli:id v=1 id=report.intro -->\n## Intro\n").unwrap();

    bin()
        .args([
            "section",
            "rename",
            path.to_str().unwrap(),
            "--id",
            "report.intro",
            "--to",
            "Introduction",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&path).unwrap();
    assert!(out.contains("## Introduction"));
    assert!(out.contains("id=report.intro"));
}

// ---------------------------------------------------------------------------
// table operations
// ---------------------------------------------------------------------------

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
fn table_upsert_replaces_existing_row_by_key() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(
        &report,
        "# Report\n\n<!-- mdli:id v=1 id=t -->\n## Tickets\n",
    )
    .unwrap();
    fs::write(&rows, "{\"key\":\"CP-1\",\"summary\":\"First\"}\n").unwrap();

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "t",
            "--name",
            "tt",
            "--columns",
            "Ticket=key,Summary=summary",
            "--from-rows",
            rows.to_str().unwrap(),
            "--key",
            "Ticket",
            "--write",
        ])
        .assert()
        .success();

    bin()
        .args([
            "table",
            "upsert",
            report.to_str().unwrap(),
            "--name",
            "tt",
            "--key",
            "Ticket",
            "--row",
            "Ticket=CP-1",
            "--row",
            "Summary=First updated",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(out.contains("First updated"));
    assert!(!out.contains("| First |"));
}

#[test]
fn table_delete_row_removes_matching_row() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(&report, "<!-- mdli:id v=1 id=t -->\n## Tickets\n").unwrap();
    fs::write(
        &rows,
        "{\"key\":\"CP-1\",\"summary\":\"First\"}\n{\"key\":\"CP-2\",\"summary\":\"Second\"}\n",
    )
    .unwrap();

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "t",
            "--name",
            "tt",
            "--columns",
            "Ticket=key,Summary=summary",
            "--from-rows",
            rows.to_str().unwrap(),
            "--key",
            "Ticket",
            "--write",
        ])
        .assert()
        .success();

    bin()
        .args([
            "table",
            "delete-row",
            report.to_str().unwrap(),
            "--name",
            "tt",
            "--key",
            "Ticket",
            "--value",
            "CP-1",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(!out.contains("| CP-1"));
    assert!(out.contains("| CP-2"));
}

#[test]
fn table_sort_orders_rows_descending() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(&report, "<!-- mdli:id v=1 id=t -->\n## Tickets\n").unwrap();
    fs::write(
        &rows,
        "{\"key\":\"CP-1\",\"summary\":\"a\"}\n{\"key\":\"CP-3\",\"summary\":\"c\"}\n{\"key\":\"CP-2\",\"summary\":\"b\"}\n",
    )
    .unwrap();

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "t",
            "--name",
            "tt",
            "--columns",
            "Ticket=key,Summary=summary",
            "--from-rows",
            rows.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    bin()
        .args([
            "table",
            "sort",
            report.to_str().unwrap(),
            "--name",
            "tt",
            "--by",
            "Ticket:desc",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    let pos1 = out.find("| CP-1").unwrap();
    let pos3 = out.find("| CP-3").unwrap();
    assert!(pos3 < pos1, "CP-3 should come before CP-1 after desc sort");
}

#[test]
fn table_fmt_canonicalizes_existing_table() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    fs::write(
        &report,
        "<!-- mdli:id v=1 id=t -->\n## Tickets\n\n<!-- mdli:table v=1 name=tt -->\n|A|B|\n|---|---|\n|1|2|\n",
    )
    .unwrap();

    bin()
        .args([
            "table",
            "fmt",
            report.to_str().unwrap(),
            "--name",
            "tt",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    // Canonical separator uses `---` (3 dashes minimum), padded.
    assert!(out.contains("| --- | --- |"));
    assert!(out.contains("| A | B |"));
    assert!(out.contains("| 1 | 2 |"));
}

#[test]
fn table_replace_rejects_duplicate_keys_by_default() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(&report, "<!-- mdli:id v=1 id=t -->\n## T\n").unwrap();
    fs::write(
        &rows,
        "{\"key\":\"X\",\"v\":\"1\"}\n{\"key\":\"X\",\"v\":\"2\"}\n",
    )
    .unwrap();

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "t",
            "--name",
            "dup",
            "--columns",
            "Key=key,V=v",
            "--from-rows",
            rows.to_str().unwrap(),
            "--key",
            "Key",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_TABLE_DUPLICATE_KEY"));
}

#[test]
fn table_replace_plan_reports_row_delta() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(
        &report,
        "# Report\n\n<!-- mdli:id v=1 id=report.analytics -->\n## Analytics\n\n<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->\n| Ticket | Summary | Status |\n|---|---|---|\n| CP-1 | Old | Open |\n| CP-2 | Remove me | Done |\n",
    )
    .unwrap();
    fs::write(
        &rows,
        "{\"key\":\"CP-1\",\"summary\":\"New\",\"status\":\"Open\"}\n{\"key\":\"CP-3\",\"summary\":\"Added\",\"status\":\"Open\"}\n",
    )
    .unwrap();

    let output = bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "report.analytics",
            "--name",
            "analytics-tickets",
            "--columns",
            "Ticket=key,Summary=summary,Status=status",
            "--from-rows",
            rows.to_str().unwrap(),
            "--key",
            "Ticket",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let op = &json["result"]["ops"][0];
    assert_eq!(op["rows_before"], 2);
    assert_eq!(op["rows_after"], 2);
    assert_eq!(op["rows_added"], 1);
    assert_eq!(op["rows_removed"], 1);
    assert_eq!(op["rows_updated"], 1);
    assert_eq!(
        fs::read_to_string(&report).unwrap().matches("CP-3").count(),
        0
    );
}

#[test]
fn table_replace_rejects_rich_cells_and_allows_missing_empty() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let rows = dir.path().join("rows.ndjson");
    fs::write(
        &report,
        "# Report\n\n<!-- mdli:id v=1 id=report.analytics -->\n## Analytics\n",
    )
    .unwrap();
    fs::write(&rows, "{\"key\":\"CP-1\",\"details\":{\"nested\":true}}\n").unwrap();

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "report.analytics",
            "--columns",
            "Ticket=key,Details=details,Optional=missing",
            "--from-rows",
            rows.to_str().unwrap(),
            "--missing",
            "empty",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_RICH_CELL"));

    bin()
        .args([
            "table",
            "replace",
            report.to_str().unwrap(),
            "--section",
            "report.analytics",
            "--columns",
            "Ticket=key,Details=details,Optional=missing",
            "--from-rows",
            rows.to_str().unwrap(),
            "--missing",
            "empty",
            "--on-rich-cell",
            "json",
            "--emit",
            "document",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("{\"nested\":true}"));
}

// ---------------------------------------------------------------------------
// managed blocks
// ---------------------------------------------------------------------------

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
fn block_replace_force_overwrites_modified_content() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let body = dir.path().join("body.md");
    fs::write(&report, "<!-- mdli:id v=1 id=t -->\n## T\n").unwrap();
    fs::write(&body, "Initial.\n").unwrap();

    bin()
        .args([
            "block",
            "ensure",
            report.to_str().unwrap(),
            "--parent-section",
            "t",
            "--id",
            "t.gen",
            "--body-from-file",
            body.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    let mut text = fs::read_to_string(&report).unwrap();
    text = text.replace("Initial.", "Tampered.");
    fs::write(&report, text).unwrap();
    fs::write(&body, "Forced.\n").unwrap();

    bin()
        .args([
            "block",
            "replace",
            report.to_str().unwrap(),
            "--id",
            "t.gen",
            "--body-from-file",
            body.to_str().unwrap(),
            "--on-modified",
            "force",
            "--write",
        ])
        .assert()
        .success();

    let out = fs::read_to_string(&report).unwrap();
    assert!(out.contains("Forced."));
    assert!(!out.contains("Tampered."));
}

#[test]
fn block_replace_three_way_writes_conflict_artifact() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let body = dir.path().join("body.md");
    fs::write(&report, "<!-- mdli:id v=1 id=t -->\n## T\n").unwrap();
    fs::write(&body, "Initial.\n").unwrap();

    bin()
        .args([
            "block",
            "ensure",
            report.to_str().unwrap(),
            "--parent-section",
            "t",
            "--id",
            "t.gen",
            "--body-from-file",
            body.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    let mut text = fs::read_to_string(&report).unwrap();
    text = text.replace("Initial.", "Human edit.");
    fs::write(&report, &text).unwrap();
    fs::write(&body, "Incoming generated.\n").unwrap();

    bin()
        .args([
            "block",
            "replace",
            report.to_str().unwrap(),
            "--id",
            "t.gen",
            "--body-from-file",
            body.to_str().unwrap(),
            "--on-modified",
            "three-way",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_BLOCK_MODIFIED"))
        .stderr(predicate::str::contains(".mdli.conflict"));

    let artifact_path = format!("{}.mdli.conflict", report.display());
    let artifact = fs::read_to_string(&artifact_path).expect("conflict artifact written");
    assert!(artifact.contains("\"kind\": \"three-way-conflict\""));
    assert!(artifact.contains("\"block_id\": \"t.gen\""));
    assert!(artifact.contains("\"on_disk_body\": \"Human edit.\""));
    assert!(artifact.contains("\"incoming_body\": \"Incoming generated.\""));
    assert!(artifact.contains("\"recorded_checksum\""));
    assert!(artifact.contains("\"actual_checksum\""));

    // Source file is left untouched.
    let after = fs::read_to_string(&report).unwrap();
    assert!(after.contains("Human edit."));
    assert!(!after.contains("Incoming generated."));
}

#[test]
fn block_lock_then_replace_is_blocked_by_default() {
    let dir = tempdir().unwrap();
    let report = dir.path().join("report.md");
    let body = dir.path().join("body.md");
    fs::write(&report, "<!-- mdli:id v=1 id=s -->\n## S\n").unwrap();
    fs::write(&body, "Locked content.\n").unwrap();

    bin()
        .args([
            "block",
            "ensure",
            report.to_str().unwrap(),
            "--parent-section",
            "s",
            "--id",
            "s.gen",
            "--body-from-file",
            body.to_str().unwrap(),
            "--write",
        ])
        .assert()
        .success();

    bin()
        .args([
            "block",
            "lock",
            report.to_str().unwrap(),
            "--id",
            "s.gen",
            "--write",
        ])
        .assert()
        .success();

    fs::write(&body, "Should not pass.\n").unwrap();
    bin()
        .args([
            "block",
            "replace",
            report.to_str().unwrap(),
            "--id",
            "s.gen",
            "--body-from-file",
            body.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_BLOCK_LOCKED"));

    bin()
        .args([
            "block",
            "unlock",
            report.to_str().unwrap(),
            "--id",
            "s.gen",
            "--write",
        ])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// frontmatter + ids
// ---------------------------------------------------------------------------

#[test]
fn frontmatter_set_and_delete_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Title\n").unwrap();

    bin()
        .args([
            "frontmatter",
            "set",
            path.to_str().unwrap(),
            "title",
            "Hello",
            "--write",
        ])
        .assert()
        .success();
    let out = fs::read_to_string(&path).unwrap();
    assert!(out.starts_with("---"));
    assert!(out.contains("title: Hello"));

    bin()
        .args([
            "frontmatter",
            "delete",
            path.to_str().unwrap(),
            "title",
            "--write",
        ])
        .assert()
        .success();
    let out = fs::read_to_string(&path).unwrap();
    assert!(!out.contains("title: Hello"));
}

#[test]
fn id_assign_all_creates_marker_for_each_unmarked_section() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# One\n## Two\n## Three\n").unwrap();

    bin()
        .args(["id", "assign", path.to_str().unwrap(), "--all", "--write"])
        .assert()
        .success();

    let out = fs::read_to_string(&path).unwrap();
    assert_eq!(out.matches("mdli:id").count(), 3);
}

// ---------------------------------------------------------------------------
// lint + inspect
// ---------------------------------------------------------------------------

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

#[test]
fn inspect_emits_sections_tables_blocks_and_issues() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(
        &path,
        "<!-- mdli:id v=1 id=a -->\n# A\n\n<!-- mdli:table v=1 name=t -->\n| X | Y |\n|---|---|\n| 1 | 2 |\n",
    )
    .unwrap();

    bin()
        .args(["inspect", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sections\""))
        .stdout(predicate::str::contains("\"tables\""))
        .stdout(predicate::str::contains("\"blocks\""))
        .stdout(predicate::str::contains("\"issues\""));
}

// ---------------------------------------------------------------------------
// negative paths: ambiguity + missing selectors
// ---------------------------------------------------------------------------

#[test]
fn ambiguous_path_selector_errors() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n\n## Same\n\nA\n\n## Other\n\n## Same\n\nB\n").unwrap();

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
fn ambiguous_path_selector_lists_match_details() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n\n## Same\n\nA\n\n## Other\n\n## Same\n\nB\n").unwrap();

    bin()
        .args([
            "--json",
            "section",
            "get",
            path.to_str().unwrap(),
            "--path",
            "Top > Same",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("E_AMBIGUOUS_SELECTOR"))
        .stdout(predicate::str::contains("\"matches\""))
        .stdout(predicate::str::contains("\"path\": \"Top > Same\""))
        .stdout(predicate::str::contains("\"line\""));
}

#[test]
fn missing_selector_errors() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n").unwrap();

    bin()
        .args([
            "section",
            "get",
            path.to_str().unwrap(),
            "--id",
            "no.such.id",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_SELECTOR_NOT_FOUND"));
}

#[test]
fn invalid_id_grammar_is_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n").unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "Bad.Id",
            "--path",
            "Top > Sub",
            "--level",
            "2",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_INVALID_ID"));
}

#[test]
fn invalid_utf8_input_is_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.md");
    fs::write(&path, b"# Bad\n\xff\xff\n").unwrap();

    bin()
        .args(["inspect", path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_INVALID_UTF8"));
}

#[test]
fn write_and_emit_document_are_mutually_exclusive() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n").unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "top.id",
            "--path",
            "Top",
            "--level",
            "1",
            "--write",
            "--emit",
            "document",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_INVALID_OUTPUT_MODE"));
}

#[test]
fn stale_preimage_hash_is_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# Top\n").unwrap();

    bin()
        .args([
            "section",
            "ensure",
            path.to_str().unwrap(),
            "--id",
            "top",
            "--path",
            "Top",
            "--level",
            "1",
            "--preimage-hash",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "--write",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("E_STALE_PREIMAGE"));
}
