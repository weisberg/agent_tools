use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
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

fn mock_once(status: u16, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let response = format!(
            "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    format!("http://{addr}")
}

#[test]
fn configured_profile_uses_live_jira_issue_view_and_caches_result() {
    let home = TempDir::new().unwrap();
    let base = mock_once(
        200,
        r#"{"key":"ENG-77","fields":{"summary":"From mock Jira","status":{"name":"Done"}}}"#,
    );

    jirali(&home)
        .args([
            "auth",
            "login",
            "--method",
            "api-token",
            "--site-url",
            &base,
            "--email",
            "agent@example.com",
            "--token",
            "secret-token",
        ])
        .assert()
        .success();

    let viewed = stdout_json(jirali(&home).args(["issue", "view", "ENG-77"]).assert());
    assert_eq!(viewed["key"], "ENG-77");
    assert_eq!(viewed["fields"]["summary"], "From mock Jira");

    let cached = stdout_json(
        jirali(&home)
            .args(["local", "search", "mock Jira"])
            .assert(),
    );
    assert_eq!(cached["backend"], "sqlite-fts5");
    assert_eq!(cached["data"].as_array().unwrap().len(), 1);
}

#[test]
fn live_http_errors_map_to_structured_exit_codes() {
    for (status, code, expected) in [
        (401, 4, "PERMISSION_DENIED"),
        (404, 3, "NOT_FOUND"),
        (409, 5, "CONFLICT"),
        (429, 6, "RATE_LIMITED"),
        (400, 7, "VALIDATION_FAILED"),
        (500, 1, "GENERAL_FAILURE"),
    ] {
        let home = TempDir::new().unwrap();
        let base = mock_once(status, r#"{"errorMessages":["fixture"]}"#);
        jirali(&home)
            .args([
                "auth",
                "login",
                "--method",
                "api-token",
                "--site-url",
                &base,
                "--email",
                "agent@example.com",
                "--token",
                "secret-token",
            ])
            .assert()
            .success();

        jirali(&home)
            .args(["issue", "view", "ENG-404"])
            .assert()
            .code(code)
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(format!("\"code\":\"{expected}\"")));
    }
}

#[test]
fn issue_list_preserves_v3_permission_failure_without_legacy_fallback_masking() {
    let home = TempDir::new().unwrap();
    let base = mock_once(401, r#"{"errorMessages":["auth required"]}"#);
    jirali(&home)
        .args([
            "auth",
            "login",
            "--method",
            "api-token",
            "--site-url",
            &base,
            "--email",
            "agent@example.com",
            "--token",
            "secret-token",
        ])
        .assert()
        .success();

    jirali(&home)
        .args(["issue", "list", "--jql", "ORDER BY updated DESC"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"code\":\"PERMISSION_DENIED\""));
}

#[test]
fn issue_list_preserves_v3_validation_failure_without_legacy_fallback_masking() {
    let home = TempDir::new().unwrap();
    let base = mock_once(400, r#"{"errorMessages":["unbounded jql"]}"#);
    jirali(&home)
        .args([
            "auth",
            "login",
            "--method",
            "api-token",
            "--site-url",
            &base,
            "--email",
            "agent@example.com",
            "--token",
            "secret-token",
        ])
        .assert()
        .success();

    jirali(&home)
        .args(["issue", "list", "--jql", "ORDER BY updated DESC"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("\"code\":\"VALIDATION_FAILED\""));
}

#[test]
fn auth_login_normalizes_jira_web_ui_site_url() {
    let home = TempDir::new().unwrap();
    let login = stdout_json(
        jirali(&home)
            .args([
                "auth",
                "login",
                "--method",
                "api-token",
                "--site-url",
                "https://example.atlassian.net/jira/",
                "--email",
                "agent@example.com",
                "--token",
                "secret-token",
            ])
            .assert(),
    );
    assert_eq!(login["site_url"], "https://example.atlassian.net");

    let config = stdout_json(jirali(&home).args(["config", "show"]).assert());
    assert_eq!(
        config["profiles"]["default"]["site_url"],
        "https://example.atlassian.net"
    );
}

#[test]
fn parser_backed_jql_reports_errors_with_rule_ids() {
    let home = TempDir::new().unwrap();
    let lint = stdout_json(
        jirali(&home)
            .args(["jql", "lint", "madeup = 1 AND created = today"])
            .assert(),
    );
    let errors = lint["errors"].as_array().unwrap();
    assert!(errors.iter().any(|e| e["rule"] == "unknown_field"));
    assert!(errors.iter().any(|e| e["rule"] == "type_mismatch"));
    assert!(lint["tokens"].as_array().unwrap().len() > 3);
}

#[test]
fn daemon_status_reports_stateless_fallback() {
    let home = TempDir::new().unwrap();
    let status = stdout_json(jirali(&home).args(["daemon", "status"]).assert());
    assert_eq!(status["ok"], true);
    assert_eq!(status["fallback"], "stateless");
}

#[test]
fn adf_pipeline_supports_marks_links_code_and_tables() {
    let home = TempDir::new().unwrap();
    let adf = stdout_json(
        jirali(&home)
            .args([
                "adf",
                "from-markdown",
                "# Title\n\nA **bold** [link](https://example.com)\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```",
            ])
            .assert(),
    );
    let text = adf.to_string();
    assert!(text.contains("heading"));
    assert!(text.contains("strong"));
    assert!(text.contains("link"));
    assert!(text.contains("table"));
    assert!(text.contains("codeBlock"));
}
