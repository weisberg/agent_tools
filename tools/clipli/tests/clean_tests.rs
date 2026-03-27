use assert_cmd::Command;
use predicates::prelude::*;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

// ---------------------------------------------------------------------------
// 1. HTML tag stripping — inline styles and formatting tags removed
// ---------------------------------------------------------------------------

#[test]
fn test_convert_strips_html_tags() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin(r#"<p style="mso-line-height:1.5">Hello <b>World</b></p>"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"))
        .stdout(predicate::str::contains("mso-line-height").not());
}

// ---------------------------------------------------------------------------
// 2. Text content preserved through table structures
// ---------------------------------------------------------------------------

#[test]
fn test_convert_preserves_text_content() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("<table><tr><td>Name</td><td>Value</td></tr></table>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Name"))
        .stdout(predicate::str::contains("Value"));
}

// ---------------------------------------------------------------------------
// 3. HTML entities decoded to literal characters
// ---------------------------------------------------------------------------

#[test]
fn test_convert_handles_entities() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("<p>&amp; &lt; &gt; &nbsp;</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("&"))
        .stdout(predicate::str::contains("<"))
        .stdout(predicate::str::contains(">"));
}

// ---------------------------------------------------------------------------
// 4. Empty input handled gracefully
// ---------------------------------------------------------------------------

#[test]
fn test_convert_handles_empty_input() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// 5. <br> tags converted to newlines / separate lines
// ---------------------------------------------------------------------------

#[test]
fn test_convert_br_to_newlines() {
    clipli()
        .args(["convert", "--from", "html", "--to", "plain"])
        .write_stdin("line1<br>line2<br/>line3")
        .assert()
        .success()
        .stdout(predicate::str::contains("line1"))
        .stdout(predicate::str::contains("line2"))
        .stdout(predicate::str::contains("line3"));
}

// ---------------------------------------------------------------------------
// 6. HTML → J2: currency values templatized
// ---------------------------------------------------------------------------

#[test]
fn test_convert_html_to_j2_detects_currency() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>$1,500</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ currency_1 }}"));
}

// ---------------------------------------------------------------------------
// 7. HTML → J2: date values templatized
// ---------------------------------------------------------------------------

#[test]
fn test_convert_html_to_j2_detects_date() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<td>2024-03-15</td>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ date_1 }}"));
}

// ---------------------------------------------------------------------------
// 8. HTML → J2: percentage values templatized
// ---------------------------------------------------------------------------

#[test]
fn test_convert_html_to_j2_detects_percentage() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin("<p>12.5%</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("{{ pct_1 }}"));
}

// ---------------------------------------------------------------------------
// 9. HTML → J2: attribute values are NOT templatized
// ---------------------------------------------------------------------------

#[test]
fn test_convert_html_to_j2_preserves_attributes() {
    clipli()
        .args(["convert", "--from", "html", "--to", "j2"])
        .write_stdin(r#"<a href="2024-01-01.html">Click</a>"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"href="2024-01-01.html""#));
}

// ---------------------------------------------------------------------------
// 10. J2 → HTML: template variables rendered with provided data
// ---------------------------------------------------------------------------

#[test]
fn test_convert_j2_to_html_renders_variables() {
    clipli()
        .args([
            "convert",
            "--from", "j2",
            "--to", "html",
            "-D", r#"{"name":"Alice","amount":"$1000"}"#,
        ])
        .write_stdin("<p>{{ name }} earned {{ amount }}</p>")
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice earned $1000"));
}

// ---------------------------------------------------------------------------
// 11. RTF → HTML: conversion now works via textutil
// ---------------------------------------------------------------------------

#[test]
fn test_convert_rtf_to_html_produces_output() {
    clipli()
        .args(["convert", "--from", "rtf", "--to", "html"])
        .write_stdin(r"{\rtf1\ansi\deff0{\fonttbl{\f0 Helvetica;}}\f0\pard This is {\b bold} text.\par}")
        .assert()
        .success()
        .stdout(predicate::str::contains("bold").or(predicate::str::contains("This is")));
}

// ---------------------------------------------------------------------------
// 12. Invalid format names rejected
// ---------------------------------------------------------------------------

#[test]
fn test_convert_invalid_format() {
    clipli()
        .args(["convert", "--from", "xyz", "--to", "abc"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unsupported")
                .or(predicate::str::contains("Unsupported"))
                .or(predicate::str::contains("invalid value")),
        );
}
