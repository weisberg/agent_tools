use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn lira(root: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("lira").expect("bin");
    cmd.env("LIRA_HOME", root);
    cmd
}

fn init_project(root: &std::path::Path) {
    lira(root).args(["init", "--json"]).assert().success();
    lira(root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();
}

fn create_ticket(root: &std::path::Path, title: &str) {
    lira(root)
        .args([
            "new",
            title,
            "--project",
            "ORION",
            "--acceptance-criterion",
            "The work is verifiable.",
            "--task",
            "Do the work.",
            "--priority",
            "high",
            "--json",
        ])
        .assert()
        .success();
}

#[test]
fn init_dry_run_does_not_create_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    lira(&root)
        .args(["init", "--dry-run", "--json"])
        .assert()
        .success()
        .stdout(contains("\"schema_version\": 3"));

    assert!(!root.exists());
}

#[test]
fn ticket_lifecycle_enforces_completion_policy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(&root, "Implement local tickets");

    lira(&root)
        .args(["mv", "ORION-1", "todo", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["mv", "ORION-1", "in-progress", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["mv", "ORION-1", "done", "--json"])
        .assert()
        .failure()
        .stdout(contains("E_COMPLETION_POLICY"));

    lira(&root)
        .args(["task", "done", "ORION-1", "T1", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["mv", "ORION-1", "done", "--json"])
        .assert()
        .success();
}

#[test]
fn candidates_return_normalized_unclaimed_unblocked_issues() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(&root, "Blocked ticket");
    create_ticket(&root, "Open ticket");

    lira(&root)
        .args(["mv", "ORION-1", "todo", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["mv", "ORION-2", "todo", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["link", "ORION-1", "--blocked-by", "ORION-2", "--json"])
        .assert()
        .success();

    lira(&root)
        .args(["candidates", "--project", "ORION", "--json"])
        .assert()
        .success()
        .stdout(contains("\"identifier\": \"ORION-2\""))
        .stdout(predicates::str::contains("\"identifier\": \"ORION-1\"").not());

    lira(&root)
        .args(["claim", "ORION-2", "--agent", "runner", "--json"])
        .assert()
        .success()
        .stdout(contains("\"claimed\": true"))
        .stdout(contains("\"issue\""));

    lira(&root)
        .args(["candidates", "--project", "ORION", "--json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"identifier\": \"ORION-2\"").not());
}

#[test]
fn issue_current_reports_found_and_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    create_ticket(&root, "Show current issue");

    lira(&root)
        .args(["issue", "show", "ORION-1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"state\": \"backlog\""));

    lira(&root)
        .args([
            "issue", "current", "--ids", "ORION-1", "--ids", "ORION-9", "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\": true"))
        .stdout(contains("\"E_TICKET_NOT_FOUND\""));
}

#[test]
fn workflow_symphony_export_and_validate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");
    init_project(&root);
    let workflow = temp.path().join("WORKFLOW.md");
    std::fs::write(
        &workflow,
        "---\ntracker:\n  kind: lira\n  project: ORION\nworkspace:\n  root: /tmp/ws\n---\n# Workflow\n",
    )
    .expect("workflow");

    lira(&root)
        .args([
            "workflow",
            "symphony",
            "export",
            "--project",
            "ORION",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"kind\": \"lira\""));

    lira(&root)
        .args([
            "workflow",
            "symphony",
            "validate",
            workflow.to_str().expect("utf8"),
            "--project",
            "ORION",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"valid\": true"));
}
