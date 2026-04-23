use calamine::{open_workbook, Reader as XlsxReader, SheetType, Xlsx};
use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use xli_core::{col_to_letter, parse_address, parse_range, XliError};
use xli_fs::fingerprint;
use zip::read::ZipArchive;
use zip::result::ZipError;

/// High-level workbook metadata returned by `xli inspect`.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct WorkbookInfo {
    pub file: String,
    pub size_bytes: u64,
    pub fingerprint: String,
    pub sheets: Vec<SheetInfo>,
    pub defined_names: HashMap<String, String>,
    pub has_macros: bool,
}

/// High-level per-sheet metadata returned by `xli inspect`.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SheetInfo {
    pub name: String,
    pub index: u32,
    pub dimensions: Option<String>,
    pub rows: u32,
    pub cols: u32,
    pub formula_count: u32,
    pub tables: Vec<String>,
    pub named_ranges: Vec<String>,
    pub merged_regions: Vec<String>,
    /// True only when this sheet *is* a chart sheet (the entire sheet is one
    /// chart sheet or when it contains an embedded chart object (Issue #25).
    pub is_chart_sheet: bool,
}

/// Inspect an OOXML workbook and summarize its structure.
pub fn inspect(path: &Path) -> Result<WorkbookInfo, XliError> {
    if !path.exists() {
        return Err(XliError::FileNotFound {
            path: path.display().to_string(),
        });
    }

    let size_bytes = path.metadata().map_err(io_error)?.len();
    let workbook_fingerprint = fingerprint(path)?;
    let mut workbook: Xlsx<BufReader<std::fs::File>> =
        open_workbook(path).map_err(calamine_error)?;
    let mut archive = open_workbook_archive(path)?;
    let sheet_part_paths = discover_sheet_parts(&mut archive).unwrap_or_default();

    let has_macros = workbook.vba_project().is_some();
    let tables_loaded = workbook.load_tables().is_ok();
    let merged_regions_loaded = workbook.load_merged_regions().is_ok();

    let defined_names = workbook
        .defined_names()
        .iter()
        .map(|(name, formula)| (name.clone(), formula.clone()))
        .collect::<HashMap<_, _>>();
    let metadata = workbook.sheets_metadata().to_vec();
    let sheet_names = workbook.sheet_names();

    let mut sheets = Vec::with_capacity(sheet_names.len());
    for (index, name) in sheet_names.iter().enumerate() {
        let range = workbook.worksheet_range(name).map_err(calamine_error)?;
        let formulas = workbook.worksheet_formula(name).map_err(calamine_error)?;
        let dimensions = range
            .start()
            .zip(range.end())
            .map(|(start, end)| format_dimension(start, end));
        let (rows, cols) = range.get_size();
        let formula_count = formulas
            .rows()
            .flat_map(|row| row.iter())
            .filter(|formula| !formula.is_empty())
            .count() as u32;

        let tables = if tables_loaded {
            workbook
                .table_names_in_sheet(name)
                .into_iter()
                .map(|table| table.to_string())
                .collect()
        } else {
            Vec::new()
        };
        let merged_regions = if merged_regions_loaded {
            workbook
                .merged_regions_by_sheet(name)
                .into_iter()
                .map(|(_, _, dims)| format_dimension(dims.start, dims.end))
                .collect()
        } else {
            Vec::new()
        };
        let named_ranges = defined_names
            .iter()
            .filter(|(_, formula)| formula_targets_sheet(formula, name))
            .map(|(defined_name, _)| defined_name.clone())
            .collect();
        let is_chart_sheet = metadata
            .get(index)
            .map(|sheet| sheet.typ == SheetType::ChartSheet)
            .unwrap_or(false)
            || sheet_part_paths
                .get(name.as_str())
                .is_some_and(|sheet_path| {
                    has_embedded_chart(&mut archive, sheet_path).unwrap_or(false)
                });

        sheets.push(SheetInfo {
            name: name.clone(),
            index: index as u32,
            dimensions,
            rows: rows as u32,
            cols: cols as u32,
            formula_count,
            tables,
            named_ranges,
            merged_regions,
            is_chart_sheet,
        });
    }

    Ok(WorkbookInfo {
        file: path.display().to_string(),
        size_bytes,
        fingerprint: workbook_fingerprint,
        sheets,
        defined_names,
        has_macros,
    })
}

