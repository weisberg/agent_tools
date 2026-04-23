use docli_core::DocliError;

use crate::part_graph::PartGraph;
use crate::run_split::{split_runs_at_offsets, RunFragment};
use crate::runs::{fragment_to_xml, merge_adjacent_runs};

/// Replace text at the specified character range in a paragraph within an XML part.
///
/// - Parse the XML part
/// - Find the paragraph at the given byte offset
/// - Use `split_runs_at_offsets` to isolate the text range
/// - Replace with new text, preserving first-run formatting
/// - Rebuild the XML
pub fn replace_text_in_part(
    graph: &mut PartGraph,
    part_path: &str,
    paragraph_byte_offset: usize,
    char_offset: usize,
    char_end: usize,
    new_text: &str,
) -> Result<(), DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str =
        std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
            message: format!("invalid UTF-8: {e}"),
        })?;

    // Find the paragraph element at the given byte offset.
    let para_start = find_paragraph_start(xml_str, paragraph_byte_offset)?;
    let para_end = find_paragraph_end(xml_str, para_start)?;

    let para_xml = &xml_str[para_start..para_end];

    // Split runs at the character offsets.
    let split =
        split_runs_at_offsets(para_xml.as_bytes(), char_offset, char_end)?;

    // Get formatting from the first target run, or first before run, or None.
    let props = split
        .target_runs
        .first()
        .and_then(|r| r.properties.clone())
        .or_else(|| {
            split
                .before_runs
                .last()
                .and_then(|r| r.properties.clone())
        });

    // Build replacement run with new text.
    let replacement = RunFragment {
        properties: props,
        text: new_text.to_string(),
    };

    // Assemble all runs: before + replacement + after.
    let mut all_runs = split.before_runs.clone();
    if !new_text.is_empty() {
        all_runs.push(replacement);
    }
    all_runs.extend(split.after_runs.iter().cloned());

    let merged = merge_adjacent_runs(&all_runs);

    // Rebuild the paragraph XML. Extract paragraph properties (w:pPr) if any.
    let para_props = extract_paragraph_properties(para_xml);
    let mut new_para = String::from("<w:p");

    // Preserve any attributes on the paragraph element.
    let tag_end = para_xml.find('>').unwrap_or(3);
    let tag_content = &para_xml[4..tag_end];
    if !tag_content.is_empty() && !tag_content.starts_with('>') {
        // Has attributes like xmlns.
        new_para.push_str(tag_content);
    }
    new_para.push('>');

    if let Some(ppr) = para_props {
        new_para.push_str(&ppr);
    }
    for frag in &merged {
        new_para.push_str(&fragment_to_xml(frag));
    }
    new_para.push_str("</w:p>");

    // Replace the paragraph in the full XML.
    let mut result = String::with_capacity(
        xml_str.len() - (para_end - para_start) + new_para.len(),
    );
    result.push_str(&xml_str[..para_start]);
    result.push_str(&new_para);
    result.push_str(&xml_str[para_end..]);

    graph.set_xml(part_path, result.into_bytes());
    Ok(())
}

/// Find the start of the `<w:p` element at or before the given byte offset.
fn find_paragraph_start(
    xml: &str,
    byte_offset: usize,
) -> Result<usize, DocliError> {
    // The byte_offset should point at or into a <w:p element.
    // Search backwards from the offset for "<w:p".
    let search_region = &xml[..byte_offset.min(xml.len())];
    search_region
        .rfind("<w:p")
        .or_else(|| xml[byte_offset..].find("<w:p").map(|i| i + byte_offset))
        .ok_or_else(|| DocliError::InvalidTarget {
            message: format!(
                "no <w:p> found at byte offset {byte_offset}"
            ),
        })
}

/// Find the end of the `</w:p>` element starting from para_start.
fn find_paragraph_end(
    xml: &str,
    para_start: usize,
) -> Result<usize, DocliError> {
    let rest = &xml[para_start..];

    // Handle self-closing <w:p/>
    if let Some(close) = rest.find("/>") {
        let open_end = rest.find('>').unwrap_or(close + 1);
        if close < open_end {
            return Ok(para_start + close + 2);
        }
    }

    rest.find("</w:p>")
        .map(|i| para_start + i + "</w:p>".len())
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!(
                "no closing </w:p> found from offset {para_start}"
            ),
        })
}

/// Extract the `<w:pPr>...</w:pPr>` block from paragraph XML, if present.
fn extract_paragraph_properties(para_xml: &str) -> Option<String> {
    let start = para_xml.find("<w:pPr")?;
    let end = para_xml.find("</w:pPr>")?;
    Some(para_xml[start..end + "</w:pPr>".len()].to_string())
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

    fn doc_xml(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
        )
    }

    #[test]
    fn basic_replacement() {
        let body =
            r#"<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:r><w:t>Hello World</w:t></w:r></w:p>"#;
        let xml = doc_xml(body);
        let para_offset = xml.find("<w:p ").unwrap();

        let mut graph = make_graph("word/document.xml", &xml);
        replace_text_in_part(
            &mut graph,
            "word/document.xml",
            para_offset,
            6,
            11,
            "Earth",
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("Hello"));
        assert!(result.contains("Earth"));
        assert!(!result.contains("World"));
    }

    #[test]
    fn preserve_formatting_on_replace() {
        let body = r#"<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:r><w:rPr><w:b/></w:rPr><w:t>Bold text</w:t></w:r></w:p>"#;
        let xml = doc_xml(body);
        let para_offset = xml.find("<w:p ").unwrap();

        let mut graph = make_graph("word/document.xml", &xml);
        replace_text_in_part(
            &mut graph,
            "word/document.xml",
            para_offset,
            5,
            9,
            "word",
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("w:b"));
        assert!(result.contains("word"));
    }

    #[test]
    fn replace_entire_text() {
        let body = r#"<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:r><w:t>Old</w:t></w:r></w:p>"#;
        let xml = doc_xml(body);
        let para_offset = xml.find("<w:p ").unwrap();

        let mut graph = make_graph("word/document.xml", &xml);
        replace_text_in_part(
            &mut graph,
            "word/document.xml",
            para_offset,
            0,
            3,
            "New",
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("New"));
        assert!(!result.contains("Old"));
    }
}
