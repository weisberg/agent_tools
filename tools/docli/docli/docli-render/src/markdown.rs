use docli_query::DocumentIndex;

/// Convert a DocumentIndex to markdown text.
/// Uses heading levels, paragraph styles, and text content.
pub fn index_to_markdown(index: &DocumentIndex) -> String {
    let mut output = String::new();

    for paragraph in &index.paragraphs {
        // Check if this paragraph is a heading
        if let Some(heading) = index
            .headings
            .iter()
            .find(|h| h.paragraph_index == paragraph.index)
        {
            let prefix = "#".repeat(heading.level as usize);
            output.push_str(&format!("{} {}\n\n", prefix, heading.text));
        } else if paragraph.text.is_empty() {
            output.push('\n');
        } else {
            output.push_str(&paragraph.text);
            output.push_str("\n\n");
        }
    }

    output
}

/// Convert a DocumentIndex to plain text.
pub fn index_to_text(index: &DocumentIndex) -> String {
    let mut output = String::new();
    for paragraph in &index.paragraphs {
        if !paragraph.text.is_empty() {
            output.push_str(&paragraph.text);
        }
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use docli_query::{HeadingEntry, ParagraphEntry};

    fn make_index(paragraphs: Vec<ParagraphEntry>, headings: Vec<HeadingEntry>) -> DocumentIndex {
        DocumentIndex {
            part_path: "word/document.xml".to_string(),
            paragraphs,
            headings,
            ..DocumentIndex::default()
        }
    }

    fn para(index: usize, text: &str) -> ParagraphEntry {
        ParagraphEntry {
            index,
            style: None,
            text: text.to_string(),
            para_id: None,
            byte_offset: 0,
            byte_end: 0,
        }
    }

    #[test]
    fn test_index_to_markdown_with_headings() {
        let index = make_index(
            vec![para(0, "Introduction"), para(1, "Some body text.")],
            vec![HeadingEntry {
                paragraph_index: 0,
                level: 1,
                text: "Introduction".to_string(),
            }],
        );
        let md = index_to_markdown(&index);
        assert!(md.contains("# Introduction"));
        assert!(md.contains("Some body text."));
    }

    #[test]
    fn test_index_to_markdown_nested_headings() {
        let index = make_index(
            vec![
                para(0, "Chapter"),
                para(1, "Section"),
                para(2, "Content here."),
            ],
            vec![
                HeadingEntry {
                    paragraph_index: 0,
                    level: 1,
                    text: "Chapter".to_string(),
                },
                HeadingEntry {
                    paragraph_index: 1,
                    level: 2,
                    text: "Section".to_string(),
                },
            ],
        );
        let md = index_to_markdown(&index);
        assert!(md.contains("# Chapter"));
        assert!(md.contains("## Section"));
        assert!(md.contains("Content here."));
    }

    #[test]
    fn test_index_to_text() {
        let index = make_index(
            vec![para(0, "First"), para(1, ""), para(2, "Third")],
            vec![],
        );
        let text = index_to_text(&index);
        assert!(text.contains("First"));
        assert!(text.contains("Third"));
        // Empty paragraph should still produce a newline
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
    }
}
