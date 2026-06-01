use serde_json::Value;
use std::collections::HashMap;
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

fn xli_stdin_json(args: &[&str], input: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_xli"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("xli command");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("xli output");
    assert!(
        out.status.success(),
        "xli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("valid json")
}

#[test]
fn value_write_preserves_unrelated_rich_artifacts() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("rich.xlsx");
    create_rich_workbook(&path);

    let before = snapshot_parts(
        &path,
        &[
            "xl/tables/table1.xml",
            "xl/drawings/drawing1.xml",
            "xl/charts/chart1.xml",
            "xl/worksheets/_rels/sheet1.xml.rels",
            "xl/workbook.xml",
        ],
    );

    let write = xli_json(&[
        "write",
        path.to_str().expect("path"),
        "Sheet1!E2",
        "--value",
        "\"patched\"",
    ]);
    assert_eq!(write["status"], "ok");
    assert_eq!(write["warnings"].as_array().expect("warnings").len(), 0);

    let after = snapshot_parts(
        &path,
        &before.keys().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(before, after);

    let sheet_xml =
        String::from_utf8(read_zip_part(&path, "xl/worksheets/sheet1.xml")).expect("sheet xml");
    assert!(sheet_xml.contains("<dataValidations"));
}

#[test]
fn format_preserves_table_chart_and_validation_artifacts() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("rich_format.xlsx");
    create_rich_workbook(&path);

    let before = snapshot_parts(
        &path,
        &[
            "xl/tables/table1.xml",
            "xl/drawings/drawing1.xml",
            "xl/charts/chart1.xml",
            "xl/worksheets/_rels/sheet1.xml.rels",
            "xl/workbook.xml",
        ],
    );

    let format = xli_json(&[
        "format",
        path.to_str().expect("path"),
        "Sheet1!B2:B3",
        "--number-format",
        "currency",
    ]);
    assert_eq!(format["status"], "ok");
    assert_eq!(format["warnings"].as_array().expect("warnings").len(), 0);

    let after = snapshot_parts(
        &path,
        &before.keys().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(before, after);

    let sheet_xml =
        String::from_utf8(read_zip_part(&path, "xl/worksheets/sheet1.xml")).expect("sheet xml");
    assert!(sheet_xml.contains("<dataValidations"));
}

#[test]
fn mixed_batch_preserves_table_chart_and_validation_artifacts() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("rich_batch.xlsx");
    create_rich_workbook(&path);

    let before = snapshot_parts(
        &path,
        &[
            "xl/tables/table1.xml",
            "xl/drawings/drawing1.xml",
            "xl/charts/chart1.xml",
            "xl/worksheets/_rels/sheet1.xml.rels",
        ],
    );

    let input = r#"{"op":"write","address":"Sheet1!E2","formula":"=SUM(B2:B3)"}
{"op":"format","range":"Sheet1!B2:B3","number_format":"currency","bold":true}"#;
    let batch = xli_stdin_json(&["batch", path.to_str().expect("path"), "--stdin"], input);
    assert_eq!(batch["status"], "ok");
    assert_eq!(batch["needs_recalc"], true);
    assert_eq!(batch["warnings"].as_array().expect("warnings").len(), 0);
    assert_eq!(batch["output"]["stats"]["cells_written"], 1);
    assert_eq!(batch["output"]["stats"]["cells_formatted"], 2);

    let after = snapshot_parts(
        &path,
        &before.keys().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(before, after);

    let sheet_xml =
        String::from_utf8(read_zip_part(&path, "xl/worksheets/sheet1.xml")).expect("sheet xml");
    assert!(sheet_xml.contains("<dataValidations"));
    assert!(sheet_xml.contains("<f>SUM(B2:B3)</f>"));
}

#[test]
fn apply_template_preserves_table_chart_and_validation_artifacts() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("rich_apply.xlsx");
    create_rich_workbook(&path);

    let before = snapshot_parts(
        &path,
        &[
            "xl/tables/table1.xml",
            "xl/drawings/drawing1.xml",
            "xl/charts/chart1.xml",
            "xl/worksheets/_rels/sheet1.xml.rels",
        ],
    );

    let apply = xli_json(&[
        "apply",
        path.to_str().expect("path"),
        "basic-table-format",
        "--param",
        "range=Sheet1!A1:B3",
        "--param",
        "number_format=currency",
    ]);
    assert_eq!(apply["status"], "ok");
    assert_eq!(apply["warnings"].as_array().expect("warnings").len(), 0);
    assert_eq!(apply["output"]["ops_executed"], 3);

    let after = snapshot_parts(
        &path,
        &before.keys().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(before, after);

    let sheet_xml =
        String::from_utf8(read_zip_part(&path, "xl/worksheets/sheet1.xml")).expect("sheet xml");
    assert!(sheet_xml.contains("<dataValidations"));
}

#[test]
fn sheet_hide_and_reorder_preserve_artifact_parts_without_warning() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("rich_sheets.xlsx");
    create_rich_workbook(&path);

    let before = snapshot_parts(
        &path,
        &[
            "xl/tables/table1.xml",
            "xl/drawings/drawing1.xml",
            "xl/charts/chart1.xml",
            "xl/worksheets/_rels/sheet1.xml.rels",
        ],
    );

    let hide = xli_json(&["sheet", path.to_str().expect("path"), "hide", "Sheet1"]);
    assert_eq!(hide["status"], "ok");
    assert_eq!(hide["warnings"].as_array().expect("warnings").len(), 0);

    let reorder = xli_json(&[
        "sheet",
        path.to_str().expect("path"),
        "reorder",
        "--order",
        "Other,Sheet1",
    ]);
    assert_eq!(reorder["status"], "ok");
    assert_eq!(reorder["warnings"].as_array().expect("warnings").len(), 0);

    let after = snapshot_parts(
        &path,
        &before.keys().map(String::as_str).collect::<Vec<_>>(),
    );
    assert_eq!(before, after);

    let inspect = xli_json(&["inspect", path.to_str().expect("path")]);
    let sheets = inspect["output"]["sheets"].as_array().expect("sheets");
    assert_eq!(sheets[0]["name"], "Other");
    assert_eq!(sheets[1]["name"], "Sheet1");
}

