use docli_core::DocliError;

/// A fragment of a run produced by splitting at character boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFragment {
    /// The run properties XML (e.g., `<w:rPr><w:b/></w:rPr>`).
    pub properties: Option<String>,
    /// The text content.
    pub text: String,
}

/// Result of splitting runs at text boundaries.
#[derive(Debug, Clone)]
pub struct SplitResult {
    /// Runs before the target range.
    pub before_runs: Vec<RunFragment>,
    /// The target runs (the text being isolated).
    pub target_runs: Vec<RunFragment>,
    /// Runs after the target range.
    pub after_runs: Vec<RunFragment>,
}

/// Collected run info from parsing.
struct ParsedRun {
    properties: Option<String>,
    text: String,
}

/// Split runs in a paragraph's XML at character boundaries.
///
/// Given raw paragraph XML bytes, a `char_offset` (inclusive start), and `char_end`
/// (exclusive end), split the text runs so that characters `[char_offset..char_end]`
/// are isolated in `target_runs`.
///
/// Must handle:
/// - Single run containing both start and end offsets
/// - Start offset in one run, end in another (cross-run spans)
/// - Runs with `xml:space="preserve"` (leading/trailing whitespace)
/// - Empty runs (no text, just formatting)
/// - Non-text run children like `<w:tab/>`, `<w:br/>` (treated as single characters)
/// - Clones `<w:rPr>` properties to split runs
pub fn split_runs_at_offsets(
    paragraph_xml: &[u8],
    char_offset: usize,
    char_end: usize,
) -> Result<SplitResult, DocliError> {
    if char_end < char_offset {
        return Err(DocliError::InvalidOperation {
            message: format!("char_end ({char_end}) < char_offset ({char_offset})"),
        });
    }

    let xml_str = std::str::from_utf8(paragraph_xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8 in paragraph XML: {e}"),
    })?;

    let doc = roxmltree::Document::parse(xml_str)?;

    let runs = collect_runs(&doc);

    let total_len: usize = runs.iter().map(|r| r.text.chars().count()).sum();

    if char_offset > total_len {
        return Err(DocliError::InvalidOperation {
            message: format!("char_offset ({char_offset}) beyond text length ({total_len})"),
        });
    }
    if char_end > total_len {
        return Err(DocliError::InvalidOperation {
            message: format!("char_end ({char_end}) beyond text length ({total_len})"),
        });
    }

    let mut before_runs = Vec::new();
    let mut target_runs = Vec::new();
    let mut after_runs = Vec::new();

    let mut pos: usize = 0;

    for run in &runs {
        let run_chars: Vec<char> = run.text.chars().collect();
        let run_len = run_chars.len();
        let run_start = pos;
        let run_end = pos + run_len;

        if run_len == 0 {
            // Empty run — classify based on position.
            if run_start < char_offset {
                before_runs.push(RunFragment {
                    properties: run.properties.clone(),
                    text: String::new(),
                });
            } else if run_start >= char_end {
                after_runs.push(RunFragment {
                    properties: run.properties.clone(),
                    text: String::new(),
                });
            } else {
                target_runs.push(RunFragment {
                    properties: run.properties.clone(),
                    text: String::new(),
                });
            }
            continue;
        }

        // Before portion: [run_start .. min(run_end, char_offset))
        let before_end = char_offset.min(run_end);
        if before_end > run_start {
            let slice: String = run_chars[0..(before_end - run_start)].iter().collect();
            before_runs.push(RunFragment {
                properties: run.properties.clone(),
                text: slice,
            });
        }

        // Target portion: [max(run_start, char_offset) .. min(run_end, char_end))
        let target_start = char_offset.max(run_start);
        let target_end_pos = char_end.min(run_end);
        if target_end_pos > target_start {
            let slice: String = run_chars[(target_start - run_start)..(target_end_pos - run_start)]
                .iter()
                .collect();
            target_runs.push(RunFragment {
                properties: run.properties.clone(),
                text: slice,
            });
        }

        // After portion: [max(run_start, char_end) .. run_end)
        let after_start = char_end.max(run_start);
        if run_end > after_start {
            let slice: String = run_chars[(after_start - run_start)..run_len].iter().collect();
            after_runs.push(RunFragment {
                properties: run.properties.clone(),
                text: slice,
            });
        }

        pos = run_end;
    }

    Ok(SplitResult {
        before_runs,
        target_runs,
        after_runs,
    })
}

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn collect_runs(doc: &roxmltree::Document) -> Vec<ParsedRun> {
    let mut runs = Vec::new();

    for node in doc.descendants() {
        if node.is_element()
            && node.tag_name().name() == "r"
            && matches!(node.tag_name().namespace(), Some(ns) if ns == W_NS)
        {
            let properties = extract_rpr(&node);
            let text = extract_run_text(&node);
            runs.push(ParsedRun { properties, text });
        }
    }

    runs
}

