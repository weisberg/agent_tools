use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::Path;
use std::time::Duration;

fn lira(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("lira").expect("bin");
    cmd.env("LIRA_HOME", root);
    cmd
}

fn init_project(root: &Path) {
    lira(root).args(["init", "--json"]).assert().success();
    lira(root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();
}

fn create_ticket(root: &Path, title: &str, acceptance: &str, task: &str, extra: &[&str]) {
    let mut args = vec![
        "new",
        title,
        "--project",
        "ORION",
        "--acceptance-criterion",
        acceptance,
        "--task",
        task,
        "--priority",
        "high",
    ];
    args.extend_from_slice(extra);
    args.push("--json");
    lira(root).args(args).assert().success();
}

#[test]
fn sqlite_index_updates_after_mutations_without_reindex() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "Mutation indexed ticket",
        "Mutation updates are indexed.",
        "Start mutation work.",
        &[],
    );

    assert!(root.join("index/tickets.sqlite").exists());

    lira(&root)
        .args(["mv", "ORION-1", "todo", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["mv", "ORION-1", "in-progress", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["label", "add", "ORION-1", "sqlite", "--json"])
        .assert()
        .success();
    lira(&root)
        .args([
            "task",
            "add",
            "ORION-1",
            "Exercise sqlite mutation cache.",
            "--tag",
            "cache",
            "--json",
        ])
        .assert()
        .success();
    lira(&root)
        .args(["task", "done", "ORION-1", "T1", "--json"])
        .assert()
        .success();

    lira(&root)
        .args(["count", "--group-by", "status", "--json"])
        .assert()
        .success()
        .stdout(contains("\"in-progress\": 1"));
    lira(&root)
        .args([
            "query",
            "--status",
            "in-progress",
            "--label",
            "sqlite",
            "--task-tag",
            "cache",
            "--task-status",
            "done",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ORION-1"));

    lira(&root)
        .args(["label", "remove", "ORION-1", "sqlite", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["query", "--label", "sqlite", "--json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("ORION-1").not());
}

#[test]
fn reindex_rebuilds_from_yaml_and_drops_stale_rows() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "Alpha survivor",
        "Alpha remains after rebuild.",
        "Keep alpha.",
        &[],
    );
    create_ticket(
        &root,
        "Beta stale",
        "Beta is removed from YAML.",
        "Remove beta.",
        &[],
    );

    lira(&root)
        .args(["reindex", "--json"])
        .assert()
        .success()
        .stdout(contains("\"tickets_indexed\": 2"));

    std::fs::remove_file(root.join("projects/ORION/tickets/backlog/ORION-2.yaml"))
        .expect("remove stale yaml");

    lira(&root)
        .args(["reindex", "--json"])
        .assert()
        .success()
        .stdout(contains("\"tickets_indexed\": 1"));
    lira(&root)
        .args(["search", "Beta", "--json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("ORION-2").not());
    lira(&root)
        .args(["search", "Alpha", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
}

#[test]
fn fts_search_covers_title_acceptance_tasks_labels_and_task_tags() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "Comet title",
        "Nebula acceptance is visible.",
        "Write quasar task.",
        &[],
    );
    lira(&root)
        .args(["label", "add", "ORION-1", "galaxy", "--json"])
        .assert()
        .success();
    lira(&root)
        .args([
            "task",
            "add",
            "ORION-1",
            "Add extra indexed tag.",
            "--tag",
            "meteor",
            "--json",
        ])
        .assert()
        .success();

    for term in ["Comet", "Nebula", "quasar", "galaxy", "meteor"] {
        lira(&root)
            .args(["search", term, "--json"])
            .assert()
            .success()
            .stdout(contains("ORION-1"));
    }
}

#[test]
fn indexed_query_combines_parent_assignee_label_task_status_and_tag_filters() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "Combined filter target",
        "Combined filters return this ticket.",
        "Complete indexed task.",
        &["--parent-jira", "VAN-1234", "--assignee", "athena"],
    );
    create_ticket(
        &root,
        "Combined filter decoy",
        "Decoy should not match.",
        "Ignore decoy.",
        &["--parent-jira", "VAN-9999", "--assignee", "other"],
    );

    lira(&root)
        .args(["label", "add", "ORION-1", "analytics", "--json"])
        .assert()
        .success();
    lira(&root)
        .args([
            "task",
            "add",
            "ORION-1",
            "Tagged indexed work.",
            "--tag",
            "sql",
            "--json",
        ])
        .assert()
        .success();
    lira(&root)
        .args(["task", "done", "ORION-1", "T1", "--json"])
        .assert()
        .success();

    lira(&root)
        .args([
            "query",
            "--parent-jira",
            "VAN-1234",
            "--assignee",
            "athena",
            "--label",
            "analytics",
            "--task-status",
            "done",
            "--task-tag",
            "sql",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ORION-1"))
        .stdout(predicates::str::contains("ORION-2").not());
}

#[test]
fn fts_search_escapes_special_query_syntax() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "SQL-backed: cache [v2]",
        "Special punctuation searches should not leak FTS syntax errors.",
        "Keep FTS queries safe.",
        &[],
    );

    lira(&root)
        .args(["search", "SQL-backed:", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["search", "[v2]", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"schema_version\": 2"))
        .stdout(contains("\"tickets_indexed\": 1"))
        .stdout(contains("\"stale\": false"));
}

#[test]
fn stale_marker_tracks_yaml_drift_and_no_index_bypasses_cache() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "Original indexed title",
        "The canonical YAML can be edited outside the cache.",
        "Detect source mtime drift.",
        &[],
    );

    lira(&root)
        .args(["reindex", "--json"])
        .assert()
        .success()
        .stdout(contains("\"tickets_indexed\": 1"));

    let ticket_path = root.join("projects/ORION/tickets/backlog/ORION-1.yaml");
    let body = std::fs::read_to_string(&ticket_path).expect("ticket yaml");
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(
        &ticket_path,
        body.replace("Original indexed title", "Edited canonical title"),
    )
    .expect("edit canonical yaml");

    lira(&root)
        .args(["search", "Edited", "--json"])
        .assert()
        .failure()
        .stdout(contains("E_INDEX_STALE"));
    assert!(root.join("index/stale.json").exists());

    lira(&root)
        .args(["search", "Edited", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["query", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["count", "--group-by", "status", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("\"backlog\": 1"));
    lira(&root)
        .args(["board", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));

    lira(&root)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"stale\": true"))
        .stdout(contains("source mtime changed"));

    lira(&root)
        .args(["reindex", "--json"])
        .assert()
        .success()
        .stdout(contains("\"tickets_indexed\": 1"));
    assert!(!root.join("index/stale.json").exists());
    lira(&root)
        .args(["search", "Edited", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"stale\": false"));
}

#[test]
fn no_index_diagnostics_do_not_create_sqlite_cache() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(
        &root,
        "Filesystem diagnostic search",
        "No-index mode reads YAML directly.",
        "Avoid creating sqlite.",
        &[],
    );

    std::fs::remove_file(root.join("index/tickets.sqlite")).expect("remove index");
    lira(&root)
        .args(["search", "Filesystem", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["query", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["count", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("\"backlog\": 1"));
    lira(&root)
        .args(["board", "--no-index", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    assert!(!root.join("index/tickets.sqlite").exists());
}
