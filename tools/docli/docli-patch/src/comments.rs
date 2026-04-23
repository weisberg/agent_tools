//! OOXML comment manipulation: adding comments, replies, and resolving.

use docli_core::DocliError;

use crate::part_graph::PartGraph;

/// Escape XML special characters.
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

const COMMENTS_PART: &str = "word/comments.xml";
const COMMENTS_NS: &str =
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Build the XML for a single `<w:comment>` element.
fn comment_element(
    comment_id: u64,
    author: &str,
    date: &str,
    text: &str,
    initials: Option<&str>,
) -> String {
    let escaped_author = escape_xml(author);
    let escaped_text = escape_xml(text);
    let initials_attr = initials.map_or(String::new(), |i| {
        format!(r#" w:initials="{}""#, escape_xml(i))
    });
    format!(
        r#"<w:comment w:id="{comment_id}" w:author="{escaped_author}" w:date="{date}"{initials_attr}><w:p><w:r><w:t>{escaped_text}</w:t></w:r></w:p></w:comment>"#
    )
}

/// Build comment range markers and reference run to insert into document.xml.
fn comment_markers(comment_id: u64) -> (String, String, String) {
    let start = format!(r#"<w:commentRangeStart w:id="{comment_id}"/>"#);
    let end = format!(r#"<w:commentRangeEnd w:id="{comment_id}"/>"#);
    let reference = format!(
        r#"<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="{comment_id}"/></w:r>"#
    );
    (start, end, reference)
}

/// Ensure the comments.xml part exists, creating a skeleton if needed.
fn ensure_comments_part(graph: &mut PartGraph) {
    if graph.xml_bytes(COMMENTS_PART).is_none() {
        let skeleton = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="{COMMENTS_NS}"></w:comments>"#,
        );
        graph.set_xml(COMMENTS_PART, skeleton.into_bytes());
    }
}

/// Add a comment to a document.
///
/// Updates both the document part (range markers + reference) and comments.xml
/// (the comment element itself). The byte offsets identify the paragraph span to
/// which the comment is anchored.
pub fn add_comment(
    graph: &mut PartGraph,
    part_path: &str,
    para_byte_offset: usize,
    para_byte_end: usize,
    comment_id: u64,
    author: &str,
    date: &str,
    text: &str,
) -> Result<(), DocliError> {
    // --- 1. Insert markers into the document part ---
    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    if para_byte_end > xml_str.len() {
        return Err(DocliError::InvalidOperation {
            message: format!(
                "para_byte_end ({para_byte_end}) exceeds part length ({})",
                xml_str.len()
            ),
        });
    }

    let (range_start, range_end, reference) = comment_markers(comment_id);

    // Insert: rangeStart before paragraph, rangeEnd + reference after paragraph.
    let mut result =
        String::with_capacity(xml_str.len() + range_start.len() + range_end.len() + reference.len());
    result.push_str(&xml_str[..para_byte_offset]);
    result.push_str(&range_start);
    result.push_str(&xml_str[para_byte_offset..para_byte_end]);
    result.push_str(&range_end);
    result.push_str(&reference);
    result.push_str(&xml_str[para_byte_end..]);

    graph.set_xml(part_path, result.into_bytes());

    // --- 2. Add comment element to comments.xml ---
    ensure_comments_part(graph);
    let comments_xml = graph.xml_bytes(COMMENTS_PART).unwrap();
    let comments_str =
        std::str::from_utf8(comments_xml).map_err(|e| DocliError::InvalidDocx {
            message: format!("invalid UTF-8 in comments.xml: {e}"),
        })?;

    let element = comment_element(comment_id, author, date, text, None);

    // Insert before closing </w:comments>.
    let close_tag = "</w:comments>";
    let insert_pos = comments_str
        .rfind(close_tag)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: "malformed comments.xml: missing </w:comments>".into(),
        })?;

    let mut new_comments = String::with_capacity(comments_str.len() + element.len());
    new_comments.push_str(&comments_str[..insert_pos]);
    new_comments.push_str(&element);
    new_comments.push_str(&comments_str[insert_pos..]);

    graph.set_xml(COMMENTS_PART, new_comments.into_bytes());
    Ok(())
}

