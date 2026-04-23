use docli_core::DocliError;

/// Ensure all `<w:t>` elements with whitespace text have `xml:space="preserve"`.
///
/// This operates on raw XML bytes via regex-based rewriting. It finds `<w:t>` elements
/// that contain leading or trailing whitespace but lack the `xml:space="preserve"`
/// attribute, and adds it.
pub fn normalize_text_spaces(xml: &[u8]) -> Result<Vec<u8>, DocliError> {
    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    // Match all <w:t ...>...</w:t> elements. We then check in the replacement callback
    // whether the element already has xml:space and whether it needs preserve.
    let re = regex::Regex::new(r#"<w:t([^>]*)>([^<]*)</w:t>"#).expect("valid regex");

    let result = re.replace_all(xml_str, |caps: &regex::Captures| {
        let attrs = &caps[1];
        let text = &caps[2];

        // Already has xml:space — leave unchanged.
        if attrs.contains("xml:space") {
            return caps[0].to_string();
        }

        let needs_preserve =
            text.starts_with(' ') || text.ends_with(' ') || text.contains("  ");

        if needs_preserve {
            format!(r#"<w:t xml:space="preserve"{attrs}>{text}</w:t>"#)
        } else {
            caps[0].to_string()
        }
    });

    Ok(result.into_owned().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_preserve_to_whitespace_text() {
        let input = b"<w:t> hello </w:t>";
        let result = normalize_text_spaces(input).unwrap();
        let s = std::str::from_utf8(&result).unwrap();
        assert!(s.contains(r#"xml:space="preserve""#));
    }

    #[test]
    fn does_not_modify_non_whitespace_text() {
        let input = b"<w:t>hello</w:t>";
        let result = normalize_text_spaces(input).unwrap();
        let s = std::str::from_utf8(&result).unwrap();
        assert!(!s.contains("preserve"));
    }

    #[test]
    fn leaves_existing_preserve_alone() {
        let input = br#"<w:t xml:space="preserve"> hello </w:t>"#;
        let result = normalize_text_spaces(input).unwrap();
        assert_eq!(result, input.to_vec());
    }
}
