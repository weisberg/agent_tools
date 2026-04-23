use std::collections::HashMap;

use docli_core::DocliError;

#[derive(Clone, Debug, Default)]
pub struct DocumentIndex {
    pub part_path: String,
    pub paragraphs: Vec<ParagraphEntry>,
    pub tables: Vec<TableEntry>,
    pub images: Vec<ImageEntry>,
    pub headings: Vec<HeadingEntry>,
    pub bookmarks: HashMap<String, usize>,
    pub comments: CommentSummary,
    pub tracked_changes: TrackedChangeSummary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphEntry {
    pub index: usize,
    pub style: Option<String>,
    pub text: String,
    pub para_id: Option<String>,
    pub byte_offset: usize,
    pub byte_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableEntry {
    pub index: usize,
    pub paragraph_index: Option<usize>,
    pub rows: usize,
    pub cols: usize,
    pub byte_offset: usize,
    pub byte_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageEntry {
    pub index: usize,
    pub paragraph_index: usize,
    pub relationship_id: String,
    pub target: Option<String>,
    pub byte_offset: usize,
    pub byte_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadingEntry {
    pub paragraph_index: usize,
    pub level: u8,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommentSummary {
    pub count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackedChangeSummary {
    pub count: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub authors: Vec<String>,
}

impl DocumentIndex {
    pub fn build(xml_bytes: &[u8]) -> Result<Self, DocliError> {
        Self::build_with_relationships(xml_bytes, None)
    }

    pub fn build_with_relationships(
        xml_bytes: &[u8],
        rels_xml: Option<&[u8]>,
    ) -> Result<Self, DocliError> {
        let xml = std::str::from_utf8(xml_bytes).map_err(|error| DocliError::InvalidDocx {
            message: error.to_string(),
        })?;
        let rels = parse_relationships(rels_xml)?;
        let document = roxmltree::Document::parse(xml)?;

        let mut index = DocumentIndex {
            part_path: "word/document.xml".to_string(),
            ..Self::default()
        };
        let mut last_paragraph_index = None;

        for node in document.descendants().filter(|node| node.is_element()) {
            match node.tag_name().name() {
                "p" => {
                    // Skip paragraphs inside table cells — they are not body-level
                    // paragraphs and must not corrupt the paragraph index or
                    // last_paragraph_index used for image/table anchoring.
                    if node.ancestors().any(|ancestor| ancestor.has_tag_name("tbl")) {
                        continue;
                    }
                    let paragraph = build_paragraph_entry(&node, index.paragraphs.len())?;
                    if let Some(heading_level) = heading_level(&node) {
                        index.headings.push(HeadingEntry {
                            paragraph_index: paragraph.index,
                            level: heading_level,
                            text: paragraph.text.clone(),
                        });
                    }
                    for bookmark in node
                        .descendants()
                        .filter(|descendant| descendant.has_tag_name("bookmarkStart"))
                    {
                        if let Some(name) = attr(&bookmark, "name") {
                            index.bookmarks.insert(name, paragraph.index);
                        }
                    }
                    last_paragraph_index = Some(paragraph.index);
                    index.paragraphs.push(paragraph);
                }
                "tbl" => {
                    index.tables.push(TableEntry {
                        index: index.tables.len(),
                        paragraph_index: last_paragraph_index,
                        rows: node
                            .children()
                            .filter(|child| child.has_tag_name("tr"))
                            .count(),
                        cols: node
                            .children()
                            .filter(|child| child.has_tag_name("tr"))
                            .map(|row| {
                                row.children()
                                    .filter(|child| child.has_tag_name("tc"))
                                    .count()
                            })
                            .max()
                            .unwrap_or(0),
                        byte_offset: node.range().start,
                        byte_end: node.range().end,
                    });
                }
                "drawing" => {
                    if let Some(paragraph_index) = last_paragraph_index {
                        if let Some((relationship_id, target)) = image_target(&node, &rels) {
                            index.images.push(ImageEntry {
                                index: index.images.len(),
                                paragraph_index,
                                relationship_id,
                                target,
                                byte_offset: node.range().start,
                                byte_end: node.range().end,
                            });
                        }
                    }
                }
                "commentRangeStart" => {
                    index.comments.count += 1;
                }
                "ins" => {
                    index.tracked_changes.count += 1;
                    index.tracked_changes.insertions += 1;
                    if let Some(author) = attr(&node, "author") {
                        push_author(&mut index.tracked_changes.authors, &author);
                    }
                }
                "del" => {
                    index.tracked_changes.count += 1;
                    index.tracked_changes.deletions += 1;
                    if let Some(author) = attr(&node, "author") {
                        push_author(&mut index.tracked_changes.authors, &author);
                    }
                }
                _ => {}
            }
        }

        Ok(index)
    }
}

fn build_paragraph_entry(
    node: &roxmltree::Node<'_, '_>,
    index: usize,
) -> Result<ParagraphEntry, DocliError> {
    let text = node
        .descendants()
        .filter(|descendant| descendant.has_tag_name("t"))
        .filter_map(|text| text.text())
        .collect::<String>();

    Ok(ParagraphEntry {
        index,
        style: paragraph_style(node),
        text,
        para_id: attr(node, "paraId"),
        byte_offset: node.range().start,
        byte_end: node.range().end,
    })
}

fn paragraph_style(node: &roxmltree::Node<'_, '_>) -> Option<String> {
    node.descendants()
        .find(|descendant| descendant.has_tag_name("pStyle"))
        .and_then(|style| attr(&style, "val"))
}

fn heading_level(node: &roxmltree::Node<'_, '_>) -> Option<u8> {
    let style = paragraph_style(node)?;
    style
        .strip_prefix("Heading")
        .and_then(|suffix| suffix.parse::<u8>().ok())
}

fn image_target(
    node: &roxmltree::Node<'_, '_>,
    relationships: &HashMap<String, String>,
) -> Option<(String, Option<String>)> {
    let blip = node
        .descendants()
        .find(|descendant| descendant.has_tag_name("blip"))?;
    let relationship_id = attr(&blip, "embed")?;
    let target = relationships.get(&relationship_id).cloned();
    Some((relationship_id, target))
}

fn parse_relationships(rels_xml: Option<&[u8]>) -> Result<HashMap<String, String>, DocliError> {
    let Some(rels_xml) = rels_xml else {
        return Ok(HashMap::new());
    };
    let xml = std::str::from_utf8(rels_xml).map_err(|error| DocliError::InvalidDocx {
        message: error.to_string(),
    })?;
    let rels = roxmltree::Document::parse(xml)?;
    let mut map = HashMap::new();
    for relationship in rels
        .descendants()
        .filter(|node| node.has_tag_name("Relationship"))
    {
        if let (Some(id), Some(target)) = (attr(&relationship, "Id"), attr(&relationship, "Target"))
        {
            map.insert(id, target);
        }
    }
    Ok(map)
}

fn push_author(authors: &mut Vec<String>, author: &str) {
    if !authors.iter().any(|existing| existing == author) {
        authors.push(author.to_string());
    }
}

fn attr<'a, 'input>(node: &'a roxmltree::Node<'a, 'input>, name: &str) -> Option<String> {
    node.attributes()
        .find(|attribute| attribute.name() == name)
        .map(|attribute| attribute.value().to_string())
}
