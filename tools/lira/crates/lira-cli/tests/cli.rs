use assert_cmd::Command;
use predicates::str::contains;

fn lira(root: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("lira").expect("bin");
    cmd.env("LIRA_HOME", root);
    cmd
}

#[test]
fn init_dry_run_does_not_create_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    lira(&root)
        .args(["init", "--dry-run", "--json"])
        .assert()
        .success()
        .stdout(contains("\"schema_version\": 3"))
        .stdout(contains("\"result\""));

    assert!(!root.exists());
}

#[test]
fn create_and_show_project() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    lira(&root).args(["init", "--json"]).assert().success();

    lira(&root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();

    lira(&root)
        .args(["project", "show", "ORION", "--json"])
        .assert()
        .success()
        .stdout(contains("Orion Project"))
        .stdout(contains("workflow"));
}

#[test]
fn ticket_lifecycle_enforces_completion_policy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    lira(&root).args(["init", "--json"]).assert().success();
    lira(&root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();
    lira(&root)
        .args([
            "new",
            "Implement local tickets",
            "--project",
            "ORION",
            "--acceptance-criterion",
            "Tickets can be created.",
            "--task",
            "Create the ticket command.",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ORION-1"));

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
fn comments_claims_and_doctor_work() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    lira(&root).args(["init", "--json"]).assert().success();
    lira(&root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();
    lira(&root)
        .args([
            "new",
            "Add comments",
            "--project",
            "ORION",
            "--acceptance-criterion",
            "Comments are visible.",
            "--task",
            "Add one comment.",
            "--json",
        ])
        .assert()
        .success();
    lira(&root)
        .args([
            "comment", "ORION-1", "Hello", "--author", "athena", "--json",
        ])
        .assert()
        .success()
        .stdout(contains("local-c1"));
    lira(&root)
        .args(["claim", "ORION-1", "--agent", "athena", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["active", "--agent", "athena", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\": true"));
}

#[test]
fn search_query_count_and_board_work() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    lira(&root).args(["init", "--json"]).assert().success();
    lira(&root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();
    lira(&root)
        .args([
            "new",
            "Add search",
            "--project",
            "ORION",
            "--acceptance-criterion",
            "Search finds ticket text.",
            "--task",
            "Index the ticket title.",
            "--parent-jira",
            "VAN-1234",
            "--json",
        ])
        .assert()
        .success();
    lira(&root)
        .args(["label", "add", "ORION-1", "search", "--json"])
        .assert()
        .success();
    lira(&root)
        .args(["search", "ticket title", "--json"])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args([
            "query",
            "--label",
            "search",
            "--parent-jira",
            "VAN-1234",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ORION-1"));
    lira(&root)
        .args(["count", "--group-by", "status", "--json"])
        .assert()
        .success()
        .stdout(contains("backlog"));
    lira(&root)
        .args(["board", "--json"])
        .assert()
        .success()
        .stdout(contains("board"));
}
