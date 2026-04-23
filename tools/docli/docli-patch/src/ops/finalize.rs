//! Accept, reject, or strip tracked changes and comments from OOXML documents.

use docli_core::DocliError;
use regex::Regex;

use crate::part_graph::PartGraph;

/// Accept all tracked changes: remove `<w:ins>` wrappers keeping their content,
/// remove `<w:del>` elements entirely (including content).
///
/// Returns the number of changes accepted.
pub fn accept_all(
    graph: &mut PartGraph,
    part_path: &str,
) -> Result<usize, DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    let mut result = xml_str.to_string();
    let mut count = 0;

    // Remove <w:del ...>...</w:del> entirely (accept = discard deleted text).
    let del_re = Regex::new(r"<w:del\b[^>]*>.*?</w:del>").map_err(|e| {
        DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        }
    })?;
    count += del_re.find_iter(&result).count();
    result = del_re.replace_all(&result, "").to_string();

    // Remove <w:ins ...> wrappers but keep inner content (accept = keep inserted text).
    let ins_open_re =
        Regex::new(r"<w:ins\b[^>]*>").map_err(|e| DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        })?;
    count += ins_open_re.find_iter(&result).count();
    result = ins_open_re.replace_all(&result, "").to_string();

    let ins_close_re =
        Regex::new(r"</w:ins>").map_err(|e| DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        })?;
    result = ins_close_re.replace_all(&result, "").to_string();

    graph.set_xml(part_path, result.into_bytes());
    Ok(count)
}

/// Reject all tracked changes: remove `<w:ins>` elements entirely (reject = discard
/// inserted text), remove `<w:del>` wrappers keeping their content (reject = restore
/// deleted text, converting `<w:delText>` back to `<w:t>`).
///
/// Returns the number of changes rejected.
pub fn reject_all(
    graph: &mut PartGraph,
    part_path: &str,
) -> Result<usize, DocliError> {
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    let mut result = xml_str.to_string();
    let mut count = 0;

    // Remove <w:ins ...>...</w:ins> entirely (reject = discard inserted text).
    let ins_re = Regex::new(r"<w:ins\b[^>]*>.*?</w:ins>").map_err(|e| {
        DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        }
    })?;
    count += ins_re.find_iter(&result).count();
    result = ins_re.replace_all(&result, "").to_string();

    // Remove <w:del ...> wrappers but keep content (reject = restore deleted text).
    let del_open_re =
        Regex::new(r"<w:del\b[^>]*>").map_err(|e| DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        })?;
    count += del_open_re.find_iter(&result).count();
    result = del_open_re.replace_all(&result, "").to_string();

    let del_close_re =
        Regex::new(r"</w:del>").map_err(|e| DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        })?;
    result = del_close_re.replace_all(&result, "").to_string();

    // Convert <w:delText> back to <w:t>.
    result = result.replace("<w:delText", "<w:t");
    result = result.replace("</w:delText>", "</w:t>");

    graph.set_xml(part_path, result.into_bytes());
    Ok(count)
}