fn snapshot_parts(path: &std::path::Path, parts: &[&str]) -> HashMap<String, Vec<u8>> {
    parts
        .iter()
        .map(|part| ((*part).to_string(), read_zip_part(path, part)))
        .collect()
}

fn read_zip_part(path: &std::path::Path, part: &str) -> Vec<u8> {
    let file = std::fs::File::open(path).expect("open workbook");
    let mut archive = zip::ZipArchive::new(file).expect("open xlsx archive");
    let mut item = archive.by_name(part).expect("zip part");
    let mut bytes = Vec::new();
    item.read_to_end(&mut bytes).expect("read zip part");
    bytes
}

fn create_rich_workbook(path: &std::path::Path) {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    workbook
        .define_name("SalesRange", "=Sheet1!$B$2:$B$3")
        .expect("defined name");

    let worksheet = workbook.add_worksheet();
    worksheet.write_string(0, 0, "Month").expect("write");
    worksheet.write_string(0, 1, "Sales").expect("write");
    worksheet.write_string(1, 0, "Jan").expect("write");
    worksheet.write_number(1, 1, 10).expect("write");
    worksheet.write_string(2, 0, "Feb").expect("write");
    worksheet.write_number(2, 1, 20).expect("write");
    worksheet
        .add_table(0, 0, 2, 1, &rust_xlsxwriter::Table::new())
        .expect("table");
    let validation = rust_xlsxwriter::DataValidation::new()
        .allow_list_strings(&["open", "closed"])
        .expect("validation");
    worksheet
        .add_data_validation(1, 3, 3, 3, &validation)
        .expect("data validation");

    let mut chart = rust_xlsxwriter::Chart::new(rust_xlsxwriter::ChartType::Column);
    chart
        .add_series()
        .set_categories("Sheet1!$A$2:$A$3")
        .set_values("Sheet1!$B$2:$B$3");
    worksheet.insert_chart(0, 5, &chart).expect("chart");

    let other = workbook.add_worksheet();
    other.set_name("Other").expect("sheet name");
    other.write_string(0, 0, "Formula").expect("write");
    other
        .write_formula(0, 1, "=SUM(Sheet1!B2:B3)")
        .expect("formula");

    workbook.save(path).expect("save");
}
