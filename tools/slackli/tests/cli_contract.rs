use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn status_outputs_expected_json_shape() {
    let mut cmd = Command::cargo_bin("slackli").expect("binary should build");
    cmd.arg("status")
        .assert()
        .success()
        .stdout(contains("\"ok\": true"))
        .stdout(contains("\"tool\": \"slackli\""))
        .stdout(contains("\"command\": \"status\""))
        .stdout(contains("\"message\": \"slackli foundation ready\""))
        .stdout(contains("\"receive_mode\": \"socket_mode\""));
}

#[test]
fn help_includes_mvp_commands() {
    let mut cmd = Command::cargo_bin("slackli").expect("binary should build");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(contains("send"))
        .stdout(contains("listen"))
        .stdout(contains("search"));
}

#[test]
fn unimplemented_command_returns_structured_error() {
    let mut cmd = Command::cargo_bin("slackli").expect("binary should build");
    cmd.arg("send")
        .assert()
        .success()
        .stdout(contains("\"ok\": false"))
        .stdout(contains("\"code\": \"NOT_IMPLEMENTED\""));
}
