use docli_core::DocliError;

use crate::part_graph::{PartData, PartGraph};

/// Replace an image's media part and optionally resize.
///
/// This replaces the binary data for the image identified by `relationship_id`,
/// updates the target path in the relationships, and optionally adjusts
/// the image dimensions in the document XML.
pub fn replace_image(
    graph: &mut PartGraph,
    relationship_id: &str,
    new_image_bytes: Vec<u8>,
    new_target_path: &str,
    width_emu: Option<i64>,
    part_path: &str,
    image_byte_offset: usize,
    image_byte_end: usize,
) -> Result<(), DocliError> {
    // 1. Store the new image binary in the part graph.
    graph.parts.insert(
        new_target_path.to_string(),
        PartData::Binary(new_image_bytes),
    );

    // 2. If width_emu is provided, update the drawing XML in the document part.
    if let Some(width) = width_emu {
        let xml = graph
            .xml_bytes(part_path)
            .ok_or_else(|| DocliError::InvalidDocx {
                message: format!("part not found: {part_path}"),
            })?;

        let xml_str =
            std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
                message: format!("invalid UTF-8: {e}"),
            })?;

        if image_byte_end > xml_str.len() {
            return Err(DocliError::InvalidOperation {
                message: format!(
                    "image_byte_end ({image_byte_end}) exceeds part length ({})",
                    xml_str.len()
                ),
            });
        }

        let drawing_xml = &xml_str[image_byte_offset..image_byte_end];

        // Update cx (width) in the extent element: <a:ext cx="..." cy="..."/>
        let updated = update_extent_width(drawing_xml, width)?;

        // Update the relationship embed reference if needed.
        let updated = update_embed_reference(&updated, relationship_id);

        let mut result =
            String::with_capacity(xml_str.len() - drawing_xml.len() + updated.len());
        result.push_str(&xml_str[..image_byte_offset]);
        result.push_str(&updated);
        result.push_str(&xml_str[image_byte_end..]);

        graph.set_xml(part_path, result.into_bytes());
    }

    Ok(())
}

/// Update the `cx` attribute in `<a:ext cx="..." cy="..."/>` elements within
/// the drawing XML.
fn update_extent_width(drawing_xml: &str, width_emu: i64) -> Result<String, DocliError> {
    // Match <a:ext cx="NNN" or <wp:extent cx="NNN".
    let re = regex::Regex::new(r#"(cx=")(\d+)(")"#).map_err(|e| {
        DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        }
    })?;

    let result = re.replace_all(drawing_xml, |caps: &regex::Captures| {
        format!("{}{}{}", &caps[1], width_emu, &caps[3])
    });

    Ok(result.into_owned())
}

/// Update the `r:embed` attribute to match the relationship ID.
fn update_embed_reference(xml: &str, relationship_id: &str) -> String {
    let re = regex::Regex::new(r#"(r:embed=")[^"]*(")"#).expect("valid regex");
    re.replace_all(xml, |caps: &regex::Captures| {
        format!("{}{}{}", &caps[1], relationship_id, &caps[2])
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_graph(path: &str, xml: &str) -> PartGraph {
        let mut parts = HashMap::new();
        parts.insert(path.to_string(), PartData::Xml(xml.as_bytes().to_vec()));
        PartGraph { parts }
    }

    #[test]
    fn replace_image_stores_binary() {
        let xml = r#"<w:body><w:p><w:drawing><a:ext cx="500" cy="300"/><a:blip r:embed="rId1"/></w:drawing></w:p></w:body>"#;
        let drawing_start = xml.find("<w:drawing>").unwrap();
        let drawing_end =
            xml.find("</w:drawing>").unwrap() + "</w:drawing>".len();

        let mut graph = make_graph("word/document.xml", xml);
        replace_image(
            &mut graph,
            "rId5",
            vec![0xFF, 0xD8, 0xFF],
            "word/media/image2.jpg",
            Some(1000),
            "word/document.xml",
            drawing_start,
            drawing_end,
        )
        .unwrap();

        // Check binary was stored.
        assert!(matches!(
            graph.parts.get("word/media/image2.jpg"),
            Some(PartData::Binary(b)) if b == &[0xFF, 0xD8, 0xFF]
        ));

        // Check width was updated.
        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("cx=\"1000\""));
    }

    #[test]
    fn replace_image_without_resize() {
        let xml = "<w:body><w:p/></w:body>";
        let mut graph = make_graph("word/document.xml", xml);
        replace_image(
            &mut graph,
            "rId3",
            vec![0x89, 0x50],
            "word/media/image3.png",
            None,
            "word/document.xml",
            0,
            0,
        )
        .unwrap();

        assert!(matches!(
            graph.parts.get("word/media/image3.png"),
            Some(PartData::Binary(b)) if b == &[0x89, 0x50]
        ));
    }

    #[test]
    fn update_extent_width_replaces_cx() {
        let xml = r#"<a:ext cx="12345" cy="67890"/>"#;
        let result = update_extent_width(xml, 99999).unwrap();
        assert!(result.contains("cx=\"99999\""));
        assert!(result.contains("cy=\"67890\""));
    }
}