/// Strip all tracked changes and comments (produce a clean document).
///
/// Removes all `<w:ins>` wrappers (keeping content), all `<w:del>` elements,
/// comment range markers, comment references, and clears comments.xml.
///
/// Returns the total number of items stripped.
pub fn strip_all(
    graph: &mut PartGraph,
    part_path: &str,
) -> Result<usize, DocliError> {
    // First accept all tracked changes.
    let tc_count = accept_all(graph, part_path)?;

    // Then strip comment markers.
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    let mut result = xml_str.to_string();
    let mut comment_count = 0;

    // Remove commentRangeStart / commentRangeEnd.
    let range_re = Regex::new(r"<w:commentRange(Start|End)\b[^/]*/\s*>").map_err(
        |e| DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        },
    )?;
    comment_count += range_re.find_iter(&result).count();
    result = range_re.replace_all(&result, "").to_string();

    // Remove comment reference runs.
    let ref_re =
        Regex::new(r"<w:r>\s*<w:rPr>\s*<w:rStyle[^/]*/>\s*</w:rPr>\s*<w:commentReference[^/]*/>\s*</w:r>")
            .map_err(|e| DocliError::InvalidOperation {
                message: format!("regex error: {e}"),
            })?;
    comment_count += ref_re.find_iter(&result).count();
    result = ref_re.replace_all(&result, "").to_string();

    graph.set_xml(part_path, result.into_bytes());

    // Clear comments.xml if it exists.
    if graph.xml_bytes("word/comments.xml").is_some() {
        let empty = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:comments>"#;
        graph.set_xml("word/comments.xml", empty.as_bytes().to_vec());
    }

    Ok(tc_count + comment_count)
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
    fn accept_all_removes_deletions_keeps_insertions() {
        let xml = r#"<w:body><w:p><w:ins w:id="1" w:author="A" w:date="2025-01-01"><w:r><w:t>new</w:t></w:r></w:ins><w:del w:id="2" w:author="A" w:date="2025-01-01"><w:r><w:delText>old</w:delText></w:r></w:del></w:p></w:body>"#;
        let mut graph = make_graph("word/document.xml", xml);

        let count =
            accept_all(&mut graph, "word/document.xml").unwrap();
        assert!(count >= 2);

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("<w:t>new</w:t>"));
        assert!(!result.contains("old"));
        assert!(!result.contains("<w:ins"));
        assert!(!result.contains("<w:del"));
    }

    #[test]
    fn reject_all_removes_insertions_restores_deletions() {
        let xml = r#"<w:body><w:p><w:ins w:id="1" w:author="A" w:date="2025-01-01"><w:r><w:t>new</w:t></w:r></w:ins><w:del w:id="2" w:author="A" w:date="2025-01-01"><w:r><w:delText>old</w:delText></w:r></w:del></w:p></w:body>"#;
        let mut graph = make_graph("word/document.xml", xml);

        let count =
            reject_all(&mut graph, "word/document.xml").unwrap();
        assert!(count >= 2);

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(!result.contains("new"));
        assert!(result.contains("<w:t>old</w:t>"));
        assert!(!result.contains("<w:ins"));
        assert!(!result.contains("<w:del"));
    }

    #[test]
    fn accept_all_no_changes_returns_zero() {
        let xml = "<w:body><w:p><w:r><w:t>plain</w:t></w:r></w:p></w:body>";
        let mut graph = make_graph("word/document.xml", xml);
        let count = accept_all(&mut graph, "word/document.xml").unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn reject_all_no_changes_returns_zero() {
        let xml = "<w:body><w:p><w:r><w:t>plain</w:t></w:r></w:p></w:body>";
        let mut graph = make_graph("word/document.xml", xml);
        let count = reject_all(&mut graph, "word/document.xml").unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn strip_all_removes_everything() {
        let xml = r#"<w:body><w:p><w:commentRangeStart w:id="10"/><w:ins w:id="1" w:author="A" w:date="2025-01-01"><w:r><w:t>new</w:t></w:r></w:ins><w:commentRangeEnd w:id="10"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="10"/></w:r></w:p></w:body>"#;
        let mut parts = HashMap::new();
        parts.insert(
            "word/document.xml".to_string(),
            PartData::Xml(xml.as_bytes().to_vec()),
        );
        let comments = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="10" w:author="A" w:date="2025-01-01"><w:p><w:r><w:t>Note</w:t></w:r></w:p></w:comment></w:comments>"#;
        parts.insert(
            "word/comments.xml".to_string(),
            PartData::Xml(comments.as_bytes().to_vec()),
        );
        let mut graph = PartGraph { parts };

        let count = strip_all(&mut graph, "word/document.xml").unwrap();
        assert!(count >= 1);

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("<w:t>new</w:t>"));
        assert!(!result.contains("commentRange"));
        assert!(!result.contains("commentReference"));

        let comments_result =
            std::str::from_utf8(graph.xml_bytes("word/comments.xml").unwrap())
                .unwrap();
        assert!(!comments_result.contains(r#"w:id="10""#));
    }

    #[test]
    fn accept_all_missing_part_errors() {
        let mut graph = PartGraph {
            parts: HashMap::new(),
        };
        let result = accept_all(&mut graph, "word/document.xml");
        assert!(result.is_err());
    }
}
