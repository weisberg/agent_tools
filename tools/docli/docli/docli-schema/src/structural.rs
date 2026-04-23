use std::collections::HashSet;

use docli_core::Package;

use crate::ValidationIssue;

pub fn validate_structure(package: &Package) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for required in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
    ] {
        if !package.inventory.entries.contains_key(required) {
            issues.push(ValidationIssue::error(
                "missing-required-part",
                format!("required package part is missing: {required}"),
                Some(required),
            ));
        }
    }

    if let Some(content_types) = package.xml_parts.get("[Content_Types].xml") {
        let xml = String::from_utf8_lossy(content_types);
        if !xml.contains("Extension=\"rels\"") {
            issues.push(ValidationIssue::error(
                "missing-content-type-default",
                "content types missing rels default registration",
                Some("[Content_Types].xml"),
            ));
        }
        if !xml.contains("Extension=\"xml\"") {
            issues.push(ValidationIssue::error(
                "missing-content-type-default",
                "content types missing xml default registration",
                Some("[Content_Types].xml"),
            ));
        }
        if !xml.contains("PartName=\"/word/document.xml\"") {
            issues.push(ValidationIssue::error(
                "missing-content-type-override",
                "content types missing document.xml override",
                Some("[Content_Types].xml"),
            ));
        }
    }

    let overrides = collect_content_type_overrides(package);
    for part in [
        "word/comments.xml",
        "word/endnotes.xml",
        "word/footnotes.xml",
    ] {
        if package.inventory.entries.contains_key(part) && !overrides.contains(part) {
            issues.push(ValidationIssue::warning(
                "missing-content-type-override",
                format!("missing override for optional part {part}"),
                Some("[Content_Types].xml"),
            ));
        }
    }

    issues
}

fn collect_content_type_overrides(package: &Package) -> HashSet<String> {
    let Some(content_types) = package.xml_parts.get("[Content_Types].xml") else {
        return HashSet::new();
    };
    let xml = String::from_utf8_lossy(content_types);
    xml.lines()
        .filter_map(|line| extract_part_name(line))
        .map(|part| part.trim_start_matches('/').to_string())
        .collect()
}

fn extract_part_name(line: &str) -> Option<&str> {
    let marker = "PartName=\"";
    let start = line.find(marker)?;
    let rest = &line[start + marker.len()..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use docli_core::Package;

    use super::validate_structure;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/minimal.docx")
    }

    #[test]
    fn reports_missing_document_relationship_part() {
        let package = Package::open(fixture_path()).unwrap();
        let issues = validate_structure(&package);

        assert!(issues.iter().any(|issue| {
            issue.code == "missing-required-part"
                && issue.part.as_deref() == Some("word/_rels/document.xml.rels")
        }));
    }
}
