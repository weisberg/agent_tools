use serde_json::{Map, Value};

use crate::error::VaultliError;
use crate::util::order_metadata;

pub fn parse_frontmatter_text(
    text: &str,
    display_path: &str,
) -> Result<(Map<String, Value>, String, bool), VaultliError> {
    let mut lines = text.split_inclusive('\n');
    let first = match lines.next() {
        Some(line) => line,
        None => return Ok((Map::new(), text.to_string(), false)),
    };
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return Ok((Map::new(), text.to_string(), false));
    }

    let mut metadata_text = String::new();
    let mut body = String::new();
    let mut found_closing = false;
    let mut closing_seen = false;
    for line in lines {
        if !closing_seen {
            if line.trim_end_matches(['\r', '\n']) == "---" {
                closing_seen = true;
                found_closing = true;
                continue;
            }
            metadata_text.push_str(line);
        } else {
            body.push_str(line);
        }
    }

    if !found_closing {
        return Err(VaultliError::MalformedFrontmatter(display_path.to_string()));
    }

    let parsed: Value = if metadata_text.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_yaml::from_str(&metadata_text).map_err(|err| {
            VaultliError::InvalidFrontmatter(display_path.to_string(), err.to_string())
        })?
    };

    let metadata = match parsed {
        Value::Object(map) => map,
        Value::Null => Map::new(),
        _ => {
            return Err(VaultliError::InvalidFrontmatter(
                display_path.to_string(),
                "Frontmatter must deserialize to a mapping".into(),
            ));
        }
    };

    Ok((order_metadata(&metadata), body, true))
}

pub fn render_frontmatter_yaml(metadata: &Map<String, Value>) -> Result<String, VaultliError> {
    let value = Value::Object(metadata.clone());
    let rendered = serde_yaml::to_string(&value)
        .map_err(|err| VaultliError::Unsupported(format!("yaml serialization failed: {err}")))?;
    Ok(rendered.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_frontmatter_text;

    #[test]
    fn parses_lists_and_scalars() {
        let text = "---\nid: docs/guide\ntitle: Guide\ntags:\n  - one\n  - two\n---\nBody";
        let (metadata, body, has_frontmatter) =
            parse_frontmatter_text(text, "docs/guide.md").unwrap();
        assert!(has_frontmatter);
        assert_eq!(metadata["id"], "docs/guide");
        assert_eq!(metadata["tags"][0], "one");
        assert_eq!(body, "Body");
    }

    #[test]
    fn parses_folded_scalars() {
        let text = "---\ndescription: >-\n  one\n  two\n---\nBody";
        let (metadata, _, _) = parse_frontmatter_text(text, "docs/guide.md").unwrap();
        assert_eq!(metadata["description"], "one two");
    }

    #[test]
    fn parses_flow_sequence() {
        let text = "---\nid: a\ntitle: t\ndescription: d\ntags: [alpha, beta]\n---\n";
        let (metadata, _, _) = parse_frontmatter_text(text, "a.md").unwrap();
        assert_eq!(metadata["tags"][0], "alpha");
        assert_eq!(metadata["tags"][1], "beta");
    }

    #[test]
    fn quoted_strings_preserve_colons() {
        let text = "---\ntitle: \"a: b\"\n---\n";
        let (metadata, _, _) = parse_frontmatter_text(text, "a.md").unwrap();
        assert_eq!(metadata["title"], "a: b");
    }

    #[test]
    fn no_frontmatter_returns_false() {
        let text = "Just body\n";
        let (_, body, has_frontmatter) = parse_frontmatter_text(text, "a.md").unwrap();
        assert!(!has_frontmatter);
        assert_eq!(body, text);
    }

    #[test]
    fn malformed_frontmatter_errors() {
        let text = "---\nfoo: bar\nno closing\n";
        assert!(parse_frontmatter_text(text, "a.md").is_err());
    }
}
