use regex::Regex;
use serde_json::Value;
use crate::registry::{ExtractorFn, ExtractionError};

pub struct RegexExtractor;
impl ExtractorFn for RegexExtractor {
    fn name(&self) -> &str { "regex" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let pattern = config.as_str().ok_or_else(|| {
            ExtractionError::InvalidConfig("regex".into(), "requires a pattern string".into())
        })?;
        let re = Regex::new(pattern)?;

        // Collect all matches. If there are named groups, return objects; otherwise arrays.
        let group_names: Vec<Option<&str>> = re.capture_names().skip(1).collect();
        let has_named = group_names.iter().any(|n| n.is_some());

        let matches: Vec<Value> = re.captures_iter(input).map(|cap| {
            if has_named {
                let mut obj = serde_json::Map::new();
                for name in re.capture_names().flatten() {
                    if let Some(m) = cap.name(name) {
                        obj.insert(name.to_string(), Value::String(m.as_str().to_string()));
                    }
                }
                Value::Object(obj)
            } else if cap.len() > 1 {
                // Return captured groups (skip full match)
                let groups: Vec<Value> = cap.iter()
                    .skip(1)
                    .map(|m| m.map(|m| Value::String(m.as_str().to_string())).unwrap_or(Value::Null))
                    .collect();
                if groups.len() == 1 {
                    groups.into_iter().next().unwrap()
                } else {
                    Value::Array(groups)
                }
            } else {
                Value::String(cap[0].to_string())
            }
        }).collect();

        if matches.len() == 1 {
            Ok(matches.into_iter().next().unwrap())
        } else {
            Ok(Value::Array(matches))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_named_groups() {
        let e = RegexExtractor;
        let result = e.extract(
            "error[E0432]: unresolved import",
            &Value::String(r"error\[(?P<code>E\d+)\]".into()),
        ).unwrap();
        assert_eq!(result.get("code").unwrap(), "E0432");
    }

    #[test]
    fn test_regex_multiple_captures() {
        let e = RegexExtractor;
        let result = e.extract(
            "error[E0432] error[E0599]",
            &Value::String(r"error\[(E\d+)\]".into()),
        ).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], Value::String("E0432".into()));
        assert_eq!(arr[1], Value::String("E0599".into()));
    }

    #[test]
    fn test_regex_no_groups() {
        let e = RegexExtractor;
        let result = e.extract("hello world", &Value::String(r"\w+".into())).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0], Value::String("hello".into()));
    }
}
