use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

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
fn live_read_requires_project_configuration() {
    Command::cargo_bin("framerli")
        .unwrap()
        .args(["project", "info"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("E_USAGE"))
        .stdout(predicate::str::contains("No Framer project URL configured"));
}

#[test]
fn mock_bridge_runs_core_read() {
    let output = Command::cargo_bin("framerli")
        .unwrap()
        .env("FRAMERLI_BRIDGE_MOCK", "1")
        .env("FRAMER_API_KEY", "mock-key")
        .args([
            "--project",
            "https://framer.com/projects/mock",
            "project",
            "info",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["project"]["name"], "Mock Framer Site");
}

#[test]
fn mock_bridge_runs_confirmed_publish() {
    let output = Command::cargo_bin("framerli")
        .unwrap()
        .env("FRAMERLI_BRIDGE_MOCK", "1")
        .env("FRAMER_API_KEY", "mock-key")
        .args([
            "--project",
            "https://framer.com/projects/mock",
            "--yes",
            "publish",
            "--promote",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["promoted"], true);
}

#[test]
fn project_use_persists_profile_config() {
    let home = tempdir().unwrap();
    let config_path = home.path().join("framerli.yaml");
    Command::cargo_bin("framerli")
        .unwrap()
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "--profile",
            "marketing",
            "project",
            "use",
            "https://framer.com/projects/mock",
        ])
        .assert()
        .success();

    let config = std::fs::read_to_string(config_path).unwrap();
    assert!(config.contains("default_profile: marketing"));
    assert!(config.contains("project: https://framer.com/projects/mock"));
}

#[test]
fn auth_login_persists_env_key_reference_only() {
    let home = tempdir().unwrap();
    let config_path = home.path().join("framerli.yaml");
    Command::cargo_bin("framerli")
        .unwrap()
        .env("MARKETING_FRAMER_KEY", "secret-value")
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "auth",
            "login",
            "--profile",
            "marketing",
            "--project",
            "https://framer.com/projects/mock",
            "--key-env",
            "MARKETING_FRAMER_KEY",
        ])
        .assert()
        .success();

    let config = std::fs::read_to_string(config_path).unwrap();
    assert!(config.contains("key_source: env:MARKETING_FRAMER_KEY"));
    assert!(!config.contains("secret-value"));
}

#[test]
fn env_config_path_and_profile_are_loaded() {
    let home = tempdir().unwrap();
    let config_path = home.path().join("framerli.yaml");
    std::fs::write(
        &config_path,
        "default_profile: marketing\nprofile:\n  marketing:\n    project: https://framer.com/projects/from-config\n    key_source: env:MARKETING_FRAMER_KEY\n",
    )
    .unwrap();

    let output = Command::cargo_bin("framerli")
        .unwrap()
        .env("FRAMERLI_CONFIG", &config_path)
        .env("FRAMERLI_BRIDGE_MOCK", "1")
        .env("MARKETING_FRAMER_KEY", "mock-key")
        .args(["project", "info"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(
        value["meta"]["project"],
        "https://framer.com/projects/from-config"
    );
}
