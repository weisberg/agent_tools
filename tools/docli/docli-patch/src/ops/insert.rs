use docli_core::{ContentBlock, DocliError, ParagraphContent, Position};

use crate::part_graph::PartGraph;

/// Serialize a [`ContentBlock`] to OOXML paragraph XML.
pub fn content_block_to_xml(block: &ContentBlock) -> Result<String, DocliError> {
    match block {
        ContentBlock::Paragraph { paragraph } => paragraph_to_xml(paragraph),
        ContentBlock::Heading1 { heading1 } => Ok(heading_xml("Heading1", heading1)),
        ContentBlock::Heading2 { heading2 } => Ok(heading_xml("Heading2", heading2)),
        ContentBlock::Heading3 { heading3 } => Ok(heading_xml("Heading3", heading3)),
        ContentBlock::Bullets { bullets } => Ok(bullets_xml(bullets)),
        ContentBlock::Numbers { numbers } => Ok(numbers_xml(numbers)),
        ContentBlock::PageBreak { .. } => Ok(page_break_xml()),
        ContentBlock::Table { table } => Ok(table_block_xml(table)),
        _ => Err(DocliError::InvalidOperation {
            message: format!("unsupported ContentBlock variant for XML serialization"),
        }),
    }
}

/// Insert content before or after a paragraph identified by byte offset.
pub fn insert_content(
    graph: &mut PartGraph,
    part_path: &str,
    paragraph_byte_offset: usize,
    paragraph_byte_end: usize,
    position: &Position,
    blocks: &[ContentBlock],
) -> Result<(), DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    if paragraph_byte_end > xml_str.len() {
        return Err(DocliError::InvalidOperation {
            message: format!(
                "paragraph_byte_end ({paragraph_byte_end}) exceeds part length ({})",
                xml_str.len()
            ),
        });
    }

    let mut new_xml = String::new();
    for block in blocks {
        new_xml.push_str(&content_block_to_xml(block)?);
    }

    let insert_pos = match position {
        Position::Before => paragraph_byte_offset,
        Position::After => paragraph_byte_end,
    };

    let mut result = String::with_capacity(xml_str.len() + new_xml.len());
    result.push_str(&xml_str[..insert_pos]);
    result.push_str(&new_xml);
    result.push_str(&xml_str[insert_pos..]);

    graph.set_xml(part_path, result.into_bytes());
    Ok(())
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

fn paragraph_to_xml(content: &ParagraphContent) -> Result<String, DocliError> {
    match content {
        ParagraphContent::Text(s) => {
            let escaped = escape_xml(s);
            Ok(format!(
                r#"<w:p><w:r><w:t xml:space="preserve">{escaped}</w:t></w:r></w:p>"#
            ))
        }
        ParagraphContent::Block(block) => {
            let mut xml = String::from("<w:p>");
            // Add paragraph properties if present.
            let has_ppr = block.style.is_some() || block.align.is_some();
            if has_ppr {
                xml.push_str("<w:pPr>");
                if let Some(ref style) = block.style {
                    xml.push_str(&format!(
                        r#"<w:pStyle w:val="{style}"/>"#
                    ));
                }
                if let Some(ref align) = block.align {
                    xml.push_str(&format!(r#"<w:jc w:val="{align}"/>"#));
                }
                xml.push_str("</w:pPr>");
            }

            for run in &block.runs {
                match run {
                    docli_core::InlineRun::Text(tr) => {
                        xml.push_str("<w:r>");
                        let has_rpr =
                            tr.bold || tr.italic || tr.underline || tr.font.is_some();
                        if has_rpr {
                            xml.push_str("<w:rPr>");
                            if tr.bold {
                                xml.push_str("<w:b/>");
                            }
                            if tr.italic {
                                xml.push_str("<w:i/>");
                            }
                            if tr.underline {
                                xml.push_str("<w:u w:val=\"single\"/>");
                            }
                            if let Some(ref font) = tr.font {
                                if let Some(ref name) = font.name {
                                    xml.push_str(&format!(
                                        r#"<w:rFonts w:ascii="{name}" w:hAnsi="{name}"/>"#
                                    ));
                                }
                            }
                            xml.push_str("</w:rPr>");
                        }
                        let escaped = escape_xml(&tr.text);
                        let space_attr =
                            if tr.text.starts_with(' ') || tr.text.ends_with(' ') {
                                r#" xml:space="preserve""#
                            } else {
                                ""
                            };
                        xml.push_str(&format!(
                            "<w:t{space_attr}>{escaped}</w:t>"
                        ));
                        xml.push_str("</w:r>");
                    }
                    docli_core::InlineRun::Footnote { .. } => {
                        // Footnotes require complex handling; skip for now.
                    }
                    docli_core::InlineRun::Link { link } => {
                        // Simplified hyperlink rendering.
                        let escaped = escape_xml(&link.text);
                        xml.push_str(&format!(
                            "<w:r><w:rPr><w:rStyle w:val=\"Hyperlink\"/></w:rPr>\
                             <w:t>{escaped}</w:t></w:r>"
                        ));
                    }
                }
            }

            xml.push_str("</w:p>");
            Ok(xml)
        }
    }
}

fn heading_xml(style: &str, text: &str) -> String {
    let escaped = escape_xml(text);
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>\
         <w:r><w:t>{escaped}</w:t></w:r></w:p>"
    )
}

fn bullets_xml(items: &[String]) -> String {
    let mut xml = String::new();
    for item in items {
        let escaped = escape_xml(item);
        xml.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"ListBullet\"/>\
             <w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr>\
             </w:pPr><w:r><w:t>{escaped}</w:t></w:r></w:p>"
        ));
    }
    xml
}

