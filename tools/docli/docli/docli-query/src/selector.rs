use docli_core::{DocliError, Story, Target};
use regex::Regex;

use crate::{
    heading::resolve_heading_path,
    index::{DocumentIndex, ParagraphEntry},
    story::StoryPartMap,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub paragraph_indices: Vec<usize>,
    pub part_path: String,
    pub byte_spans: Vec<(usize, usize)>,
}

/// Resolve a `Target` selector against a `DocumentIndex`.
///
/// `story_map` is optional. When provided it is used to validate that the
/// target's `story` field corresponds to the same XML part as `index`. When
/// `None` the check is skipped for non-Body stories (Body is always checked).
pub fn resolve(
    target: &Target,
    index: &DocumentIndex,
    story_map: Option<&StoryPartMap>,
) -> Result<ResolvedTarget, DocliError> {
    ensure_story_matches(target, index, story_map)?;

    match target {
        Target::Paragraph { paragraph, .. } => resolve_paragraphs([*paragraph], index),
        Target::Paragraphs { paragraphs, .. } => {
            resolve_paragraphs(paragraphs.iter().copied(), index)
        }
        Target::Range { range, .. } => {
            let (start, end) = parse_range(range)?;
            resolve_paragraphs(start..=end, index)
        }
        Target::Heading {
            heading, offset, ..
        } => {
            let anchor = index
                .headings
                .iter()
                .find(|candidate| candidate.text.contains(heading))
                .map(|entry| entry.paragraph_index)
                .ok_or_else(|| DocliError::InvalidTarget {
                    message: format!("heading not found: {heading}"),
                })?;
            resolve_paragraphs([anchor + offset], index)
        }
        Target::HeadingPath {
            heading_path,
            offset,
        } => {
            let anchor = resolve_heading_path(&index.headings, heading_path).ok_or_else(|| {
                DocliError::InvalidTarget {
                    message: format!("heading path not found: {heading_path}"),
                }
            })?;
            resolve_paragraphs([anchor + offset], index)
        }
        Target::Table { table } => {
            let table = index
                .tables
                .get(*table)
                .ok_or_else(|| DocliError::InvalidTarget {
                    message: format!("table index out of range: {table}"),
                })?;
            Ok(ResolvedTarget {
                paragraph_indices: table.paragraph_index.into_iter().collect(),
                part_path: index.part_path.clone(),
                byte_spans: vec![(table.byte_offset, table.byte_end)],
            })
        }
        Target::Image { image } => {
            let image = index
                .images
                .get(*image)
                .ok_or_else(|| DocliError::InvalidTarget {
                    message: format!("image index out of range: {image}"),
                })?;
            Ok(ResolvedTarget {
                paragraph_indices: vec![image.paragraph_index],
                part_path: index.part_path.clone(),
                byte_spans: vec![(image.byte_offset, image.byte_end)],
            })
        }
        Target::Style { style, .. } => {
            let matches = index
                .paragraphs
                .iter()
                .filter(|paragraph| paragraph.style.as_deref() == Some(style.as_str()))
                .map(|paragraph| paragraph.index)
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(DocliError::InvalidTarget {
                    message: format!("no paragraphs found with style: {style}"),
                });
            }
            resolve_paragraphs(matches, index)
        }
        Target::Text {
            text,
            regex,
            occurrence,
            ..
        } => resolve_text_match(index, text, *regex, occurrence.unwrap_or(1)),
        Target::Bookmark { bookmark } => {
            let paragraph = index.bookmarks.get(bookmark).copied().ok_or_else(|| {
                DocliError::InvalidTarget {
                    message: format!("bookmark not found: {bookmark}"),
                }
            })?;
            resolve_paragraphs([paragraph], index)
        }
        Target::NodeId { node_id } => {
            let paragraph = index
                .paragraphs
                .iter()
                .find(|paragraph| paragraph.para_id.as_deref() == Some(node_id.as_str()))
                .map(|paragraph| paragraph.index)
                .ok_or_else(|| DocliError::InvalidTarget {
                    message: format!("node id not found: {node_id}"),
                })?;
            resolve_paragraphs([paragraph], index)
        }
        Target::Contains {
            contains,
            occurrence,
            ..
        } => resolve_text_match(index, contains, false, *occurrence),
    }
}

fn ensure_story_matches(
    target: &Target,
    index: &DocumentIndex,
    story_map: Option<&StoryPartMap>,
) -> Result<(), DocliError> {
    let story = match target {
        Target::Paragraph { story, .. }
        | Target::Paragraphs { story, .. }
        | Target::Range { story, .. }
        | Target::Heading { story, .. }
        | Target::Style { story, .. }
        | Target::Text { story, .. }
        | Target::Contains { story, .. } => Some(story),
        _ => None,
    };

    if let Some(story) = story {
        // Determine the expected part path:
        // - Body is always "word/document.xml"
        // - Other stories use the map if available (actual filenames are
        //   assigned by Word and are not guaranteed to follow header1/2/3.xml)
        // - Without a map, non-Body story checks are skipped
        let expected = match story {
            Story::Body => Some("word/document.xml".to_string()),
            other => story_map
                .and_then(|m| m.path_for(other))
                .map(str::to_string),
        };

        if let Some(expected) = expected {
            if expected != index.part_path {
                return Err(DocliError::InvalidTarget {
                    message: format!(
                        "target story {:?} does not map to {}",
                        story, index.part_path
                    ),
                });
            }
        }
    }

    Ok(())
}

