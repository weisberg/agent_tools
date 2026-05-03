use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn init_dry_run_does_not_create_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    Command::cargo_bin("lira")
        .expect("bin")
        .env("LIRA_HOME", &root)
        .args(["init", "--dry-run", "--json"])
        .assert()
        .success();

    assert!(!root.exists());
}

#[test]
fn create_and_show_project() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".lira-test");

    Command::cargo_bin("lira")
        .expect("bin")
        .env("LIRA_HOME", &root)
        .args(["init", "--json"])
        .assert()
        .success();

    Command::cargo_bin("lira")
        .expect("bin")
        .env("LIRA_HOME", &root)
        .args(["project", "create", "ORION", "Orion Project", "--json"])
        .assert()
        .success();

    Command::cargo_bin("lira")
        .expect("bin")
        .env("LIRA_HOME", &root)
        .args(["project", "show", "ORION", "--json"])
        .assert()
        .success()
        .stdout(contains("Orion Project"));
}
