use serde_json::Value;
use std::io::Read;
use std::process::Command;
use std::process::Output;
use tempfile::tempdir;

#[test]
fn create_and_inspect_workbook() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");

    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args([
            "create",
            path.to_str().expect("path"),
            "--sheets",
            "Summary,Data",
        ])
        .output()
        .expect("create");
    assert!(output.status.success());

    let inspect = run_json(["inspect", path.to_str().expect("path")]);
    assert_eq!(inspect["status"], "ok");
    assert_eq!(
        inspect["output"]["sheets"]
            .as_array()
            .map(|items| items.len()),
        Some(2)
    );
}

#[test]
fn write_and_read_cell_round_trip() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary,Data",
    ]);

    let write = run_json([
        "write",
        path.to_str().expect("path"),
        "Data!A1",
        "--value",
        "42",
    ]);
    assert_eq!(write["status"], "ok");

    let read = run_json(["read", path.to_str().expect("path"), "Data!A1"]);
    assert_eq!(read["status"], "ok");
    assert_eq!(read["output"]["value"], 42.0);
}

#[test]
fn batch_writes_multiple_cells_atomically() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary,Data",
    ]);

    let batch_input = r#"{"op":"write","address":"Data!A1","value":100}
{"op":"write","address":"Data!A2","value":200}
{"op":"write","address":"Data!A3","formula":"=SUM(A1:A2)"}"#;
    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(["batch", path.to_str().expect("path"), "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(batch_input.as_bytes())?;
            child.wait_with_output()
        })
        .expect("batch");
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["stats"]["cells_written"], 3);

    let read = run_json([
        "read",
        path.to_str().expect("path"),
        "Data!A1:A3",
        "--headers",
    ]);
    assert_eq!(read["status"], "ok");
}

#[test]
fn sheet_add_rename_and_hide() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary,Data",
    ]);

    let add = run_json(["sheet", path.to_str().expect("path"), "add", "Charts"]);
    assert_eq!(add["status"], "ok");

    let rename = run_json([
        "sheet",
        path.to_str().expect("path"),
        "rename",
        "Charts",
        "--to",
        "Dashboard",
    ]);
    assert_eq!(rename["status"], "ok");

    let hide = run_json(["sheet", path.to_str().expect("path"), "hide", "Dashboard"]);
    assert_eq!(hide["status"], "ok");

    let inspect = run_json(["inspect", path.to_str().expect("path")]);
    let sheet_names = inspect["output"]["sheets"]
        .as_array()
        .expect("sheet list")
        .iter()
        .map(|sheet| sheet["name"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(sheet_names.contains(&"Dashboard".to_string()));
}

#[test]
fn format_command_keeps_workbook_openable() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary",
    ]);
    run_status([
        "write",
        path.to_str().expect("path"),
        "Summary!A1",
        "--value",
        "\"hello\"",
    ]);

    let format = run_json([
        "format",
        path.to_str().expect("path"),
        "Summary!A1:A1",
        "--bold",
        "--fill",
        "4472C4",
        "--font-color",
        "FFFFFF",
    ]);
    assert_eq!(format["status"], "ok");

    let inspect = run_json(["inspect", path.to_str().expect("path")]);
    assert_eq!(inspect["status"], "ok");
}

#[test]
fn format_command_resolves_number_format_aliases() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary",
    ]);
    run_status([
        "write",
        path.to_str().expect("path"),
        "Summary!A1",
        "--value",
        "1234",
    ]);

    let format = run_json([
        "format",
        path.to_str().expect("path"),
        "Summary!A1:A1",
        "--number-format",
        "currency",
    ]);
    assert_eq!(format["status"], "ok");

    let styles = read_styles_xml(&path);
    assert!(
        styles.contains("$#,##0;[Red]($#,##0)"),
        "styles.xml should contain resolved currency format, got: {styles}"
    );
}

#[test]
fn sheet_add_dry_run_does_not_modify_workbook() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary,Data",
    ]);

    let before = run_json(["inspect", path.to_str().expect("path")]);
    let before_len = before["output"]["sheets"].as_array().expect("sheets").len();

    let dry_run = run_json([
        "sheet",
        path.to_str().expect("path"),
        "--dry-run",
        "add",
        "Charts",
    ]);
    assert_eq!(dry_run["status"], "ok");
    assert_eq!(dry_run["commit_mode"], "dry_run");

    let after = run_json(["inspect", path.to_str().expect("path")]);
    assert_eq!(
        after["output"]["sheets"].as_array().expect("sheets").len(),
        before_len
    );
}

#[test]
fn sheet_add_with_bad_fingerprint_fails() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    run_status([
        "create",
        path.to_str().expect("path"),
        "--sheets",
        "Summary,Data",
    ]);

    let result = run_output([
        "sheet",
        path.to_str().expect("path"),
        "--expect-fingerprint",
        "sha256:0000",
        "add",
        "Charts",
    ]);

    assert!(!result.status.success());
    let json: Value = serde_json::from_slice(&result.stdout).expect("json");
    assert_eq!(json["status"], "error");
    assert_eq!(json["errors"][0]["code"], "FINGERPRINT_MISMATCH");
}

fn run_status<const N: usize>(args: [&str; N]) {
    let output = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(args)
        .output()
        .expect("command");
    assert!(output.status.success());
}

fn run_json<const N: usize>(args: [&str; N]) -> Value {
    let output = run_output(args);
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("json")
}

fn read_styles_xml(path: &std::path::Path) -> String {
    let file = std::fs::File::open(path).expect("open workbook");
    let mut archive = zip::ZipArchive::new(file).expect("open xlsx archive");
    let mut styles = archive.by_name("xl/styles.xml").expect("styles.xml");
    let mut xml = String::new();
    styles.read_to_string(&mut xml).expect("read styles.xml");
    xml
}

fn run_output<const N: usize>(args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(args)
        .output()
        .expect("command")
}
