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

// ---------------------------------------------------------------------------
// 12. capture --json: invalid name produces JSON error envelope
// ---------------------------------------------------------------------------

#[test]
fn test_error_json_capture_invalid_name() {
    let output = clipli()
        .args(["capture", "--name", "../evil", "--json"])
        .output()
        .unwrap();
    // Should fail with exit code 1
    assert!(!output.status.success());
    // Error should be JSON on stdout (not stderr)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""ok":false"#) || stdout.contains(r#""ok": false"#),
        "expected JSON error envelope on stdout, got: {stdout}"
    );
    assert!(
        stdout.contains(r#""code""#),
        "expected error code in JSON, got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 13. error without --json still goes to stderr
// ---------------------------------------------------------------------------

#[test]
fn test_error_non_json_goes_to_stderr() {
    let output = clipli()
        .args(["show", "nonexistent_template_xyz_99999"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error:"),
        "expected plain text error on stderr, got: {stderr}"
    );
    // stdout should be empty (no JSON envelope)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(r#""ok""#),
        "expected no JSON on stdout without --json flag"
    );
}

// ---------------------------------------------------------------------------
// 14. RTF conversion now works (not "not implemented")
// ---------------------------------------------------------------------------

#[test]
fn test_convert_rtf_to_html() {
    clipli()
        .args(["convert", "--from", "rtf", "--to", "html"])
        .write_stdin(r"{\rtf1\ansi\deff0{\fonttbl{\f0 Helvetica;}}\f0\pard Hello world.\par}")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello").or(predicate::str::contains("world")));
}
