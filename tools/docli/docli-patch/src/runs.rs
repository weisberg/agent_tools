use super::run_split::RunFragment;

/// Merge adjacent runs that share identical formatting.
pub fn merge_adjacent_runs(runs: &[RunFragment]) -> Vec<RunFragment> {
    let mut merged: Vec<RunFragment> = Vec::new();

    for run in runs {
        if let Some(last) = merged.last_mut() {
            if last.properties == run.properties {
                last.text.push_str(&run.text);
                continue;
            }
        }
        merged.push(run.clone());
    }

    merged
}

/// Serialize a [`RunFragment`] back to OOXML run XML.
pub fn fragment_to_xml(fragment: &RunFragment) -> String {
    let mut xml = String::from("<w:r>");

    if let Some(ref props) = fragment.properties {
        xml.push_str(props);
    }

    // Determine if we need xml:space="preserve".
    let needs_preserve = fragment.text.starts_with(' ')
        || fragment.text.ends_with(' ')
        || fragment.text.contains("  ");

    let space_attr = if needs_preserve {
        r#" xml:space="preserve""#
    } else {
        ""
    };

    let escaped = escape_xml_text(&fragment.text);
    xml.push_str(&format!("<w:t{space_attr}>{escaped}</w:t>"));
    xml.push_str("</w:r>");

    xml
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_adjacent_same_props() {
        let runs = vec![
            RunFragment {
                properties: Some("<w:rPr><w:b/></w:rPr>".into()),
                text: "Hello".into(),
            },
            RunFragment {
                properties: Some("<w:rPr><w:b/></w:rPr>".into()),
                text: " World".into(),
            },
        ];
        let merged = merge_adjacent_runs(&runs);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "Hello World");
    }

    #[test]
    fn merge_adjacent_different_props() {
        let runs = vec![
            RunFragment {
                properties: Some("<w:rPr><w:b/></w:rPr>".into()),
                text: "Bold".into(),
            },
            RunFragment {
                properties: None,
                text: "Normal".into(),
            },
        ];
        let merged = merge_adjacent_runs(&runs);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn fragment_to_xml_basic() {
        let frag = RunFragment {
            properties: None,
            text: "Hello".into(),
        };
        let xml = fragment_to_xml(&frag);
        assert!(xml.contains("<w:t>Hello</w:t>"));
    }

    #[test]
    fn fragment_to_xml_with_space_preserve() {
        let frag = RunFragment {
            properties: None,
            text: " Hello ".into(),
        };
        let xml = fragment_to_xml(&frag);
        assert!(xml.contains(r#"xml:space="preserve""#));
    }

    #[test]
    fn fragment_to_xml_escapes_special_chars() {
        let frag = RunFragment {
            properties: None,
            text: "A & B < C".into(),
        };
        let xml = fragment_to_xml(&frag);
        assert!(xml.contains("A &amp; B &lt; C"));
    }
}
