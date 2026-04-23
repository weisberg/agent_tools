use std::collections::{HashMap, HashSet};

use docli_core::Package;

use crate::ValidationIssue;

pub fn check_invariants(package: &Package) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut ids: HashMap<String, String> = HashMap::new();

    for (part_path, bytes) in &package.xml_parts {
        let xml = String::from_utf8_lossy(bytes);
        let document = match roxmltree::Document::parse(&xml) {
            Ok(document) => document,
            Err(error) => {
                issues.push(ValidationIssue::error(
                    "invalid-xml",
                    error.to_string(),
                    Some(part_path),
                ));
                continue;
            }
        };

        for node in document.descendants().filter(|node| node.is_element()) {
            for attribute in node.attributes() {
                if attribute.name() == "id"
                    && attribute
                        .namespace()
                        .is_some_and(|namespace| namespace.contains("wordprocessingml"))
                {
                    if let Some(previous_part) =
                        ids.insert(attribute.value().to_string(), part_path.clone())
                    {
                        issues.push(ValidationIssue::error(
                            "duplicate-word-id",
                            format!(
                                "duplicate w:id {} found in {} and {}",
                                attribute.value(),
                                previous_part,
                                part_path
                            ),
                            Some(part_path),
                        ));
                    }
                }
            }

            match node.tag_name().name() {
                "ins" | "del" => {
                    if node
                        .ancestors()
                        .skip(1)
                        .any(|ancestor| ancestor.has_tag_name("r") || ancestor.has_tag_name("t"))
                    {
                        issues.push(ValidationIssue::error(
                            "invalid-tracked-change-nesting",
                            "tracked changes may not be nested inside w:r or w:t",
                            Some(part_path),
                        ));
                    }
                }
                "commentRangeStart" | "commentRangeEnd" => {
                    if node.parent().is_some_and(|parent| parent.has_tag_name("r")) {
                        issues.push(ValidationIssue::error(
                            "invalid-comment-range-placement",
                            "comment range markers must be siblings of w:r, not children",
                            Some(part_path),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    let content_types = declared_content_types(package);
    let relationships = package
        .xml_parts
        .get("word/_rels/document.xml.rels")
        .and_then(|bytes| parse_relationship_targets(bytes).ok())
        .unwrap_or_default();

    if package.inventory.entries.contains_key("word/comments.xml")
        && !content_types.contains("word/comments.xml")
    {
        issues.push(ValidationIssue::error(
            "missing-comments-content-type",
            "word/comments.xml exists but is not declared in [Content_Types].xml",
            Some("[Content_Types].xml"),
        ));
    }

    for target in relationships {
        if target.starts_with("media/") {
            let extension = target.rsplit('.').next().unwrap_or_default();
            if !content_types.contains(&format!("*.{extension}")) {
                issues.push(ValidationIssue::warning(
                    "missing-media-content-type",
                    format!("missing content type default for media extension {extension}"),
                    Some("[Content_Types].xml"),
                ));
            }
        }
    }

    issues
}

fn declared_content_types(package: &Package) -> HashSet<String> {
    let Some(content_types) = package.xml_parts.get("[Content_Types].xml") else {
        return HashSet::new();
    };
    let xml = String::from_utf8_lossy(content_types);
    let mut entries = HashSet::new();
    for line in xml.lines() {
        if let Some(part_name) = extract_attribute(line, "PartName") {
            entries.insert(part_name.trim_start_matches('/').to_string());
        }
        if let Some(extension) = extract_attribute(line, "Extension") {
            entries.insert(format!("*.{extension}"));
        }
    }
    entries
}

fn parse_relationship_targets(bytes: &[u8]) -> Result<Vec<String>, roxmltree::Error> {
    let xml = String::from_utf8_lossy(bytes);
    let document = roxmltree::Document::parse(&xml)?;
    Ok(document
        .descendants()
        .filter(|node| node.has_tag_name("Relationship"))
        .filter_map(|node| node.attribute("Target").map(ToString::to_string))
        .collect())
}

fn extract_attribute<'a>(line: &'a str, attr: &str) -> Option<&'a str> {
    let marker = format!("{attr}=\"");
    let start = line.find(&marker)?;
    let rest = &line[start + marker.len()..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write, path::PathBuf};

    use docli_core::Package;
    use tempfile::NamedTempFile;
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::check_invariants;

    fn build_docx(document_xml: &str, rels_xml: &str, content_types: &str) -> PathBuf {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(content_types.as_bytes()).unwrap();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(rels_xml.as_bytes()).unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(document_xml.as_bytes()).unwrap();
        zip.start_file("word/_rels/document.xml.rels", options)
            .unwrap();
        zip.write_all(rels_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
        let (_file, kept_path) = temp.keep().unwrap();
        kept_path
    }

    #[test]
    fn reports_duplicate_word_ids() {
        let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
              <w:p><w:bookmarkStart w:id="7" w:name="a"/></w:p>
              <w:p><w:bookmarkStart w:id="7" w:name="b"/></w:p>
            </w:body>
        </w:document>"#;
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#;
        let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
            <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
            <Default Extension="xml" ContentType="application/xml"/>
            <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
        </Types>"#;
        let package = Package::open(build_docx(document, rels, types)).unwrap();

        let issues = check_invariants(&package);
        assert!(issues.iter().any(|issue| issue.code == "duplicate-word-id"));
    }

    #[test]
    fn reports_invalid_tracked_change_nesting() {
        let document = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body><w:p><w:r><w:ins/></w:r></w:p></w:body>
        </w:document>"#;
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#;
        let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
            <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
            <Default Extension="xml" ContentType="application/xml"/>
            <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
        </Types>"#;
        let package = Package::open(build_docx(document, rels, types)).unwrap();

        let issues = check_invariants(&package);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "invalid-tracked-change-nesting"));
    }
}
