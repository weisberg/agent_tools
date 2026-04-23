use docli_core::DocliError;

/// Add a new relationship to a `.rels` file (XML bytes).
/// Returns `(modified_xml_bytes, new_rId)`.
pub fn add_relationship(
    rels_xml: &[u8],
    rel_type: &str,
    target: &str,
) -> Result<(Vec<u8>, String), DocliError> {
    let xml_str = std::str::from_utf8(rels_xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8 in .rels: {e}"),
    })?;

    let doc = roxmltree::Document::parse(xml_str)?;

    // Find the highest existing rId number.
    let mut max_id: u64 = 0;
    for node in doc.descendants() {
        if node.is_element() && node.tag_name().name() == "Relationship" {
            if let Some(id_val) = node.attribute("Id") {
                if let Some(num_str) = id_val.strip_prefix("rId") {
                    if let Ok(n) = num_str.parse::<u64>() {
                        max_id = max_id.max(n);
                    }
                }
            }
        }
    }

    let new_id = max_id + 1;
    let new_rid = format!("rId{new_id}");
    let new_element = format!(
        r#"<Relationship Id="{new_rid}" Type="{rel_type}" Target="{target}"/>"#
    );

    // Insert before the closing </Relationships> tag.
    let closing = "</Relationships>";
    let insert_pos = xml_str.rfind(closing).ok_or_else(|| DocliError::InvalidDocx {
        message: "missing </Relationships> tag in .rels".into(),
    })?;

    let mut result = String::with_capacity(xml_str.len() + new_element.len() + 1);
    result.push_str(&xml_str[..insert_pos]);
    result.push_str(&new_element);
    result.push_str(closing);

    Ok((result.into_bytes(), new_rid))
}

/// Add a content type to `[Content_Types].xml`.
pub fn add_content_type(
    content_types_xml: &[u8],
    part_path: &str,
    content_type: &str,
) -> Result<Vec<u8>, DocliError> {
    let xml_str =
        std::str::from_utf8(content_types_xml).map_err(|e| DocliError::InvalidDocx {
            message: format!("invalid UTF-8 in [Content_Types].xml: {e}"),
        })?;

    // Ensure the part path starts with '/'.
    let part_name = if part_path.starts_with('/') {
        part_path.to_string()
    } else {
        format!("/{part_path}")
    };

    let new_element = format!(
        r#"<Override PartName="{part_name}" ContentType="{content_type}"/>"#
    );

    let closing = "</Types>";
    let insert_pos = xml_str.rfind(closing).ok_or_else(|| DocliError::InvalidDocx {
        message: "missing </Types> tag in [Content_Types].xml".into(),
    })?;

    let mut result = String::with_capacity(xml_str.len() + new_element.len() + 1);
    result.push_str(&xml_str[..insert_pos]);
    result.push_str(&new_element);
    result.push_str(closing);

    Ok(result.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rels() -> Vec<u8> {
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://example.com/type1" Target="target1.xml"/>
  <Relationship Id="rId2" Type="http://example.com/type2" Target="target2.xml"/>
</Relationships>"#
            .to_vec()
    }

    #[test]
    fn add_relationship_creates_next_rid() {
        let (result, rid) =
            add_relationship(&sample_rels(), "http://example.com/type3", "target3.xml").unwrap();
        assert_eq!(rid, "rId3");
        let result_str = std::str::from_utf8(&result).unwrap();
        assert!(result_str.contains("rId3"));
        assert!(result_str.contains("target3.xml"));
        assert!(result_str.ends_with("</Relationships>"));
    }

    #[test]
    fn add_content_type_inserts_override() {
        let ct = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
</Types>"#;
        let result = add_content_type(
            ct,
            "word/comments.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
        )
        .unwrap();
        let result_str = std::str::from_utf8(&result).unwrap();
        assert!(result_str.contains(r#"PartName="/word/comments.xml""#));
        assert!(result_str.ends_with("</Types>"));
    }

    #[test]
    fn add_content_type_with_leading_slash() {
        let ct = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"></Types>"#;
        let result = add_content_type(ct, "/word/foo.xml", "application/xml").unwrap();
        let result_str = std::str::from_utf8(&result).unwrap();
        assert!(result_str.contains(r#"PartName="/word/foo.xml""#));
    }
}
