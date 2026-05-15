use serde_json::Value;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use tempfile::tempdir;

fn xli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(args)
        .output()
        .expect("xli command")
}

fn xli_json(args: &[&str]) -> Value {
    let out = xli(args);
    assert!(
        out.status.success(),
        "xli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("valid json")
}

fn create_workbook(path: &std::path::Path) {
    let out = xli(&["create", path.to_str().unwrap()]);
    assert!(out.status.success());
}

fn batch_stdin(path: &std::path::Path, input: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(["batch", path.to_str().unwrap(), "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("batch");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "batch failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("valid json")
}

#[test]
fn batch_mixed_write_and_format() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"write","address":"Sheet1!A1","value":10}
{"op":"write","address":"Sheet1!A2","value":20}
{"op":"format","range":"Sheet1!A1:A2","bold":true}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["ops_executed"], 3);
    assert_eq!(json["warnings"].as_array().expect("warnings").len(), 0);
}

#[test]
fn batch_resolves_number_format_aliases() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"write","address":"Sheet1!A1","value":0.125}
{"op":"format","range":"Sheet1!A1:A1","number_format":"percent_1dp"}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");

    let styles = read_styles_xml(&path);
    assert!(
        styles.contains("0.0%"),
        "styles.xml should contain resolved percentage format, got: {styles}"
    );
}

#[test]
fn batch_with_formulas_sets_needs_recalc() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"write","address":"Sheet1!A1","value":10}
{"op":"write","address":"Sheet1!A2","formula":"=A1*2"}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["needs_recalc"], true);
}

#[test]
fn write_only_batch_preserves_chart_artifacts_without_warning() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("chart_batch.xlsx");
    create_chart_workbook(&path);
    let drawing_before = read_zip_part(&path, "xl/drawings/drawing1.xml");
    let chart_before = read_zip_part(&path, "xl/charts/chart1.xml");
    let rels_before = read_zip_part(&path, "xl/worksheets/_rels/sheet1.xml.rels");

    let input = r#"{"op":"write","address":"Sheet1!C2","value":"patched"}
{"op":"write","address":"Sheet1!C3","formula":"=SUM(B2:B3)"}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["needs_recalc"], true);
    assert_eq!(json["warnings"].as_array().expect("warnings").len(), 0);
    assert_eq!(json["output"]["stats"]["cells_written"], 2);
    assert_eq!(json["output"]["stats"]["formulas_written"], 1);

    assert_eq!(
        drawing_before,
        read_zip_part(&path, "xl/drawings/drawing1.xml")
    );
    assert_eq!(chart_before, read_zip_part(&path, "xl/charts/chart1.xml"));
    assert_eq!(
        rels_before,
        read_zip_part(&path, "xl/worksheets/_rels/sheet1.xml.rels")
    );
}

#[test]
fn batch_without_formulas_no_recalc() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"write","address":"Sheet1!A1","value":1}
{"op":"write","address":"Sheet1!A2","value":2}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["needs_recalc"], false);
}

#[test]
fn batch_sheet_ops() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"sheet","action":{"action":"add","name":"NewSheet"}}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert!(
        json["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning
                .as_str()
                .is_some_and(|text| text.contains("umya-spreadsheet fallback"))),
        "unsupported batch sheet add should disclose fallback usage"
    );

    let inspect = xli_json(&["inspect", path.to_str().unwrap()]);
    let sheets = inspect["output"]["sheets"].as_array().expect("sheets");
    let names: Vec<&str> = sheets.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"NewSheet"));
}

#[test]
fn batch_tracks_cells_written() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"write","address":"Sheet1!A1","value":1}
{"op":"write","address":"Sheet1!A2","value":2}
{"op":"write","address":"Sheet1!A3","value":3}
{"op":"write","address":"Sheet1!A4","value":4}
{"op":"write","address":"Sheet1!A5","value":5}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["stats"]["cells_written"], 5);
}

#[test]
fn batch_tracks_formulas_written() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let input = r#"{"op":"write","address":"Sheet1!A1","value":10}
{"op":"write","address":"Sheet1!A2","formula":"=A1+1"}
{"op":"write","address":"Sheet1!A3","formula":"=A1+A2"}"#;

    let json = batch_stdin(&path, input);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["stats"]["formulas_written"], 2);
}

#[test]
fn batch_from_file() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let ops_path = dir.path().join("ops.ndjson");
    let ops_content = r#"{"op":"write","address":"Sheet1!A1","value":42}
{"op":"write","address":"Sheet1!B1","value":99}"#;
    std::fs::write(&ops_path, ops_content).expect("write ops file");

    let json = xli_json(&[
        "batch",
        path.to_str().unwrap(),
        "--file-input",
        ops_path.to_str().unwrap(),
    ]);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["stats"]["cells_written"], 2);
}

#[test]
fn batch_empty_stdin_succeeds() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("test.xlsx");
    create_workbook(&path);

    let json = batch_stdin(&path, "");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"]["ops_executed"], 0);
}

fn read_styles_xml(path: &std::path::Path) -> String {
    let file = std::fs::File::open(path).expect("open workbook");
    let mut archive = zip::ZipArchive::new(file).expect("open xlsx archive");
    let mut styles = archive.by_name("xl/styles.xml").expect("styles.xml");
    let mut xml = String::new();
    styles.read_to_string(&mut xml).expect("read styles.xml");
    xml
}

fn read_zip_part(path: &std::path::Path, part: &str) -> Vec<u8> {
    let file = std::fs::File::open(path).expect("open workbook");
    let mut archive = zip::ZipArchive::new(file).expect("open xlsx archive");
    let mut item = archive.by_name(part).expect("zip part");
    let mut bytes = Vec::new();
    item.read_to_end(&mut bytes).expect("read zip part");
    bytes
}

fn create_chart_workbook(path: &std::path::Path) {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.write_string(0, 0, "Month").expect("write");
    worksheet.write_string(1, 0, "Jan").expect("write");
    worksheet.write_number(1, 1, 10).expect("write");
    worksheet.write_string(2, 0, "Feb").expect("write");
    worksheet.write_number(2, 1, 20).expect("write");
    let mut chart = rust_xlsxwriter::Chart::new(rust_xlsxwriter::ChartType::Column);
    chart
        .add_series()
        .set_categories("Sheet1!$A$2:$A$3")
        .set_values("Sheet1!$B$2:$B$3");
    worksheet.insert_chart(0, 3, &chart).expect("chart");
    workbook.save(path).expect("save");
}