fn open_workbook_archive(path: &Path) -> Result<ZipArchive<BufReader<File>>, XliError> {
    let file = File::open(path).map_err(io_error)?;
    ZipArchive::new(BufReader::new(file)).map_err(zip_error)
}

fn discover_sheet_parts(
    archive: &mut ZipArchive<BufReader<File>>,
) -> Result<HashMap<String, String>, XliError> {
    let workbook_xml = read_xml_part(archive, "xl/workbook.xml")?;
    let workbook_rels = read_xml_part(archive, "xl/_rels/workbook.xml.rels")?;

    let sheet_relationships = parse_workbook_sheets(&workbook_xml)?;
    let rel_targets = parse_relationship_targets(&workbook_rels, "xl/workbook.xml")?;

    let mut sheet_parts = HashMap::new();
    for (sheet_name, rel_id) in sheet_relationships {
        if let Some(sheet_path) = rel_targets.get(&rel_id) {
            sheet_parts.insert(sheet_name, sheet_path.to_owned());
        }
    }

    Ok(sheet_parts)
}

fn has_embedded_chart(
    archive: &mut ZipArchive<BufReader<File>>,
    sheet_part_path: &str,
) -> Result<bool, XliError> {
    let sheet_rels = rels_path_for_part(sheet_part_path);
    let rels_xml = match read_xml_part_optional(archive, &sheet_rels)? {
        Some(xml) => xml,
        None => return Ok(false),
    };

    let drawing_parts =
        parse_relationship_targets_by_type(&rels_xml, sheet_part_path, |relationship_type| {
            relationship_type.ends_with("/drawing")
        })?;

    for drawing_part in drawing_parts {
        let drawing_rels = rels_path_for_part(&drawing_part);
        let drawing_rels_xml = match read_xml_part_optional(archive, &drawing_rels)? {
            Some(xml) => xml,
            None => continue,
        };

        if parse_relationship_targets_by_type(
            &drawing_rels_xml,
            &drawing_part,
            |relationship_type| relationship_type.contains("/chart"),
        )?
        .is_empty()
        {
            continue;
        }

        return Ok(true);
    }

    Ok(false)
}

fn parse_workbook_sheets(workbook_xml: &[u8]) -> Result<Vec<(String, String)>, XliError> {
    let mut reader = XmlReader::from_reader(Cursor::new(workbook_xml));
    let mut buffer = Vec::new();
    let mut sheets = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                if start.name().local_name().as_ref() != b"sheet" {
                    continue;
                }

                let mut sheet_name = None;
                let mut rel_id = None;

                for attribute in start.attributes() {
                    let attribute = attribute.map_err(parse_xml_error)?;
                    let key = normalize_xml_attr_name(&attribute)?;

                    match key {
                        key if key == "name" => {
                            sheet_name = Some(decode_attribute(&reader, &attribute)?);
                        }
                        key if key == "id" => {
                            rel_id = Some(decode_attribute(&reader, &attribute)?);
                        }
                        _ => {}
                    }
                }

                if let (Some(name), Some(rel_id)) = (sheet_name, rel_id) {
                    sheets.push((name, rel_id));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(io_error_from(error)),
            _ => {}
        }

        buffer.clear();
    }

    Ok(sheets)
}

