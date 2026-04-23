use docli_core::{DocliError, Scope};

use crate::part_graph::PartGraph;

/// Find all occurrences of `find` text and replace with `replace` text.
///
/// Scope controls: `All` (entire document), `First` (first occurrence only).
/// Returns the count of replacements made.
pub fn find_and_replace(
    graph: &mut PartGraph,
    part_path: &str,
    find: &str,
    replace: &str,
    scope: &Scope,
) -> Result<usize, DocliError> {
    if find.is_empty() {
        return Err(DocliError::InvalidOperation {
            message: "find string must not be empty".into(),
        });
    }

    let xml = graph
        .xml_bytes(part_path)
        .ok_or_else(|| DocliError::InvalidDocx {
            message: format!("part not found: {part_path}"),
        })?;

    let xml_str = std::str::from_utf8(xml).map_err(|e| DocliError::InvalidDocx {
        message: format!("invalid UTF-8: {e}"),
    })?;

    // Strategy: use regex to find <w:t ...>...</w:t> elements and replace text within them.
    // This handles the common case where find text is contained within single <w:t> elements.
    let re = regex::Regex::new(r#"<w:t([^>]*)>([^<]*)</w:t>"#).map_err(|e| {
        DocliError::InvalidOperation {
            message: format!("regex error: {e}"),
        }
    })?;

    let mut count = 0usize;
    let mut done = false;

    let result = re.replace_all(xml_str, |caps: &regex::Captures| {
        let attrs = &caps[1];
        let text = &caps[2];

        if done {
            return caps[0].to_string();
        }

        if !text.contains(find) {
            return caps[0].to_string();
        }

        let new_text = match scope {
            Scope::First => {
                if count == 0 {
                    let replaced = text.replacen(find, replace, 1);
                    count += 1;
                    done = true;
                    replaced
                } else {
                    return caps[0].to_string();
                }
            }
            Scope::All | Scope::Section(_) => {
                let occurrences = text.matches(find).count();
                count += occurrences;
                text.replace(find, replace)
            }
        };

        format!("<w:t{attrs}>{new_text}</w:t>")
    });

    if count > 0 {
        graph.set_xml(part_path, result.into_owned().into_bytes());
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::part_graph::PartData;

    fn make_graph(path: &str, xml: &str) -> PartGraph {
        let mut parts = HashMap::new();
        parts.insert(path.to_string(), PartData::Xml(xml.as_bytes().to_vec()));
        PartGraph { parts }
    }

    fn doc_with_text(texts: &[&str]) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        );
        for t in texts {
            xml.push_str(&format!(
                r#"<w:p><w:r><w:t xml:space="preserve">{t}</w:t></w:r></w:p>"#
            ));
        }
        xml.push_str("</w:body></w:document>");
        xml
    }

    #[test]
    fn replace_all_occurrences() {
        let xml = doc_with_text(&["Hello World", "Hello Again"]);
        let mut graph = make_graph("word/document.xml", &xml);

        let count = find_and_replace(
            &mut graph,
            "word/document.xml",
            "Hello",
            "Hi",
            &Scope::All,
        )
        .unwrap();

        assert_eq!(count, 2);
        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(!result.contains("Hello"));
        assert!(result.contains("Hi World"));
        assert!(result.contains("Hi Again"));
    }

    #[test]
    fn replace_first_only() {
        let xml = doc_with_text(&["Hello World", "Hello Again"]);
        let mut graph = make_graph("word/document.xml", &xml);

        let count = find_and_replace(
            &mut graph,
            "word/document.xml",
            "Hello",
            "Hi",
            &Scope::First,
        )
        .unwrap();

        assert_eq!(count, 1);
        let result =
            std::str::from_utf8(graph.xml_bytes("word/document.xml").unwrap())
                .unwrap();
        assert!(result.contains("Hi World"));
        assert!(result.contains("Hello Again"));
    }

    #[test]
    fn no_match_returns_zero() {
        let xml = doc_with_text(&["Hello World"]);
        let mut graph = make_graph("word/document.xml", &xml);

        let count = find_and_replace(
            &mut graph,
            "word/document.xml",
            "Missing",
            "Found",
            &Scope::All,
        )
        .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn empty_find_returns_error() {
        let xml = doc_with_text(&["Hello"]);
        let mut graph = make_graph("word/document.xml", &xml);
        let result =
            find_and_replace(&mut graph, "word/document.xml", "", "x", &Scope::All);
        assert!(result.is_err());
    }
}