/// Add a reply to an existing comment.
///
/// The reply is added as a new `<w:comment>` in comments.xml. Word links replies
/// by placing them adjacent to the parent and using extended properties, but the
/// minimal viable approach is to add a new comment element referencing the parent
/// via a `w:paraId` pattern. Here we keep it simple: a new comment whose text
/// starts with `@reply:{parent_id}` by convention until extended comments part
/// support is added.
pub fn add_comment_reply(
    graph: &mut PartGraph,
    parent_id: u64,
    comment_id: u64,
    author: &str,
    date: &str,
    text: &str,
) -> Result<(), DocliError> {
    ensure_comments_part(graph);

    let comments_xml = graph.xml_bytes(COMMENTS_PART).unwrap();
    let comments_str =
        std::str::from_utf8(comments_xml).map_err(|e| DocliError::InvalidDocx {
            message: format!("invalid UTF-8 in comments.xml: {e}"),
        })?;

    // Verify parent exists.
    let parent_marker = format!(r#"w:id="{parent_id}""#);
    if !comments_str.contains(&parent_marker) {
        return Err(DocliError::RefNotFound {
            reference: format!("comment {parent_id}"),
        });
    }

    // Build reply element. The reply text is prefixed internally to mark lineage.
    let reply_text = format!("@reply:{parent_id} {text}");
    let element = comment_element(comment_id, author, date, &reply_text, None);

    let close_tag = "</w:comments>";
    let insert_pos = comments_str
        .rfind(close_tag)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: "malformed comments.xml: missing </w:comments>".into(),
        })?;

    let mut new_xml = String::with_capacity(comments_str.len() + element.len());
    new_xml.push_str(&comments_str[..insert_pos]);
    new_xml.push_str(&element);
    new_xml.push_str(&comments_str[insert_pos..]);

    graph.set_xml(COMMENTS_PART, new_xml.into_bytes());
    Ok(())
}