fn extract_rpr(run_node: &roxmltree::Node) -> Option<String> {
    for child in run_node.children() {
        if child.is_element()
            && child.tag_name().name() == "rPr"
            && matches!(child.tag_name().namespace(), Some(ns) if ns == W_NS)
        {
            return Some(serialize_element(&child));
        }
    }
    None
}

fn serialize_element(node: &roxmltree::Node) -> String {
    let range = node.range();
    let src = node.document().input_text();
    src[range].to_string()
}

fn extract_run_text(run_node: &roxmltree::Node) -> String {
    let mut text = String::new();

    for child in run_node.children() {
        if !child.is_element() {
            continue;
        }
        let local = child.tag_name().name();
        let ns = child.tag_name().namespace();
        let is_w = matches!(ns, Some(n) if n == W_NS);

        if is_w && local == "t" {
            if let Some(t) = child.text() {
                text.push_str(t);
            }
        } else if is_w && local == "tab" {
            text.push('\t');
        } else if is_w && local == "br" {
            text.push('\n');
        }
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(runs_xml: &str) -> Vec<u8> {
        format!(
            r#"<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{runs_xml}</w:p>"#
        )
        .into_bytes()
    }

    fn run(text: &str) -> String {
        format!(r#"<w:r><w:t xml:space="preserve">{text}</w:t></w:r>"#)
    }

    fn run_with_props(text: &str, props: &str) -> String {
        format!(
            r#"<w:r><w:rPr>{props}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>"#
        )
    }

    fn target_text(result: &SplitResult) -> String {
        result.target_runs.iter().map(|r| r.text.as_str()).collect()
    }

    fn before_text(result: &SplitResult) -> String {
        result.before_runs.iter().map(|r| r.text.as_str()).collect()
    }

    fn after_text(result: &SplitResult) -> String {
        result.after_runs.iter().map(|r| r.text.as_str()).collect()
    }

    // 1. Single run, split in middle
    #[test]
    fn single_run_split_in_middle() {
        let xml = para(&run("Hello World"));
        let r = split_runs_at_offsets(&xml, 5, 6).unwrap();
        assert_eq!(before_text(&r), "Hello");
        assert_eq!(target_text(&r), " ");
        assert_eq!(after_text(&r), "World");
    }

    // 2. Single run, split at start
    #[test]
    fn single_run_split_at_start() {
        let xml = para(&run("Hello"));
        let r = split_runs_at_offsets(&xml, 0, 2).unwrap();
        assert_eq!(before_text(&r), "");
        assert_eq!(target_text(&r), "He");
        assert_eq!(after_text(&r), "llo");
    }

    // 3. Single run, split at end
    #[test]
    fn single_run_split_at_end() {
        let xml = para(&run("Hello"));
        let r = split_runs_at_offsets(&xml, 3, 5).unwrap();
        assert_eq!(before_text(&r), "Hel");
        assert_eq!(target_text(&r), "lo");
        assert_eq!(after_text(&r), "");
    }

    // 4. Two runs, split spans both
    #[test]
    fn two_runs_split_spans_both() {
        let xml = para(&format!("{}{}", run("Hello"), run(" World")));
        let r = split_runs_at_offsets(&xml, 3, 8).unwrap();
        assert_eq!(before_text(&r), "Hel");
        assert_eq!(target_text(&r), "lo Wo");
        assert_eq!(after_text(&r), "rld");
    }

    // 5. Three runs, target is entire middle run
    #[test]
    fn three_runs_target_entire_middle() {
        let xml = para(&format!("{}{}{}", run("AA"), run("BB"), run("CC")));
        let r = split_runs_at_offsets(&xml, 2, 4).unwrap();
        assert_eq!(before_text(&r), "AA");
        assert_eq!(target_text(&r), "BB");
        assert_eq!(after_text(&r), "CC");
    }

    // 6. Empty paragraph
    #[test]
    fn empty_paragraph() {
        let xml = para("");
        let r = split_runs_at_offsets(&xml, 0, 0).unwrap();
        assert!(r.before_runs.is_empty());
        assert!(r.target_runs.is_empty());
        assert!(r.after_runs.is_empty());
    }

    // 7. Run with no text (only formatting)
    #[test]
    fn run_with_no_text() {
        let xml = para(r#"<w:r><w:rPr><w:b/></w:rPr></w:r>"#);
        let r = split_runs_at_offsets(&xml, 0, 0).unwrap();
        let total = r.before_runs.len() + r.target_runs.len() + r.after_runs.len();
        assert_eq!(total, 1);
    }

    // 8. Run with xml:space="preserve" and leading spaces
    #[test]
    fn run_with_preserve_and_leading_spaces() {
        let xml = para(&run("  hello"));
        let r = split_runs_at_offsets(&xml, 0, 2).unwrap();
        assert_eq!(target_text(&r), "  ");
        assert_eq!(after_text(&r), "hello");
    }

    // 9. Run with <w:tab/>
    #[test]
    fn run_with_tab() {
        let xml = para(r#"<w:r><w:t>A</w:t><w:tab/><w:t>B</w:t></w:r>"#);
        let r = split_runs_at_offsets(&xml, 0, 2).unwrap();
        assert_eq!(target_text(&r), "A\t");
        assert_eq!(after_text(&r), "B");
    }

    // 10. Run with <w:br/>
    #[test]
    fn run_with_br() {
        let xml = para(r#"<w:r><w:t>A</w:t><w:br/><w:t>B</w:t></w:r>"#);
        let r = split_runs_at_offsets(&xml, 1, 2).unwrap();
        assert_eq!(target_text(&r), "\n");
    }

    // 11. Multiple <w:t> elements in one run
    #[test]
    fn multiple_t_in_one_run() {
        let xml = para(r#"<w:r><w:t>AB</w:t><w:t>CD</w:t></w:r>"#);
        let r = split_runs_at_offsets(&xml, 1, 3).unwrap();
        assert_eq!(target_text(&r), "BC");
    }

    // 12. Split at exact run boundary
    #[test]
    fn split_at_run_boundary() {
        let xml = para(&format!("{}{}", run("Hello"), run("World")));
        let r = split_runs_at_offsets(&xml, 5, 10).unwrap();
        assert_eq!(before_text(&r), "Hello");
        assert_eq!(target_text(&r), "World");
        assert_eq!(after_text(&r), "");
    }

    // 13. char_offset == 0 (from start)
    #[test]
    fn offset_zero_from_start() {
        let xml = para(&run("ABCDE"));
        let r = split_runs_at_offsets(&xml, 0, 5).unwrap();
        assert_eq!(target_text(&r), "ABCDE");
        assert!(r.before_runs.is_empty());
        assert!(r.after_runs.is_empty());
    }

    // 14. char_end == total_length (to end)
    #[test]
    fn char_end_equals_total_length() {
        let xml = para(&run("ABCDE"));
        let r = split_runs_at_offsets(&xml, 3, 5).unwrap();
        assert_eq!(before_text(&r), "ABC");
        assert_eq!(target_text(&r), "DE");
        assert!(r.after_runs.is_empty());
    }

    // 15. char_offset == char_end (empty target)
    #[test]
    fn empty_target_range() {
        let xml = para(&run("Hello"));
        let r = split_runs_at_offsets(&xml, 3, 3).unwrap();
        assert_eq!(target_text(&r), "");
        assert_eq!(before_text(&r), "Hel");
        assert_eq!(after_text(&r), "lo");
    }

    // 16. Offset beyond text length (error)
    #[test]
    fn offset_beyond_text_length() {
        let xml = para(&run("Hi"));
        let r = split_runs_at_offsets(&xml, 5, 8);
        assert!(r.is_err());
    }

    // 17. Run with bold + italic formatting preserved in split
    #[test]
    fn bold_italic_preserved() {
        let xml = para(&run_with_props("Hello", "<w:b/><w:i/>"));
        let r = split_runs_at_offsets(&xml, 2, 4).unwrap();
        assert!(r.before_runs[0].properties.is_some());
        assert!(r.target_runs[0].properties.is_some());
        assert!(r.after_runs[0].properties.is_some());
        let props = r.target_runs[0].properties.as_ref().unwrap();
        assert!(props.contains("w:b"));
        assert!(props.contains("w:i"));
    }

    // 18. Run with font specification preserved
    #[test]
    fn font_spec_preserved() {
        let xml = para(&run_with_props("Test", r#"<w:rFonts w:ascii="Arial"/>"#));
        let r = split_runs_at_offsets(&xml, 1, 3).unwrap();
        let props = r.target_runs[0].properties.as_ref().unwrap();
        assert!(props.contains("Arial"));
    }

    // 19. Cross-run span: start in first, end in third
    #[test]
    fn cross_run_first_to_third() {
        let xml = para(&format!("{}{}{}", run("AA"), run("BB"), run("CC")));
        let r = split_runs_at_offsets(&xml, 1, 5).unwrap();
        assert_eq!(before_text(&r), "A");
        assert_eq!(target_text(&r), "ABBC");
        assert_eq!(after_text(&r), "C");
    }

    // 20. Unicode text (multi-byte chars)
    #[test]
    fn unicode_multibyte_chars() {
        let xml = para(&run("\u{00e9}l\u{00e8}ve")); // "eleve" with accents
        let r = split_runs_at_offsets(&xml, 1, 3).unwrap();
        assert_eq!(target_text(&r), "l\u{00e8}");
    }

    // 21. char_end < char_offset error
    #[test]
    fn char_end_less_than_offset_error() {
        let xml = para(&run("Hello"));
        let r = split_runs_at_offsets(&xml, 3, 1);
        assert!(r.is_err());
    }

    // 22. Entire text as target
    #[test]
    fn entire_text_as_target() {
        let xml = para(&format!("{}{}", run("AB"), run("CD")));
        let r = split_runs_at_offsets(&xml, 0, 4).unwrap();
        assert_eq!(before_text(&r), "");
        assert_eq!(target_text(&r), "ABCD");
        assert_eq!(after_text(&r), "");
    }

    // 23. Run with mixed tab and text
    #[test]
    fn run_mixed_tab_and_text() {
        let xml =
            para(r#"<w:r><w:t>A</w:t><w:tab/><w:t>B</w:t><w:br/><w:t>C</w:t></w:r>"#);
        let r = split_runs_at_offsets(&xml, 1, 4).unwrap();
        assert_eq!(target_text(&r), "\tB\n");
    }
}
