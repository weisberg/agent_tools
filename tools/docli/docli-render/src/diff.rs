use similar::{ChangeTag, TextDiff};
use docli_query::DocumentIndex;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub changes: Vec<DiffChange>,
    pub summary: DiffSummary,
}

#[derive(Debug, Serialize)]
pub struct DiffChange {
    pub tag: String, // "equal", "insert", "delete"
    pub old_index: Option<usize>,
    pub new_index: Option<usize>,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct DiffSummary {
    pub insertions: usize,
    pub deletions: usize,
    pub unchanged: usize,
}

/// Compute a semantic diff between two DocumentIndex instances.
/// Compares paragraph text line by line.
pub fn semantic_diff(old: &DocumentIndex, new: &DocumentIndex) -> DiffResult {
    let old_text: Vec<&str> = old.paragraphs.iter().map(|p| p.text.as_str()).collect();
    let new_text: Vec<&str> = new.paragraphs.iter().map(|p| p.text.as_str()).collect();

    let old_joined = old_text.join("\n");
    let new_joined = new_text.join("\n");

    let diff = TextDiff::from_lines(&old_joined, &new_joined);

    let mut changes = Vec::new();
    let mut insertions = 0;
    let mut deletions = 0;
    let mut unchanged = 0;

    for change in diff.iter_all_changes() {
        let (tag_str, old_idx, new_idx) = match change.tag() {
            ChangeTag::Equal => {
                unchanged += 1;
                ("equal", change.old_index(), change.new_index())
            }
            ChangeTag::Insert => {
                insertions += 1;
                ("insert", None, change.new_index())
            }
            ChangeTag::Delete => {
                deletions += 1;
                ("delete", change.old_index(), None)
            }
        };
        changes.push(DiffChange {
            tag: tag_str.to_string(),
            old_index: old_idx,
            new_index: new_idx,
            value: change.value().to_string(),
        });
    }

    DiffResult {
        changes,
        summary: DiffSummary {
            insertions,
            deletions,
            unchanged,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docli_query::ParagraphEntry;

    fn make_index(texts: &[&str]) -> DocumentIndex {
        DocumentIndex {
            part_path: "word/document.xml".to_string(),
            paragraphs: texts
                .iter()
                .enumerate()
                .map(|(i, t)| ParagraphEntry {
                    index: i,
                    style: None,
                    text: t.to_string(),
                    para_id: None,
                    byte_offset: 0,
                    byte_end: 0,
                })
                .collect(),
            ..DocumentIndex::default()
        }
    }

    #[test]
    fn test_identical_documents() {
        let a = make_index(&["Hello", "World"]);
        let b = make_index(&["Hello", "World"]);
        let result = semantic_diff(&a, &b);
        assert_eq!(result.summary.insertions, 0);
        assert_eq!(result.summary.deletions, 0);
        assert!(result.summary.unchanged > 0);
    }

    #[test]
    fn test_with_insertions() {
        let a = make_index(&["Hello", "World"]);
        let b = make_index(&["Hello", "Beautiful", "World"]);
        let result = semantic_diff(&a, &b);
        assert!(result.summary.insertions > 0);
    }

    #[test]
    fn test_with_deletions() {
        let a = make_index(&["Hello", "Beautiful", "World"]);
        let b = make_index(&["Hello", "World"]);
        let result = semantic_diff(&a, &b);
        assert!(result.summary.deletions > 0);
    }

    #[test]
    fn test_empty_documents() {
        let a = make_index(&[]);
        let b = make_index(&[]);
        let result = semantic_diff(&a, &b);
        assert_eq!(result.summary.insertions, 0);
        assert_eq!(result.summary.deletions, 0);
    }
}
