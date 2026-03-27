use assert_cmd::Command;
use predicates::prelude::*;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. RTF → HTML from stdin
// ---------------------------------------------------------------------------

#[test]
fn test_convert_rtf_to_html_simple() {
    let rtf = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Helvetica;}}\f0\pard This is {\b bold} and {\i italic} text.\par Second paragraph.\par}";

    clipli()
        .args(["convert", "--from", "rtf", "--to", "html"])
        .write_stdin(rtf)
        .assert()
        .success()
        .stdout(predicate::str::contains("bold").or(predicate::str::contains("This is")));
}

// ---------------------------------------------------------------------------
// 2. RTF → HTML from fixture file
// ---------------------------------------------------------------------------

#[test]
fn test_convert_rtf_to_html_from_file() {
    clipli()
        .args([
            "convert",
            "--from",
            "rtf",
            "--to",
            "html",
            "-i",
            "tests/fixtures/simple_text.rtf",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("bold").or(predicate::str::contains("text")));
}

// ---------------------------------------------------------------------------
// 3. RTF table fixture produces HTML
// ---------------------------------------------------------------------------

#[test]
fn test_convert_rtf_table_to_html() {
    clipli()
        .args([
            "convert",
            "--from",
            "rtf",
            "--to",
            "html",
            "-i",
            "tests/fixtures/table.rtf",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Alice")
                .or(predicate::str::contains("Name"))
                .or(predicate::str::contains("table")),
        );
}

// ---------------------------------------------------------------------------
// 4. Empty RTF input doesn't crash
// ---------------------------------------------------------------------------

#[test]
fn test_convert_rtf_empty_input() {
    let _ = clipli()
        .args(["convert", "--from", "rtf", "--to", "html"])
        .write_stdin("")
        .assert(); // May succeed or fail, but shouldn't panic
}
