use docli_core::DocliError;

use crate::part_graph::PartGraph;

/// Delete the XML content at the given byte span from the part.
pub fn delete_content(
    graph: &mut PartGraph,
    part_path: &str,
    byte_offset: usize,
    byte_end: usize,
) -> Result<(), DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    if byte_offset > byte_end {
        return Err(DocliError::InvalidOperation {
            message: format!(
                "byte_offset ({byte_offset}) > byte_end ({byte_end})"
            ),
        });
    }
    if byte_end > xml.len() {
        return Err(DocliError::InvalidOperation {
            message: format!(
                "byte_end ({byte_end}) exceeds part length ({})",
                xml.len()
            ),
        });
    }

    let mut result = Vec::with_capacity(xml.len() - (byte_end - byte_offset));
    result.extend_from_slice(&xml[..byte_offset]);
    result.extend_from_slice(&xml[byte_end..]);

    graph.set_xml(part_path, result);
    Ok(())
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
    fn delete_paragraph_from_body() {
        let xml = r#"<w:body><w:p><w:r><w:t>Keep</w:t></w:r></w:p><w:p><w:r><w:t>Delete</w:t></w:r></w:p></w:body>"#;
        let para_start = xml.find("<w:p><w:r><w:t>Delete").unwrap();
        let para_end = para_start
            + "<w:p><w:r><w:t>Delete</w:t></w:r></w:p>".len();

        let mut graph = make_graph("word/document.xml", xml);
        delete_content(&mut graph, "word/document.xml", para_start, para_end)
            .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("Keep"));
        assert!(!result.contains("Delete"));
    }

    #[test]
    fn delete_entire_content() {
        let xml = "<w:body><w:p><w:r><w:t>Only</w:t></w:r></w:p></w:body>";
        let start = xml.find("<w:p>").unwrap();
        let end = xml.find("</w:p>").unwrap() + "</w:p>".len();

        let mut graph = make_graph("word/document.xml", xml);
        delete_content(&mut graph, "word/document.xml", start, end).unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert_eq!(result, "<w:body></w:body>");
    }

    #[test]
    fn delete_invalid_range_returns_error() {
        let xml = "<w:body><w:p/></w:body>";
        let mut graph = make_graph("word/document.xml", xml);
        let result = delete_content(&mut graph, "word/document.xml", 10, 5);
        assert!(result.is_err());
    }

    #[test]
    fn delete_beyond_length_returns_error() {
        let xml = "<w:body/>";
        let mut graph = make_graph("word/document.xml", xml);
        let result = delete_content(&mut graph, "word/document.xml", 0, 999);
        assert!(result.is_err());
    }
}
