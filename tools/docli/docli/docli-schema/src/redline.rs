//! Validate tracked-change (redline) integrity in OOXML packages.

use std::collections::HashSet;

use docli_core::Package;

use crate::ValidationIssue;

/// Check tracked change integrity: balanced ins/del, valid IDs, proper nesting.
///
/// Also called as `validate_redlines` (the public re-export name).
pub fn check_redline(package: &Package) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for (part_path, bytes) in &package.xml_parts {
        let xml = String::from_utf8_lossy(bytes);
        let document = match roxmltree::Document::parse(&xml) {
            Ok(d) => d,
            Err(_) => continue, // XML parse errors are reported by other validators.
        };

        let mut seen_ids: HashSet<u64> = HashSet::new();

        for node in document.descendants().filter(|n| n.is_element()) {
            let tag = node.tag_name().name();

            match tag {
                "ins" | "del" => {
                    // Check for required w:id attribute.
                    let id_attr = node
                        .attributes()
                        .find(|a| a.name() == "id")
                        .map(|a| a.value().to_string());

                    match id_attr {
                        None => {
                            issues.push(ValidationIssue::error(
                                "redline-missing-id",
                                format!("w:{tag} element missing w:id attribute"),
                                Some(part_path),
                            ));
                        }
                        Some(ref val) => {
                            if let Ok(id) = val.parse::<u64>() {
                                if !seen_ids.insert(id) {
                                    issues.push(ValidationIssue::error(
                                        "redline-duplicate-id",
                                        format!(
                                            "duplicate tracked-change id {id} in w:{tag}"
                                        ),
                                        Some(part_path),
                                    ));
                                }
                            } else {
                                issues.push(ValidationIssue::error(
                                    "redline-invalid-id",
                                    format!(
                                        "w:{tag} has non-numeric w:id: {val}"
                                    ),
                                    Some(part_path),
                                ));
                            }
                        }
                    }

                    // Check for required w:author.
                    let has_author = node
                        .attributes()
                        .any(|a| a.name() == "author");
                    if !has_author {
                        issues.push(ValidationIssue::warning(
                            "redline-missing-author",
                            format!("w:{tag} element missing w:author attribute"),
                            Some(part_path),
                        ));
                    }

                    // Check nesting: ins/del should not be nested inside another ins/del.
                    let nested = node
                        .ancestors()
                        .skip(1)
                        .any(|a| {
                            a.is_element()
                                && (a.tag_name().name() == "ins"
                                    || a.tag_name().name() == "del")
                        });
                    if nested {
                        issues.push(ValidationIssue::error(
                            "redline-nested",
                            format!(
                                "w:{tag} is nested inside another tracked change"
                            ),
                            Some(part_path),
                        ));
                    }

                    // Check that del contains delText, not regular t.
                    if tag == "del" {
                        let has_del_text = node.descendants().any(|d| {
                            d.is_element() && d.tag_name().name() == "delText"
                        });
                        let has_regular_t = node.descendants().any(|d| {
                            d.is_element()
                                && d.tag_name().name() == "t"
                                && d.parent()
                                    .is_some_and(|p| p.tag_name().name() == "r")
                        });
                        if has_regular_t && !has_del_text {
                            issues.push(ValidationIssue::warning(
                                "redline-del-uses-t",
                                "w:del contains w:t instead of w:delText",
                                Some(part_path),
                            ));
                        }
                    }
                }
                "commentRangeStart" => {
                    // Check that a matching commentRangeEnd exists.
                    if let Some(id_val) = node
                        .attributes()
                        .find(|a| a.name() == "id")
                        .map(|a| a.value().to_string())
                    {
                        let has_end = document.descendants().any(|d| {
                            d.is_element()
                                && d.tag_name().name() == "commentRangeEnd"
                                && d.attributes()
                                    .any(|a| a.name() == "id" && a.value() == id_val)
                        });
                        if !has_end {
                            issues.push(ValidationIssue::error(
                                "redline-unbalanced-comment-range",
                                format!(
                                    "commentRangeStart id={id_val} has no matching commentRangeEnd"
                                ),
                                Some(part_path),
                            ));
                        }
                    }
                }
                "commentRangeEnd" => {
                    if let Some(id_val) = node
                        .attributes()
                        .find(|a| a.name() == "id")
                        .map(|a| a.value().to_string())
                    {
                        let has_start = document.descendants().any(|d| {
                            d.is_element()
                                && d.tag_name().name() == "commentRangeStart"
                                && d.attributes()
                                    .any(|a| a.name() == "id" && a.value() == id_val)
                        });
                        if !has_start {
                            issues.push(ValidationIssue::error(
                                "redline-unbalanced-comment-range",
                                format!(
                                    "commentRangeEnd id={id_val} has no matching commentRangeStart"
                                ),
                                Some(part_path),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    issues
}

/// Public alias kept for backward compatibility with the existing `validate_redlines` export.
pub fn validate_redlines(package: &Package) -> Vec<ValidationIssue> {
    check_redline(package)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        path::PathBuf,
    };

    use docli_core::{Package, PartEntry, PartInventory};

    /// Build a minimal in-memory Package with the given XML parts.
    fn make_package(parts: Vec<(&str, &str)>) -> Package {
        let mut xml_parts = HashMap::new();
        let mut entries = BTreeMap::new();

        for (path, content) in &parts {
            xml_parts.insert(path.to_string(), content.as_bytes().to_vec());
            entries.insert(
                path.to_string(),
                PartEntry {
                    path: path.to_string(),
                    sha256: String::new(),
                    is_xml: true,
                    size_bytes: content.len() as u64,
                },
            );
        }

        Package {
            path: PathBuf::from("test.docx"),
            source_hash: String::new(),
            inventory: PartInventory { entries },
            xml_parts,
            binary_parts: BTreeSet::new(),
        }
    }

    #[test]
    fn valid_tracked_changes_produce_no_issues() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="1" w:author="A" w:date="2025-01-01"><w:r><w:t>new</w:t></w:r></w:ins></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(issues.is_empty(), "expected no issues, got: {issues:?}");
    }

    #[test]
    fn detects_duplicate_tracked_change_ids() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="1" w:author="A" w:date="2025-01-01"><w:r><w:t>a</w:t></w:r></w:ins></w:p><w:p><w:del w:id="1" w:author="A" w:date="2025-01-01"><w:r><w:delText>b</w:delText></w:r></w:del></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(
            issues.iter().any(|i| i.code == "redline-duplicate-id"),
            "expected duplicate id issue, got: {issues:?}"
        );
    }

    #[test]
    fn detects_missing_id_attribute() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:author="A" w:date="2025-01-01"><w:r><w:t>x</w:t></w:r></w:ins></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(
            issues.iter().any(|i| i.code == "redline-missing-id"),
            "expected missing id issue, got: {issues:?}"
        );
    }

    #[test]
    fn detects_nested_tracked_changes() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="1" w:author="A" w:date="2025-01-01"><w:ins w:id="2" w:author="A" w:date="2025-01-01"><w:r><w:t>nested</w:t></w:r></w:ins></w:ins></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(
            issues.iter().any(|i| i.code == "redline-nested"),
            "expected nested issue, got: {issues:?}"
        );
    }

    #[test]
    fn detects_unbalanced_comment_range() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="10"/><w:r><w:t>text</w:t></w:r></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(
            issues
                .iter()
                .any(|i| i.code == "redline-unbalanced-comment-range"),
            "expected unbalanced comment range issue, got: {issues:?}"
        );
    }

    #[test]
    fn balanced_comment_range_is_ok() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:commentRangeStart w:id="10"/><w:r><w:t>text</w:t></w:r><w:commentRangeEnd w:id="10"/></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(
            !issues
                .iter()
                .any(|i| i.code == "redline-unbalanced-comment-range"),
            "expected no unbalanced issue, got: {issues:?}"
        );
    }

    #[test]
    fn detects_missing_author() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:ins w:id="1" w:date="2025-01-01"><w:r><w:t>x</w:t></w:r></w:ins></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(
            issues.iter().any(|i| i.code == "redline-missing-author"),
            "expected missing author issue, got: {issues:?}"
        );
    }

    #[test]
    fn empty_document_produces_no_issues() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
        let package = make_package(vec![("word/document.xml", xml)]);
        let issues = check_redline(&package);
        assert!(issues.is_empty());
    }
}