/// Resolve a comment (set `w:done="1"` on the comment element).
///
/// Locates the `<w:comment w:id="N"` element and adds the `w:done="1"` attribute.
pub fn resolve_comment(
    graph: &mut PartGraph,
    comment_id: u64,
) -> Result<(), DocliError> {
    let comments_xml =
        graph
            .xml_bytes(COMMENTS_PART)
            .ok_or_else(|| DocliError::InvalidDocx {
                message: "comments.xml not found".into(),
            })?;

    let comments_str =
        std::str::from_utf8(comments_xml).map_err(|e| DocliError::InvalidDocx {
            message: format!("invalid UTF-8 in comments.xml: {e}"),
        })?;

    let needle = format!(r#"<w:comment w:id="{comment_id}""#);
    let pos = comments_str
        .find(&needle)
        .ok_or_else(|| DocliError::RefNotFound {
            reference: format!("comment {comment_id}"),
        })?;

    // Find the end of the opening tag attributes to insert w:done.
    let after_needle = pos + needle.len();
    let replacement = format!(r#"{needle} w:done="1""#);

    let mut new_xml = String::with_capacity(comments_str.len() + 12);
    new_xml.push_str(&comments_str[..pos]);
    new_xml.push_str(&replacement);
    new_xml.push_str(&comments_str[after_needle..]);

    graph.set_xml(COMMENTS_PART, new_xml.into_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::part_graph::PartData;

    fn make_graph(doc_xml: &str) -> PartGraph {
        let mut parts = HashMap::new();
        parts.insert(
            "word/document.xml".to_string(),
            PartData::Xml(doc_xml.as_bytes().to_vec()),
        );
        PartGraph { parts }
    }

    fn make_graph_with_comments(doc_xml: &str, comments_xml: &str) -> PartGraph {
        let mut parts = HashMap::new();
        parts.insert(
            "word/document.xml".to_string(),
            PartData::Xml(doc_xml.as_bytes().to_vec()),
        );
        parts.insert(
            COMMENTS_PART.to_string(),
            PartData::Xml(comments_xml.as_bytes().to_vec()),
        );
        PartGraph { parts }
    }

    #[test]
    fn add_comment_inserts_markers_and_element() {
        let doc = "<w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body>";
        let para_start = doc.find("<w:p>").unwrap();
        let para_end = doc.find("</w:p>").unwrap() + "</w:p>".len();

        let mut graph = make_graph(doc);
        add_comment(
            &mut graph,
            "word/document.xml",
            para_start,
            para_end,
            100,
            "Alice",
            "2025-01-01T00:00:00Z",
            "Good point",
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap()).unwrap();
        assert!(result.contains(r#"<w:commentRangeStart w:id="100"/>"#));
        assert!(result.contains(r#"<w:commentRangeEnd w:id="100"/>"#));
        assert!(result.contains(r#"<w:commentReference w:id="100"/>"#));

        let comments =
            std::str::from_utf8(graph.xml_bytes(COMMENTS_PART).unwrap()).unwrap();
        assert!(comments.contains(r#"w:id="100""#));
        assert!(comments.contains("Good point"));
    }

    #[test]
    fn add_comment_creates_comments_part_if_missing() {
        let doc = "<w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body>";
        let mut graph = make_graph(doc);
        let para_start = doc.find("<w:p>").unwrap();
        let para_end = doc.find("</w:p>").unwrap() + "</w:p>".len();

        add_comment(
            &mut graph,
            "word/document.xml",
            para_start,
            para_end,
            1,
            "Bob",
            "2025-01-01T00:00:00Z",
            "Note",
        )
        .unwrap();

        assert!(graph.xml_bytes(COMMENTS_PART).is_some());
    }

    #[test]
    fn add_comment_error_on_bad_range() {
        let doc = "<w:body><w:p/></w:body>";
        let mut graph = make_graph(doc);
        let result = add_comment(
            &mut graph,
            "word/document.xml",
            0,
            9999,
            1,
            "A",
            "2025-01-01T00:00:00Z",
            "x",
        );
        assert!(result.is_err());
    }

    #[test]
    fn add_reply_links_to_parent() {
        let doc = "<w:body><w:p/></w:body>";
        let comments = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="1" w:author="Alice" w:date="2025-01-01T00:00:00Z"><w:p><w:r><w:t>Original</w:t></w:r></w:p></w:comment></w:comments>"#;
        let mut graph = make_graph_with_comments(doc, comments);

        add_comment_reply(
            &mut graph,
            1,
            2,
            "Bob",
            "2025-01-02T00:00:00Z",
            "I agree",
        )
        .unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes(COMMENTS_PART).unwrap()).unwrap();
        assert!(result.contains(r#"w:id="2""#));
        assert!(result.contains("@reply:1 I agree"));
    }

    #[test]
    fn add_reply_error_on_missing_parent() {
        let doc = "<w:body><w:p/></w:body>";
        let comments = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:comments>"#;
        let mut graph = make_graph_with_comments(doc, comments);

        let result = add_comment_reply(
            &mut graph,
            999,
            2,
            "Bob",
            "2025-01-02T00:00:00Z",
            "reply",
        );
        assert!(result.is_err());
    }

    #[test]
    fn resolve_comment_adds_done_attribute() {
        let doc = "<w:body><w:p/></w:body>";
        let comments = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="5" w:author="Carol" w:date="2025-01-01T00:00:00Z"><w:p><w:r><w:t>Fix this</w:t></w:r></w:p></w:comment></w:comments>"#;
        let mut graph = make_graph_with_comments(doc, comments);

        resolve_comment(&mut graph, 5).unwrap();

        let result =
            std::str::from_utf8(graph.xml_bytes(COMMENTS_PART).unwrap()).unwrap();
        assert!(result.contains(r#"w:done="1""#));
    }

    #[test]
    fn resolve_comment_error_on_missing() {
        let doc = "<w:body><w:p/></w:body>";
        let comments = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:comments>"#;
        let mut graph = make_graph_with_comments(doc, comments);

        let result = resolve_comment(&mut graph, 999);
        assert!(result.is_err());
    }
}
