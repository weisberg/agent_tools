//! Build OOXML tracked-change elements (insertions and deletions).

/// Escape XML special characters in attribute values and text content.
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

/// Build a tracked insertion element wrapping content.
///
/// Produces OOXML like:
/// ```xml
/// <w:ins w:id="N" w:author="..." w:date="...">
///   <w:r><w:rPr>...</w:rPr><w:t>text</w:t></w:r>
/// </w:ins>
/// ```
pub fn build_tracked_insertion(
    content: &str,
    run_props: Option<&str>,
    id: u64,
    author: &str,
    date: &str,
) -> String {
    let escaped_content = escape_xml(content);
    let escaped_author = escape_xml(author);
    let rpr = run_props.map_or(String::new(), |p| p.to_string());

    let space_attr = if content.starts_with(' ') || content.ends_with(' ') {
        r#" xml:space="preserve""#
    } else {
        ""
    };

    format!(
        r#"<w:ins w:id="{id}" w:author="{escaped_author}" w:date="{date}"><w:r>{rpr}<w:t{space_attr}>{escaped_content}</w:t></w:r></w:ins>"#
    )
}

/// Build a tracked deletion element wrapping existing runs.
///
/// Produces OOXML like:
/// ```xml
/// <w:del w:id="N" w:author="..." w:date="...">
///   <w:r><w:rPr>...</w:rPr><w:delText>text</w:delText></w:r>
/// </w:del>
/// ```
pub fn build_tracked_deletion(
    deleted_text: &str,
    run_props: Option<&str>,
    id: u64,
    author: &str,
    date: &str,
) -> String {
    let escaped_text = escape_xml(deleted_text);
    let escaped_author = escape_xml(author);
    let rpr = run_props.map_or(String::new(), |p| p.to_string());

    let space_attr = if deleted_text.starts_with(' ') || deleted_text.ends_with(' ') {
        r#" xml:space="preserve""#
    } else {
        ""
    };

    format!(
        r#"<w:del w:id="{id}" w:author="{escaped_author}" w:date="{date}"><w:r>{rpr}<w:delText{space_attr}>{escaped_text}</w:delText></w:r></w:del>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_basic() {
        let xml = build_tracked_insertion("Hello", None, 1, "Alice", "2025-01-01T00:00:00Z");
        assert!(xml.contains(r#"<w:ins w:id="1""#));
        assert!(xml.contains(r#"w:author="Alice""#));
        assert!(xml.contains("<w:t>Hello</w:t>"));
        assert!(xml.contains("</w:ins>"));
    }

    #[test]
    fn insertion_with_run_props() {
        let xml = build_tracked_insertion(
            "Bold text",
            Some("<w:rPr><w:b/></w:rPr>"),
            42,
            "Bob",
            "2025-06-15T12:00:00Z",
        );
        assert!(xml.contains("<w:rPr><w:b/></w:rPr>"));
        assert!(xml.contains(r#"w:id="42""#));
        assert!(xml.contains("<w:t>Bold text</w:t>"));
    }

    #[test]
    fn insertion_escapes_author() {
        let xml = build_tracked_insertion(
            "text",
            None,
            1,
            "O'Brien & Co",
            "2025-01-01T00:00:00Z",
        );
        assert!(xml.contains("O&apos;Brien &amp; Co") || xml.contains("O'Brien &amp; Co"));
    }

    #[test]
    fn insertion_preserves_space() {
        let xml = build_tracked_insertion(" spaced ", None, 1, "A", "2025-01-01T00:00:00Z");
        assert!(xml.contains(r#"xml:space="preserve""#));
    }

    #[test]
    fn deletion_basic() {
        let xml =
            build_tracked_deletion("removed", None, 5, "Carol", "2025-03-01T09:00:00Z");
        assert!(xml.contains(r#"<w:del w:id="5""#));
        assert!(xml.contains(r#"w:author="Carol""#));
        assert!(xml.contains("<w:delText>removed</w:delText>"));
        assert!(xml.contains("</w:del>"));
    }

    #[test]
    fn deletion_with_run_props() {
        let xml = build_tracked_deletion(
            "old",
            Some("<w:rPr><w:i/></w:rPr>"),
            10,
            "Dan",
            "2025-04-01T00:00:00Z",
        );
        assert!(xml.contains("<w:rPr><w:i/></w:rPr>"));
        assert!(xml.contains("<w:delText>old</w:delText>"));
    }

    #[test]
    fn deletion_escapes_text() {
        let xml =
            build_tracked_deletion("a < b & c", None, 1, "E", "2025-01-01T00:00:00Z");
        assert!(xml.contains("a &lt; b &amp; c"));
    }
}
