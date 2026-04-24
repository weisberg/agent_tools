use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn jirali(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("jirali").unwrap();
    cmd.env("JIRALI_HOME", home.path());
    cmd
}

fn stdout_json(assert: assert_cmd::assert::Assert) -> Value {
    let output = assert.success().get_output().stdout.clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn issue_lifecycle_is_json_and_idempotent() {
    let home = TempDir::new().unwrap();

    let created = stdout_json(
        jirali(&home)
            .args([
                "issue",
                "create",
                "--project",
                "ENG",
                "--type",
                "Task",
                "--summary",
                "Build Jirali",
                "--assignee",
                "agent@example.com",
            ])
            .assert(),
    );
    assert_eq!(created["key"], "ENG-1");

    let viewed = stdout_json(jirali(&home).args(["issue", "view", "ENG-1"]).assert());
    assert_eq!(viewed["key"], "ENG-1");
    assert_eq!(viewed["summary"], "Build Jirali");

    let edited = stdout_json(
        jirali(&home)
            .args([
                "issue",
                "edit",
                "ENG-1",
                "--add-label",
                "jirali",
                "--field",
                "story_points=5",
            ])
            .assert(),
    );
    assert_eq!(edited["key"], "ENG-1");

    jirali(&home)
        .args(["issue", "ensure", "ENG-1", "--field", "story_points=5"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("\"code\":\"CONFLICT\""));
}

#[test]
fn validation_errors_are_structured_stderr() {
    let home = TempDir::new().unwrap();
    jirali(&home)
        .args([
            "issue",
            "create",
            "--project",
            "ENG",
            "--type",
            "Task",
            "--summary",
            "Needs review",
        ])
        .assert()
        .success();

    jirali(&home)
        .args(["issue", "transition", "ENG-1", "Code Review"])
        .assert()
        .code(7)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"code\":\"VALIDATION_FAILED\""))
        .stderr(predicate::str::contains("root_cause"));
}

#[test]
fn adf_round_trip_and_lint_are_machine_readable() {
    let home = TempDir::new().unwrap();
    let adf = stdout_json(
        jirali(&home)
            .args(["adf", "from-markdown", "# Hello"])
            .assert(),
    );
    assert_eq!(adf["type"], "doc");

    let lint = stdout_json(
        jirali(&home)
            .args([
                "jql",
                "lint",
                "project = ENG AND status != Done ORDER BY created",
            ])
            .assert(),
    );
    assert_eq!(lint["valid"], true);
    assert!(lint["warnings"].as_array().unwrap().len() >= 2);
}

#[test]
fn auth_redacts_tokens_and_audit_records_exist() {
    let home = TempDir::new().unwrap();
    jirali(&home)
        .args([
            "auth",
            "login",
            "--method",
            "api-token",
            "--site-url",
            "https://example.atlassian.net",
            "--email",
            "agent@example.com",
            "--token",
            "secret-token",
        ])
        .assert()
        .success();

    let config = stdout_json(jirali(&home).args(["config", "show"]).assert());
    assert_eq!(config["profiles"]["default"]["api_token"], "***REDACTED***");

    let audit = stdout_json(jirali(&home).args(["audit", "list"]).assert());
    assert!(audit["data"].as_array().unwrap().len() >= 1);
    assert!(!audit.to_string().contains("secret-token"));
}

#[test]
fn roadmap_surfaces_have_contract_outputs() {
    let home = TempDir::new().unwrap();
    for args in [
        vec!["tools"],
        vec!["skill", "emit"],
        vec!["mcp", "serve"],
        vec!["report", "velocity"],
        vec!["assets", "schemas"],
        vec!["automation", "list"],
        vec![
            "webhook",
            "listen",
            "--event",
            "jira:issue_updated",
            "--timeout",
            "1",
        ],
        vec!["local", "search", "nothing", "--semantic"],
    ] {
        let value = stdout_json(jirali(&home).args(args).assert());
        assert!(value.is_object());
    }
}