fn parse_range(range: &str) -> Result<(usize, usize), DocliError> {
    let mut parts = range.split(':');
    let start = parts
        .next()
        .ok_or_else(|| DocliError::InvalidTarget {
            message: format!("invalid range selector: {range}"),
        })?
        .parse::<usize>()
        .map_err(|_| DocliError::InvalidTarget {
            message: format!("invalid range selector: {range}"),
        })?;
    let end = parts
        .next()
        .ok_or_else(|| DocliError::InvalidTarget {
            message: format!("invalid range selector: {range}"),
        })?
        .parse::<usize>()
        .map_err(|_| DocliError::InvalidTarget {
            message: format!("invalid range selector: {range}"),
        })?;

    if start > end || parts.next().is_some() {
        return Err(DocliError::InvalidTarget {
            message: format!("invalid range selector: {range}"),
        });
    }

    Ok((start, end))
}

fn resolve_paragraphs<I>(paragraphs: I, index: &DocumentIndex) -> Result<ResolvedTarget, DocliError>
where
    I: IntoIterator<Item = usize>,
{
    let mut paragraph_indices = Vec::new();
    let mut byte_spans = Vec::new();

    for paragraph_index in paragraphs {
        let paragraph = paragraph_entry(index, paragraph_index)?;
        paragraph_indices.push(paragraph.index);
        byte_spans.push((paragraph.byte_offset, paragraph.byte_end));
    }

    Ok(ResolvedTarget {
        paragraph_indices,
        part_path: index.part_path.clone(),
        byte_spans,
    })
}

fn resolve_text_match(
    index: &DocumentIndex,
    pattern: &str,
    is_regex: bool,
    occurrence: usize,
) -> Result<ResolvedTarget, DocliError> {
    let matches = if is_regex {
        let regex = Regex::new(pattern).map_err(|error| DocliError::InvalidTarget {
            message: error.to_string(),
        })?;
        index
            .paragraphs
            .iter()
            .filter(|paragraph| regex.is_match(&paragraph.text))
            .map(|paragraph| paragraph.index)
            .collect::<Vec<_>>()
    } else {
        index
            .paragraphs
            .iter()
            .filter(|paragraph| paragraph.text.contains(pattern))
            .map(|paragraph| paragraph.index)
            .collect::<Vec<_>>()
    };

    let paragraph_index = matches
        .get(occurrence.saturating_sub(1))
        .copied()
        .ok_or_else(|| DocliError::InvalidTarget {
            message: format!("text match not found: {pattern}"),
        })?;
    resolve_paragraphs([paragraph_index], index)
}

fn paragraph_entry(
    index: &DocumentIndex,
    paragraph_index: usize,
) -> Result<&ParagraphEntry, DocliError> {
    index
        .paragraphs
        .get(paragraph_index)
        .ok_or_else(|| DocliError::InvalidTarget {
            message: format!("paragraph index out of range: {paragraph_index}"),
        })
}

#[cfg(test)]
mod tests {
    use docli_core::{Story, Target};

    use crate::{
        index::DocumentIndex,
        selector::{resolve, ResolvedTarget},
    };

    const DOC_XML: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <w:body>
            <w:p w14:paraId="AAA111"><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Results</w:t></w:r></w:p>
            <w:p w14:paraId="BBB222"><w:r><w:t xml:space="preserve">  spaced text</w:t></w:r><w:bookmarkStart w:name="bookmark-1"/></w:p>
            <w:p w14:paraId="CCC333"><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>CUPED</w:t></w:r></w:p>
            <w:p w14:paraId="DDD444"><w:pPr><w:pStyle w:val="Caption"/></w:pPr><w:r><w:t>Revenue after 30 days</w:t></w:r></w:p>
            <w:tbl><w:tr><w:tc/><w:tc/></w:tr><w:tr><w:tc/></w:tr></w:tbl>
            <w:p w14:paraId="EEE555"><w:r><w:drawing><a:graphic><a:graphicData><a:pic><a:blipFill><a:blip r:embed="rIdImage1"/></a:blipFill></a:pic></a:graphicData></a:graphic></w:drawing></w:r></w:p>
            <w:p w14:paraId="FFF666"><w:r><w:t>Revenue after 60 days</w:t></w:r><w:commentRangeStart/></w:p>
            <w:p w14:paraId="GGG777"><w:ins w:author="Claude"><w:r><w:t>Inserted</w:t></w:r></w:ins><w:del w:author="Jane"><w:r><w:t>Deleted</w:t></w:r></w:del></w:p>
          </w:body>
        </w:document>"#;

