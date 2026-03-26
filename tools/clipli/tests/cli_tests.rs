use assert_cmd::Command;
use predicates::prelude::*;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. Help output
// ---------------------------------------------------------------------------

#[test]
fn test_help_output() {
    clipli()
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("clipboard").or(predicate::str::contains("Clipboard")),
        )
        .stdout(
            predicate::str::contains("SUBCOMMAND").or(predicate::str::contains("Commands")),
        );
}

// ---------------------------------------------------------------------------
// 2. Version output
// ---------------------------------------------------------------------------

#[test]
fn test_version_output() {
    clipli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("clipli"));
}

// ---------------------------------------------------------------------------
// 3. No args shows help / usage on stderr and exits with error
// ---------------------------------------------------------------------------

#[test]
fn test_no_args_shows_help() {
    clipli()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("usage")));
}

// ---------------------------------------------------------------------------
// 4. Invalid subcommand
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_subcommand() {
    clipli()
        .arg("nonexistent")
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// 5. convert: html -> plain
// ---------------------------------------------------------------------------

#[test]
fn test_convert_html_to_plain() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("<p>Hello</p><p>World</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello"))
        .stdout(predicate::str::contains("World"));
}

// ---------------------------------------------------------------------------
// 6. convert: html -> j2 (templatization)
// ---------------------------------------------------------------------------

#[test]
fn test_convert_html_to_j2() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>$1,234</td><td>2024-03-15</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{"));
}

// ---------------------------------------------------------------------------
// 7. convert: j2 -> html (render template with data)
// ---------------------------------------------------------------------------

#[test]
fn test_convert_j2_to_html() {
    clipli()
        .args([
            "convert",
            "--from", "j2",
            "--to", "html",
            "-D", r#"{"name":"Alice"}"#,
        ])
        .write_stdin("<p>Hello {{ name }}</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello Alice"));
}

// ---------------------------------------------------------------------------
// 8. convert: unsupported format
// ---------------------------------------------------------------------------

#[test]
fn test_convert_unsupported() {
    clipli()
        .args(["convert", "--from", "pdf", "--to", "html"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unsupported")
                .or(predicate::str::contains("Unsupported"))
                .or(predicate::str::contains("invalid value")),
        );
}

// ---------------------------------------------------------------------------
// 9. read: binary type without --output should fail
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires macOS pasteboard"]
fn test_read_binary_without_output() {
    clipli()
        .args(["read", "--type", "png"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("requires --output")
                .or(predicate::str::contains("binary")),
        );
}

// ---------------------------------------------------------------------------
// 10. capture: invalid name (contains spaces)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires macOS pasteboard"]
fn test_capture_invalid_name() {
    clipli()
        .args(["capture", "--name", "hello world"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid").or(predicate::str::contains("Invalid")));
}

// ---------------------------------------------------------------------------
// 11. list: works even with no stored templates
// ---------------------------------------------------------------------------

#[test]
fn test_list_empty_store() {
    clipli()
        .arg("list")
        .assert()
        .success();
}