fn parse_relationship_targets_by_type(
    xml: &[u8],
    base_part_path: &str,
    target_predicate: impl Fn(&str) -> bool,
) -> Result<Vec<String>, XliError> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    let mut targets = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                if start.name().local_name().as_ref() != b"Relationship" {
                    continue;
                }

                let mut rel_type = None;
                let mut rel_target = None;

                for attribute in start.attributes() {
                    let attribute = attribute.map_err(parse_xml_error)?;
                    let key = normalize_xml_attr_name(&attribute)?;

                    match key {
                        key if key == "type" => {
                            rel_type = Some(decode_attribute(&reader, &attribute)?);
                        }
                        key if key == "target" => {
                            rel_target = Some(decode_attribute(&reader, &attribute)?);
                        }
                        _ => {}
                    }
                }

                if let (Some(rel_type), Some(rel_target)) = (rel_type, rel_target) {
                    if target_predicate(&rel_type) {
                        targets.push(resolve_ooxml_path(base_part_path, &rel_target));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(io_error_from(error)),
            _ => {}
        }

        buffer.clear();
    }

    Ok(targets)
}

fn parse_relationship_targets(
    xml: &[u8],
    base_part_path: &str,
) -> Result<HashMap<String, String>, XliError> {
    let mut reader = XmlReader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    let mut rel_targets = HashMap::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) | Ok(Event::Empty(start)) => {
                if start.name().local_name().as_ref() != b"Relationship" {
                    continue;
                }

                let mut rel_id = None;
                let mut rel_target = None;

                for attribute in start.attributes() {
                    let attribute = attribute.map_err(parse_xml_error)?;
                    let key = normalize_xml_attr_name(&attribute)?;

                    match key {
                        key if key == "id" => {
                            rel_id = Some(decode_attribute(&reader, &attribute)?);
                        }
                        key if key == "target" => {
                            rel_target = Some(decode_attribute(&reader, &attribute)?);
                        }
                        _ => {}
                    }
                }

                if let (Some(rel_id), Some(rel_target)) = (rel_id, rel_target) {
                    rel_targets.insert(rel_id, resolve_ooxml_path(base_part_path, &rel_target));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(io_error_from(error)),
            _ => {}
        }

        buffer.clear();
    }

    Ok(rel_targets)
}

fn rels_path_for_part(part_path: &str) -> String {
    let mut iter = part_path.rsplitn(2, '/');
    let file_name = iter.next().unwrap_or_default();
    let parent = iter.next().unwrap_or_default();

    if parent.is_empty() {
        format!("_rels/{file_name}.rels")
    } else {
        format!("{parent}/_rels/{file_name}.rels")
    }
}

fn resolve_ooxml_path(base_part_path: &str, target: &str) -> String {
    let mut parts: Vec<&str> = base_part_path.split('/').collect();
    let _ = parts.pop();

    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                let _ = parts.pop();
            }
            _ => parts.push(segment),
        }
    }

    parts.join("/")
}

fn read_xml_part(
    archive: &mut ZipArchive<BufReader<File>>,
    part_path: &str,
) -> Result<Vec<u8>, XliError> {
    read_xml_part_optional(archive, part_path)?.ok_or_else(|| XliError::OoxmlCorrupt {
        details: format!("Part missing in workbook archive: {part_path}"),
    })
}

fn read_xml_part_optional(
    archive: &mut ZipArchive<BufReader<File>>,
    part_path: &str,
) -> Result<Option<Vec<u8>>, XliError> {
    let mut file = match archive.by_name(part_path) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(zip_error(error)),
    };

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(Some(bytes))
}

fn decode_attribute(
    reader: &XmlReader<Cursor<&[u8]>>,
    attribute: &quick_xml::events::attributes::Attribute,
) -> Result<String, XliError> {
    attribute
        .decode_and_unescape_value(reader.decoder())
        .map(|value| value.into_owned())
        .map_err(io_error_from)
}

fn normalize_xml_attr_name(
    attribute: &quick_xml::events::attributes::Attribute,
) -> Result<String, XliError> {
    let local_name = attribute.key.local_name();
    let key = local_name.as_ref();
    Ok(std::str::from_utf8(key)
        .map_err(io_error_from)?
        .to_ascii_lowercase())
}

