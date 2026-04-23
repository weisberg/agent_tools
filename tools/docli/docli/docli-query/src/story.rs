use std::collections::HashMap;

use docli_core::{DocliError, Package, Story};

#[derive(Clone, Debug, Default)]
pub struct StoryPartMap {
    parts: HashMap<Story, String>,
}

impl StoryPartMap {
    pub fn from_package(package: &Package) -> Result<Self, DocliError> {
        let document_xml =
            package
                .xml_parts
                .get("word/document.xml")
                .ok_or_else(|| DocliError::InvalidDocx {
                    message: "missing word/document.xml".to_string(),
                })?;
        let rels_xml = package
            .xml_parts
            .get("word/_rels/document.xml.rels")
            .map(Vec::as_slice);
        let mut map = Self::from_bytes(document_xml, rels_xml)?;
        // Footnotes, endnotes, and comments are optional parts. Remove them from
        // the map when the corresponding XML part is absent from the package so
        // that callers can rely on path_for() returning None for missing parts.
        for (story, path) in [
            (Story::Footnotes, "word/footnotes.xml"),
            (Story::Endnotes, "word/endnotes.xml"),
            (Story::Comments, "word/comments.xml"),
        ] {
            if !package.xml_parts.contains_key(path) {
                map.parts.remove(&story);
            }
        }
        Ok(map)
    }

    pub fn from_bytes(document_xml: &[u8], rels_xml: Option<&[u8]>) -> Result<Self, DocliError> {
        let document =
            std::str::from_utf8(document_xml).map_err(|error| DocliError::InvalidDocx {
                message: error.to_string(),
            })?;
        let rels = parse_relationships(rels_xml)?;
        let document = roxmltree::Document::parse(document)?;
        let mut parts = HashMap::from([(Story::Body, "word/document.xml".to_string())]);

        for node in document.descendants().filter(|node| node.is_element()) {
            match node.tag_name().name() {
                "headerReference" => {
                    if let Some(story) =
                        story_from_reference_type(true, attr(&node, "type").as_deref())
                    {
                        if let Some(target) =
                            attr(&node, "id").as_deref().and_then(|id| rels.get(id))
                        {
                            parts.insert(story, normalize_word_target(target));
                        }
                    }
                }
                "footerReference" => {
                    if let Some(story) =
                        story_from_reference_type(false, attr(&node, "type").as_deref())
                    {
                        if let Some(target) =
                            attr(&node, "id").as_deref().and_then(|id| rels.get(id))
                        {
                            parts.insert(story, normalize_word_target(target));
                        }
                    }
                }
                _ => {}
            }
        }

        for (story, path) in [
            (Story::Footnotes, "word/footnotes.xml"),
            (Story::Endnotes, "word/endnotes.xml"),
            (Story::Comments, "word/comments.xml"),
        ] {
            parts.insert(story, path.to_string());
        }

        Ok(Self { parts })
    }

    pub fn path_for(&self, story: &Story) -> Option<&str> {
        self.parts.get(story).map(String::as_str)
    }
}

fn parse_relationships(rels_xml: Option<&[u8]>) -> Result<HashMap<String, String>, DocliError> {
    let Some(rels_xml) = rels_xml else {
        return Ok(HashMap::new());
    };
    let rels = std::str::from_utf8(rels_xml).map_err(|error| DocliError::InvalidDocx {
        message: error.to_string(),
    })?;
    let rels = roxmltree::Document::parse(rels)?;
    let mut map = HashMap::new();
    for node in rels
        .descendants()
        .filter(|node| node.has_tag_name("Relationship"))
    {
        if let (Some(id), Some(target)) = (attr(&node, "Id"), attr(&node, "Target")) {
            map.insert(id.to_string(), target.to_string());
        }
    }
    Ok(map)
}

fn story_from_reference_type(is_header: bool, reference_type: Option<&str>) -> Option<Story> {
    match (is_header, reference_type.unwrap_or("default")) {
        (true, "default") => Some(Story::HeaderDefault),
        (true, "first") => Some(Story::HeaderFirst),
        (true, "even") => Some(Story::HeaderEven),
        (false, "default") => Some(Story::FooterDefault),
        (false, "first") => Some(Story::FooterFirst),
        (false, "even") => Some(Story::FooterEven),
        _ => None,
    }
}

fn normalize_word_target(target: &str) -> String {
    if target.starts_with("word/") {
        target.to_string()
    } else {
        format!("word/{target}")
    }
}

fn attr<'a, 'input>(node: &'a roxmltree::Node<'a, 'input>, name: &str) -> Option<String> {
    node.attributes()
        .find(|attribute| attribute.name() == name)
        .map(|attribute| attribute.value().to_string())
}

#[cfg(test)]
mod tests {
    use docli_core::Story;

    use super::StoryPartMap;

    #[test]
    fn maps_header_and_footer_references_from_document_relationships() {
        let document = br#"<?xml version="1.0" encoding="UTF-8"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
              <w:body>
                <w:sectPr>
                  <w:headerReference w:type="default" r:id="rId1"/>
                  <w:footerReference w:type="first" r:id="rId2"/>
                </w:sectPr>
              </w:body>
            </w:document>"#;
        let rels = br#"<?xml version="1.0" encoding="UTF-8"?>
            <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
              <Relationship Id="rId1" Target="header1.xml" />
              <Relationship Id="rId2" Target="footer1.xml" />
            </Relationships>"#;

        let map = StoryPartMap::from_bytes(document, Some(rels)).unwrap();

        assert_eq!(map.path_for(&Story::Body), Some("word/document.xml"));
        assert_eq!(
            map.path_for(&Story::HeaderDefault),
            Some("word/header1.xml")
        );
        assert_eq!(map.path_for(&Story::FooterFirst), Some("word/footer1.xml"));
    }
}
