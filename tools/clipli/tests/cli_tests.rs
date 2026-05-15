use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn clipli() -> Command {
    Command::cargo_bin("clipli").unwrap()
}

fn isolated_clipli(home: &TempDir) -> Command {
    let mut cmd = clipli();
    cmd.env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"));
    cmd
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
        .stdout(predicate::str::contains("clipboard").or(predicate::str::contains("Clipboard")))
        .stdout(predicate::str::contains("SUBCOMMAND").or(predicate::str::contains("Commands")));
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
        .stdout(predicate::str::contains("clipli 1.0.0"));
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
    clipli().arg("nonexistent").assert().failure();
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
            "--from",
            "j2",
            "--to",
            "html",
            "-D",
            r#"{"name":"Alice"}"#,
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
            predicate::str::contains("requires --output").or(predicate::str::contains("binary")),
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
    clipli().arg("list").assert().success();
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

#[test]
fn test_doctor_json_skip_clipboard() {
    clipli()
        .args(["doctor", "--json", "--skip-clipboard"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""checks""#))
        .stdout(predicate::str::contains(r#""pasteboard""#))
        .stdout(predicate::str::contains(r#""skipped""#));
}

#[test]
fn test_excel_svg_dry_run() {
    clipli()
        .args(["excel", "-", "--copy-as", "svg", "--dry-run"])
        .write_stdin("Name,Revenue\nAlice,$1200\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("<svg"))
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Revenue"));
}

#[test]
fn test_excel_png_dry_run_requires_output_file() {
    clipli()
        .args(["excel", "-", "--copy-as", "png", "--dry-run"])
        .write_stdin("Name,Revenue\nAlice,$1200\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("PNG dry-run requires --out-file"));
}

#[test]
fn test_excel_png_dry_run_writes_png_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let png_path = dir.path().join("table.png");

    clipli()
        .args([
            "excel",
            "-",
            "--copy-as",
            "png",
            "--dry-run",
            "--out-file",
            png_path.to_str().unwrap(),
        ])
        .write_stdin("Name,Revenue\nAlice,$1200\n")
        .assert()
        .success();

    let png = std::fs::read(png_path).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn test_excel_json_input_with_preset_dry_run() {
    clipli()
        .args([
            "excel",
            "-",
            "--input-format",
            "json",
            "--preset",
            "finance",
            "--copy-as",
            "svg",
            "--dry-run",
        ])
        .write_stdin(r#"[{"Name":"Alice","Revenue":1200,"Margin":0.42}]"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("<svg"))
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Revenue"));
}

#[test]
fn test_preview_stdin_writes_json_path_without_clipboard() {
    let home = tempfile::TempDir::new().unwrap();
    let out_dir = tempfile::TempDir::new().unwrap();
    let out = out_dir.path().join("preview.html");

    let output = isolated_clipli(&home)
        .args(["preview", "--output", out.to_str().unwrap(), "--json"])
        .write_stdin("<p>Preview me</p>")
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(std::fs::read_to_string(out).unwrap(), "<p>Preview me</p>");
}

#[test]
fn test_completions_emit_shell_script() {
    clipli()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef clipli"));
}

#[test]
fn test_history_record_search_show_restore_dry_run() {
    let home = tempfile::TempDir::new().unwrap();
    let input_dir = tempfile::TempDir::new().unwrap();
    let input = input_dir.path().join("payload.txt");
    std::fs::write(&input, "Quarterly launch notes").unwrap();

    let output = isolated_clipli(&home)
        .args([
            "history",
            "record",
            "--type",
            "plain",
            "--input",
            input.to_str().unwrap(),
            "--source-app",
            "fixture.app",
            "--sensitive",
            "allow",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = json["entries"][0]["id"].as_str().unwrap().to_string();

    isolated_clipli(&home)
        .args(["history", "search", "launch", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&id));

    isolated_clipli(&home)
        .args(["history", "show", &id, "--content", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Quarterly launch notes"));

    isolated_clipli(&home)
        .args(["history", "restore", &id, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Quarterly launch notes"));

    let restore_json = isolated_clipli(&home)
        .args(["history", "restore", &id, "--dry-run", "--json"])
        .output()
        .unwrap();
    assert!(restore_json.status.success());
    let json: Value = serde_json::from_slice(&restore_json.stdout).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["id"], id);
}

#[test]
fn test_history_filters_and_prune() {
    let home = tempfile::TempDir::new().unwrap();
    let input_dir = tempfile::TempDir::new().unwrap();
    let alpha = input_dir.path().join("alpha.txt");
    let beta = input_dir.path().join("beta.txt");
    let gamma = input_dir.path().join("gamma.html");
    std::fs::write(&alpha, "Alpha launch notes").unwrap();
    std::fs::write(&beta, "Beta launch notes").unwrap();
    std::fs::write(&gamma, "<p>Gamma launch notes</p>").unwrap();

    for (path, pb_type, source) in [
        (&alpha, "plain", "alpha.app"),
        (&beta, "plain", "beta.app"),
        (&gamma, "html", "alpha.app"),
    ] {
        isolated_clipli(&home)
            .args([
                "history",
                "record",
                "--type",
                pb_type,
                "--input",
                path.to_str().unwrap(),
                "--source-app",
                source,
                "--sensitive",
                "allow",
                "--json",
            ])
            .assert()
            .success();
    }

    isolated_clipli(&home)
        .args(["history", "list", "--source-app", "alpha", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha.app"))
        .stdout(predicate::str::contains("Gamma launch notes").not());

    isolated_clipli(&home)
        .args(["history", "search", "launch", "--type", "html", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("public.html"))
        .stdout(predicate::str::contains("public.utf8-plain-text").not());

    let dry_run = isolated_clipli(&home)
        .args([
            "history",
            "prune",
            "--keep-latest",
            "2",
            "--dry-run",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(dry_run.status.success());
    let json: Value = serde_json::from_slice(&dry_run.stdout).unwrap();
    assert_eq!(json["result"]["removed"], 1);
    assert_eq!(json["result"]["dry_run"], true);

    isolated_clipli(&home)
        .args(["history", "prune", "--keep-latest", "2", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""removed": 1"#));

    let list = isolated_clipli(&home)
        .args(["history", "list", "--limit", "10", "--json"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let json: Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(json["entries"].as_array().unwrap().len(), 2);
}

#[test]
fn test_history_record_skips_sensitive_payload_by_default() {
    let home = tempfile::TempDir::new().unwrap();
    let input_dir = tempfile::TempDir::new().unwrap();
    let input = input_dir.path().join("secret.txt");
    std::fs::write(&input, "NOTION_API_KEY=secret").unwrap();

    let output = isolated_clipli(&home)
        .args([
            "history",
            "record",
            "--type",
            "plain",
            "--input",
            input.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["entries"][0]["redacted"], true);
    assert!(json["entries"][0]["payload_path"].is_null());
}