fn format_dimension(start: (u32, u32), end: (u32, u32)) -> String {
    let start_col = col_to_letter(start.1);
    let end_col = col_to_letter(end.1);
    format!("{start_col}{}:{end_col}{}", start.0 + 1, end.0 + 1)
}

fn formula_targets_sheet(formula: &str, sheet_name: &str) -> bool {
    if let Ok(range) = parse_range(formula) {
        return range.sheet.as_deref() == Some(sheet_name);
    }

    if let Ok(cell) = parse_address(formula) {
        return cell.sheet.as_deref() == Some(sheet_name);
    }

    false
}

fn io_error(error: std::io::Error) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn calamine_error<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn io_error_from<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn parse_xml_error<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn zip_error(error: ZipError) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::inspect;
    use rust_xlsxwriter::{Chart, ChartType, Workbook};
    use tempfile::tempdir;
    use xli_core::XliError;

    #[test]
    fn inspects_basic_workbook_metadata() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("simple.xlsx");
        let mut workbook = Workbook::new();

        let summary = workbook.add_worksheet();
        summary.set_name("Summary").expect("name");
        summary.write_string(0, 0, "Metric").expect("write");
        summary.write_number(0, 1, 42.0).expect("write");
        summary.write_formula(1, 1, "=SUM(B1:B1)").expect("write");

        let raw = workbook.add_worksheet();
        raw.set_name("Raw Data").expect("name");
        raw.write_string(0, 0, "Value").expect("write");

        workbook.save(&path).expect("save");

        let info = inspect(&path).expect("inspect");
        assert_eq!(info.sheets.len(), 2);
        assert_eq!(info.sheets[0].name, "Summary");
        assert_eq!(info.sheets[0].formula_count, 1);
        assert_eq!(info.sheets[1].name, "Raw Data");
        assert!(info.fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn worksheet_with_embedded_chart_is_chart_sheet() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("embedded_chart.xlsx");

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        worksheet.write_string(0, 0, "Month").expect("write");
        worksheet.write_string(1, 0, "Jan").expect("write");
        worksheet.write_number(1, 1, 10.0).expect("write");
        worksheet.write_number(2, 1, 20.0).expect("write");
        worksheet.write_number(3, 1, 30.0).expect("write");

        let mut chart = Chart::new(ChartType::Column);
        chart.add_series().set_values("Sheet1!$B$1:$B$4");
        worksheet.insert_chart(0, 2, &chart).expect("insert chart");

        workbook.save(&path).expect("save");

        let info = inspect(&path).expect("inspect");
        assert_eq!(info.sheets.len(), 1);
        assert!(info.sheets[0].is_chart_sheet);
    }

    #[test]
    fn chartsheet_is_chart_sheet() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("chartsheet.xlsx");

        let mut workbook = Workbook::new();
        let data_sheet = workbook.add_worksheet();
        data_sheet.write_string(0, 0, "Jan").expect("write");
        data_sheet.write_number(0, 1, 10.0).expect("write");
        data_sheet.write_string(1, 0, "Feb").expect("write");
        data_sheet.write_number(1, 1, 20.0).expect("write");
        data_sheet.write_string(2, 0, "Mar").expect("write");
        data_sheet.write_number(2, 1, 30.0).expect("write");

        let chart_sheet = workbook.add_chartsheet();
        let mut chart = Chart::new(ChartType::Line);
        chart.add_series().set_values("Sheet1!$B$1:$B$3");
        chart_sheet
            .insert_chart(0, 0, &chart)
            .expect("insert chart");

        workbook.save(&path).expect("save");

        let info = inspect(&path).expect("inspect");
        assert_eq!(info.sheets.len(), 2);
        assert_eq!(info.sheets[0].name, "Sheet1");
        assert_eq!(info.sheets[1].name, "Chart1");
        assert!(info.sheets[1].is_chart_sheet);
    }

    #[test]
    fn missing_workbook_returns_file_not_found() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing.xlsx");
        let error = inspect(&path).expect_err("missing");
        assert!(matches!(error, XliError::FileNotFound { .. }));
    }
}