    const RELS_XML: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
        <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
          <Relationship Id="rIdImage1" Target="media/chart.png" />
        </Relationships>"#;

    fn index() -> DocumentIndex {
        DocumentIndex::build_with_relationships(DOC_XML, Some(RELS_XML)).unwrap()
    }

    fn assert_single_paragraph(target: Target, expected: usize) {
        let resolved = resolve(&target, &index(), None).unwrap();
        assert_eq!(
            resolved,
            ResolvedTarget {
                paragraph_indices: vec![expected],
                part_path: "word/document.xml".to_string(),
                byte_spans: vec![resolved.byte_spans[0]],
            }
        );
    }

    #[test]
    fn indexes_paragraphs_headings_images_and_summaries() {
        let index = index();
        assert_eq!(index.paragraphs.len(), 7);
        assert_eq!(index.headings.len(), 2);
        assert_eq!(index.images.len(), 1);
        assert_eq!(index.tables.len(), 1);
        assert_eq!(index.bookmarks.get("bookmark-1"), Some(&1));
        assert_eq!(index.comments.count, 1);
        assert_eq!(index.tracked_changes.insertions, 1);
        assert_eq!(index.tracked_changes.deletions, 1);
        assert!(index.paragraphs[1].text.starts_with("  "));
    }

    #[test]
    fn resolves_paragraph_selector() {
        assert_single_paragraph(
            Target::Paragraph {
                paragraph: 1,
                story: Story::Body,
            },
            1,
        );
    }

    #[test]
    fn resolves_paragraphs_selector() {
        let resolved = resolve(
            &Target::Paragraphs {
                paragraphs: vec![1, 2],
                story: Story::Body,
            },
            &index(),
            None,
        )
        .unwrap();
        assert_eq!(resolved.paragraph_indices, vec![1, 2]);
        assert_eq!(resolved.byte_spans.len(), 2);
    }

    #[test]
    fn resolves_range_selector() {
        let resolved = resolve(
            &Target::Range {
                range: "1:3".to_string(),
                story: Story::Body,
            },
            &index(),
            None,
        )
        .unwrap();
        assert_eq!(resolved.paragraph_indices, vec![1, 2, 3]);
    }

    #[test]
    fn resolves_heading_with_offset_selector() {
        assert_single_paragraph(
            Target::Heading {
                heading: "Results".to_string(),
                offset: 1,
                story: Story::Body,
            },
            1,
        );
    }

    #[test]
    fn resolves_heading_path_selector() {
        assert_single_paragraph(
            Target::HeadingPath {
                heading_path: "Results/CUPED".to_string(),
                offset: 1,
            },
            3,
        );
    }

    #[test]
    fn resolves_table_selector() {
        let resolved = resolve(&Target::Table { table: 0 }, &index(), None).unwrap();
        assert_eq!(resolved.paragraph_indices, vec![3]);
        assert_eq!(resolved.byte_spans.len(), 1);
    }

    #[test]
    fn resolves_image_selector() {
        let resolved = resolve(&Target::Image { image: 0 }, &index(), None).unwrap();
        assert_eq!(resolved.paragraph_indices, vec![4]);
    }

    #[test]
    fn resolves_style_selector() {
        let resolved = resolve(
            &Target::Style {
                style: "Caption".to_string(),
                story: Story::Body,
            },
            &index(),
            None,
        )
        .unwrap();
        assert_eq!(resolved.paragraph_indices, vec![3]);
    }

    #[test]
    fn resolves_text_selector_with_occurrence() {
        let resolved = resolve(
            &Target::Text {
                text: "Revenue".to_string(),
                regex: false,
                occurrence: Some(2),
                story: Story::Body,
            },
            &index(),
            None,
        )
        .unwrap();
        assert_eq!(resolved.paragraph_indices, vec![5]);
    }

    #[test]
    fn resolves_regex_text_selector() {
        let resolved = resolve(
            &Target::Text {
                text: r"\d+ days".to_string(),
                regex: true,
                occurrence: None,
                story: Story::Body,
            },
            &index(),
            None,
        )
        .unwrap();
        assert_eq!(resolved.paragraph_indices, vec![3]);
    }

    #[test]
    fn resolves_bookmark_selector() {
        assert_single_paragraph(
            Target::Bookmark {
                bookmark: "bookmark-1".to_string(),
            },
            1,
        );
    }

    #[test]
    fn resolves_node_id_selector() {
        assert_single_paragraph(
            Target::NodeId {
                node_id: "DDD444".to_string(),
            },
            3,
        );
    }

    #[test]
    fn resolves_contains_selector() {
        let resolved = resolve(
            &Target::Contains {
                contains: "Revenue".to_string(),
                occurrence: 1,
                story: Story::Body,
            },
            &index(),
            None,
        )
        .unwrap();
        assert_eq!(resolved.paragraph_indices, vec![3]);
    }
}
