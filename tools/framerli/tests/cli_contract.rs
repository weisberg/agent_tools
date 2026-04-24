use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

#[test]
fn tools_emits_json_command_tree() {
    let output = Command::cargo_bin("framerli")
        .unwrap()
        .arg("tools")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["name"], "framerli");
}

#[test]
fn destructive_command_requires_approval() {
    Command::cargo_bin("framerli")
        .unwrap()
        .args(["deploy", "promote", "dep_123"])
        .assert()
        .code(5)
        .stdout(predicate::str::contains("E_APPROVAL_REQUIRED"));
}

#[test]
fn dry_run_returns_plan_for_mutation() {
    let output = Command::cargo_bin("framerli")
        .unwrap()
        .args([
            "--dry-run",
            "cms",
            "items",
            "add",
            "Blog",
            "--file",
            "items.ndjson",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["mutating"], true);
    assert_eq!(value["data"]["status"], "planned");
}

#[test]
fn live_read_reports_bridge_unavailable() {
    Command::cargo_bin("framerli")
        .unwrap()
        .args(["project", "info"])
        .assert()
        .code(10)
        .stdout(predicate::str::contains("E_BRIDGE_UNAVAILABLE"));
}
