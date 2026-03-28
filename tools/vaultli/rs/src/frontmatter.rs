use serde_json::{Map, Number, Value};

use crate::error::VaultliError;

pub fn parse_frontmatter_text(
    text: &str,
    display_path: &str,
) -> Result<(Map<String, Value>, String, bool), VaultliError> {
    if !text.starts_with("---\n") && text.trim() != "---" {
        return Ok((Map::new(), text.to_string(), false));
    }

    let mut lines = text.lines();
    let first = lines.next();
    if first != Some("---") {
        return Ok((Map::new(), text.to_string(), false));
    }

    let mut metadata_lines = Vec::new();
    let mut found_closing = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            found_closing = true;
            break;
        }
        metadata_lines.push(line.to_string());
    }

    if !found_closing {
        return Err(VaultliError::MalformedFrontmatter(display_path.to_string()));
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    let metadata = parse_frontmatter_map(&metadata_lines, display_path)?;
    Ok((metadata, body, true))
}

fn parse_frontmatter_map(
    lines: &[String],
    display_path: &str,
) -> Result<Map<String, Value>, VaultliError> {
    let mut result = Map::new();
    let mut index = 0;

    while index < lines.len() {
        let raw = &lines[index];
        if raw.trim().is_empty() || raw.trim_start().starts_with('#') {
            index += 1;
            continue;
        }
        if raw.starts_with(' ') || raw.starts_with('\t') {
            return Err(VaultliError::InvalidFrontmatter(
                display_path.to_string(),
                format!("unexpected indentation at line {}", index + 1),
            ));
        }

        let (key, value_part) = raw.split_once(':').ok_or_else(|| {
            VaultliError::InvalidFrontmatter(display_path.to_string(), raw.clone())
        })?;
        let key = key.trim().to_string();
        let value_part = value_part.trim();

        if value_part.is_empty() {
            let mut items = Vec::new();
            let mut probe = index + 1;
            while probe < lines.len() {
                let candidate = &lines[probe];
                let trimmed = candidate.trim();
                if trimmed.is_empty() {
                    probe += 1;
                    continue;
                }
                if candidate.starts_with("  - ") || candidate.starts_with("\t- ") {
                    let value = candidate
                        .trim_start()
                        .strip_prefix("- ")
                        .unwrap_or_default()
                        .trim();
                    items.push(parse_scalar(value));
                    probe += 1;
                    continue;
                }
                if candidate.starts_with(' ') || candidate.starts_with('\t') {
                    return Err(VaultliError::InvalidFrontmatter(
                        display_path.to_string(),
                        format!("unsupported indented block for key {}", key),
                    ));
                }
                break;
            }
            result.insert(key, Value::Array(items));
            index = probe;
            continue;
        }

        if value_part == ">-" || value_part == ">" || value_part == "|" {
            let folded = value_part != "|";
            let mut collected = Vec::new();
            let mut probe = index + 1;
            while probe < lines.len() {
                let candidate = &lines[probe];
                if candidate.starts_with("  ") || candidate.starts_with('\t') {
                    collected.push(candidate.trim().to_string());
                    probe += 1;
                    continue;
                }
                if candidate.trim().is_empty() {
                    collected.push(String::new());
                    probe += 1;
                    continue;
                }
                break;
            }
            let rendered = if folded {
                collected
                    .into_iter()
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                collected.join("\n")
            };
            result.insert(key, Value::String(rendered));
            index = probe;
            continue;
        }

        result.insert(key, parse_scalar(value_part));
        index += 1;
    }

    Ok(result)
}

fn parse_scalar(raw: &str) -> Value {
    if raw.starts_with('[') && raw.ends_with(']') {
        let inner = &raw[1..raw.len() - 1];
        let items = inner
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(parse_scalar)
            .collect::<Vec<_>>();
        return Value::Array(items);
    }

    if let Some(stripped) = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Value::String(stripped.to_string());
    }
    if let Some(stripped) = raw
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        return Value::String(stripped.to_string());
    }
    if let Ok(number) = raw.parse::<i64>() {
        return Value::Number(Number::from(number));
    }
    if raw == "true" {
        return Value::Bool(true);
    }
    if raw == "false" {
        return Value::Bool(false);
    }
    Value::String(raw.to_string())
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
}