fn numbers_xml(items: &[String]) -> String {
    let mut xml = String::new();
    for item in items {
        let escaped = escape_xml(item);
        xml.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"ListNumber\"/>\
             <w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"2\"/></w:numPr>\
             </w:pPr><w:r><w:t>{escaped}</w:t></w:r></w:p>"
        ));
    }
    xml
}

fn page_break_xml() -> String {
    "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>".to_string()
}

fn table_block_xml(table: &docli_core::TableBlock) -> String {
    let mut xml = String::from("<w:tbl><w:tblPr>");
    if let Some(ref style) = table.style {
        xml.push_str(&format!(
            r#"<w:tblStyle w:val="{style}"/>"#
        ));
    }
    xml.push_str(
        r#"<w:tblW w:w="0" w:type="auto"/></w:tblPr>"#,
    );

    // Headers as first row.
    if !table.headers.is_empty() {
        xml.push_str("<w:tr>");
        for cell in &table.headers {
            let escaped = escape_xml(cell);
            xml.push_str(&format!(
                "<w:tc><w:p><w:r><w:rPr><w:b/></w:rPr>\
                 <w:t>{escaped}</w:t></w:r></w:p></w:tc>"
            ));
        }
        xml.push_str("</w:tr>");
    }

    for row in &table.rows {
        xml.push_str("<w:tr>");
        for cell in row {
            let escaped = escape_xml(cell);
            xml.push_str(&format!(
                "<w:tc><w:p><w:r><w:t>{escaped}</w:t></w:r></w:p></w:tc>"
            ));
        }
        xml.push_str("</w:tr>");
    }

    xml.push_str("</w:tbl>");
    xml
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

    #[test]
    fn content_block_paragraph_text() {
        let block = ContentBlock::Paragraph {
            paragraph: ParagraphContent::Text("Hello World".to_string()),
        };
        let xml = content_block_to_xml(&block).unwrap();
        assert!(xml.contains("Hello World"));
        assert!(xml.contains("<w:p>"));
        assert!(xml.contains("<w:t"));
    }

    #[test]
    fn content_block_heading1() {
        let block = ContentBlock::Heading1 {
            heading1: "Title".to_string(),
        };
        let xml = content_block_to_xml(&block).unwrap();
        assert!(xml.contains("Heading1"));
        assert!(xml.contains("Title"));
    }

    #[test]
    fn content_block_bullets() {
        let block = ContentBlock::Bullets {
            bullets: vec!["Item 1".to_string(), "Item 2".to_string()],
        };
        let xml = content_block_to_xml(&block).unwrap();
        assert!(xml.contains("ListBullet"));
        assert!(xml.contains("Item 1"));
        assert!(xml.contains("Item 2"));
    }

    #[test]
    fn content_block_escapes_special_chars() {
        let block = ContentBlock::Paragraph {
            paragraph: ParagraphContent::Text("A & B < C".to_string()),
        };
        let xml = content_block_to_xml(&block).unwrap();
        assert!(xml.contains("A &amp; B &lt; C"));
    }

    #[test]
    fn insert_before_paragraph() {
        let xml = "<w:body><w:p><w:r><w:t>Existing</w:t></w:r></w:p></w:body>";
        let para_start = xml.find("<w:p>").unwrap();
        let para_end = xml.find("</w:p>").unwrap() + "</w:p>".len();

        let blocks = vec![ContentBlock::Paragraph {
            paragraph: ParagraphContent::Text("New".to_string()),
        }];

        let mut graph = make_graph("word/document.xml", xml);
        insert_content(
            &mut graph,
            "word/document.xml",
            para_start,
            para_end,
            &Position::Before,
            &blocks,
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        let new_pos = result.find("New").unwrap();
        let existing_pos = result.find("Existing").unwrap();
        assert!(new_pos < existing_pos);
    }

    #[test]
    fn insert_after_paragraph() {
        let xml = "<w:body><w:p><w:r><w:t>Existing</w:t></w:r></w:p></w:body>";
        let para_start = xml.find("<w:p>").unwrap();
        let para_end = xml.find("</w:p>").unwrap() + "</w:p>".len();

        let blocks = vec![ContentBlock::Paragraph {
            paragraph: ParagraphContent::Text("After".to_string()),
        }];

        let mut graph = make_graph("word/document.xml", xml);
        insert_content(
            &mut graph,
            "word/document.xml",
            para_start,
            para_end,
            &Position::After,
            &blocks,
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        let existing_pos = result.find("Existing").unwrap();
        let after_pos = result.find("After").unwrap();
        assert!(after_pos > existing_pos);
    }
}
