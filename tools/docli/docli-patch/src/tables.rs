use docli_core::DocliError;

use crate::part_graph::PartGraph;

/// Update a single cell in a table.
///
/// Locates the table by byte span, then finds the specified row and column,
/// and replaces the cell's text content.
pub fn update_cell(
    graph: &mut PartGraph,
    part_path: &str,
    table_byte_offset: usize,
    table_byte_end: usize,
    row: usize,
    col: usize,
    content: &str,
) -> Result<(), DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    if table_byte_end > xml_str.len() {
        return Err(DocliError::InvalidOperation {
            message: format!(
                "table_byte_end ({table_byte_end}) exceeds part length ({})",
                xml_str.len()
            ),
        });
    }

    let table_xml = &xml_str[table_byte_offset..table_byte_end];

    // Parse the table to find rows and cells.
    let rows = find_table_rows(table_xml);

    if row >= rows.len() {
        return Err(DocliError::InvalidTarget {
            message: format!(
                "row {row} out of range (table has {} rows)",
                rows.len()
            ),
        });
    }

    let cells = find_row_cells(&rows[row].content);
    if col >= cells.len() {
        return Err(DocliError::InvalidTarget {
            message: format!(
                "col {col} out of range (row has {} cells)",
                cells.len()
            ),
        });
    }

    let cell = &cells[col];

    // Build a new cell with the same structure but new text content.
    let escaped = escape_xml(content);
    let new_cell = format!(
        "<w:tc><w:p><w:r><w:t>{escaped}</w:t></w:r></w:p></w:tc>"
    );

    // Calculate absolute positions.
    let cell_abs_start = table_byte_offset + rows[row].offset + cell.offset;
    let cell_abs_end = cell_abs_start + cell.content.len();

    let mut result = String::with_capacity(xml_str.len());
    result.push_str(&xml_str[..cell_abs_start]);
    result.push_str(&new_cell);
    result.push_str(&xml_str[cell_abs_end..]);

    graph.set_xml(part_path, result.into_bytes());
    Ok(())
}

/// Append a row to a table.
pub fn append_row(
    graph: &mut PartGraph,
    part_path: &str,
    table_byte_offset: usize,
    table_byte_end: usize,
    cells: &[String],
) -> Result<(), DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    if table_byte_end > xml_str.len() {
        return Err(DocliError::InvalidOperation {
            message: format!(
                "table_byte_end ({table_byte_end}) exceeds part length ({})",
                xml_str.len()
            ),
        });
    }

    let table_xml = &xml_str[table_byte_offset..table_byte_end];

    // Build new row XML.
    let mut row_xml = String::from("<w:tr>");
    for cell in cells {
        let escaped = escape_xml(cell);
        row_xml.push_str(&format!(
            "<w:tc><w:p><w:r><w:t>{escaped}</w:t></w:r></w:p></w:tc>"
        ));
    }
    row_xml.push_str("</w:tr>");

    // Insert before the closing </w:tbl> tag.
    let close_tag = "</w:tbl>";
    let insert_rel = table_xml.rfind(close_tag).ok_or_else(|| {
        DocliError::InvalidDocx {
            message: "missing </w:tbl> closing tag".into(),
        }
    })?;
    let insert_abs = table_byte_offset + insert_rel;

    let mut result = String::with_capacity(xml_str.len() + row_xml.len());
    result.push_str(&xml_str[..insert_abs]);
    result.push_str(&row_xml);
    result.push_str(&xml_str[insert_abs..]);

    graph.set_xml(part_path, result.into_bytes());
    Ok(())
}

struct Span {
    offset: usize,
    content: String,
}

fn find_table_rows(table_xml: &str) -> Vec<Span> {
    let mut rows = Vec::new();
    let mut search_from = 0;

    while let Some(start) = table_xml[search_from..].find("<w:tr") {
        let abs_start = search_from + start;
        // Find the matching </w:tr>.
        if let Some(end_rel) = table_xml[abs_start..].find("</w:tr>") {
            let abs_end = abs_start + end_rel + "</w:tr>".len();
            rows.push(Span {
                offset: abs_start,
                content: table_xml[abs_start..abs_end].to_string(),
            });
            search_from = abs_end;
        } else {
            break;
        }
    }

    rows
}

fn find_row_cells(row_xml: &str) -> Vec<Span> {
    let mut cells = Vec::new();
    let mut search_from = 0;

    while let Some(start) = row_xml[search_from..].find("<w:tc") {
        let abs_start = search_from + start;
        if let Some(end_rel) = row_xml[abs_start..].find("</w:tc>") {
            let abs_end = abs_start + end_rel + "</w:tc>".len();
            cells.push(Span {
                offset: abs_start,
                content: row_xml[abs_start..abs_end].to_string(),
            });
            search_from = abs_end;
        } else {
            break;
        }
    }

    cells
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::part_graph::PartData;

    fn make_graph(path: &str, xml: &str) -> PartGraph {
        let mut parts = HashMap::new();
        parts.insert(path.to_string(), PartData::Xml(xml.as_bytes().to_vec()));
        PartGraph { parts }
    }

    fn simple_table() -> String {
        r#"<w:body><w:tbl><w:tblPr/><w:tr><w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>"#.to_string()
    }

    #[test]
    fn update_cell_changes_content() {
        let xml = simple_table();
        let tbl_start = xml.find("<w:tbl>").unwrap();
        let tbl_end = xml.find("</w:tbl>").unwrap() + "</w:tbl>".len();

        let mut graph = make_graph("word/document.xml", &xml);
        update_cell(
            &mut graph,
            "word/document.xml",
            tbl_start,
            tbl_end,
            1,
            0,
            "Updated",
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("Updated"));
        assert!(result.contains("A1")); // Other cells unchanged.
        assert!(result.contains("B1"));
        assert!(result.contains("B2"));
    }

    #[test]
    fn update_cell_out_of_range() {
        let xml = simple_table();
        let tbl_start = xml.find("<w:tbl>").unwrap();
        let tbl_end = xml.find("</w:tbl>").unwrap() + "</w:tbl>".len();

        let mut graph = make_graph("word/document.xml", &xml);
        let result = update_cell(
            &mut graph,
            "word/document.xml",
            tbl_start,
            tbl_end,
            5,
            0,
            "X",
        );
        assert!(result.is_err());
    }

    #[test]
    fn append_row_adds_to_end() {
        let xml = simple_table();
        let tbl_start = xml.find("<w:tbl>").unwrap();
        let tbl_end = xml.find("</w:tbl>").unwrap() + "</w:tbl>".len();

        let mut graph = make_graph("word/document.xml", &xml);
        append_row(
            &mut graph,
            "word/document.xml",
            tbl_start,
            tbl_end,
            &["A3".to_string(), "B3".to_string()],
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("A3"));
        assert!(result.contains("B3"));
        // Verify the new row is before </w:tbl>.
        let a3_pos = result.find("A3").unwrap();
        let tbl_close = result.find("</w:tbl>").unwrap();
        assert!(a3_pos < tbl_close);
    }
}
