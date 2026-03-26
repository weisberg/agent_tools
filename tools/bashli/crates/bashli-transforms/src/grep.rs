use regex::RegexBuilder;
use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct GrepTransform;
impl TransformFn for GrepTransform {
    fn name(&self) -> &str { "grep" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let pattern = config.get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TransformError::InvalidConfig("grep".into(), "requires 'pattern' field".into()))?;
        let invert = config.get("invert").and_then(|v| v.as_bool()).unwrap_or(false);
        let ignore_case = config.get("ignore_case").and_then(|v| v.as_bool()).unwrap_or(false);
        let only_matching = config.get("only_matching").and_then(|v| v.as_bool()).unwrap_or(false);
        let count = config.get("count").and_then(|v| v.as_bool()).unwrap_or(false);

        let re = RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()?;

        let matches: Vec<&str> = input
            .lines()
            .filter(|line| re.is_match(line) != invert)
            .collect();

        if count {
            return Ok(Value::Number(matches.len().into()));
        }

        if only_matching {
            let extracts: Vec<Value> = matches
                .iter()
                .filter_map(|line| re.find(line).map(|m| Value::String(m.as_str().to_string())))
                .collect();
            return Ok(Value::Array(extracts));
        }

        Ok(Value::String(matches.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_basic() {
        let t = GrepTransform;
        let config = serde_json::json!({"pattern": "error"});
        let result = t.apply("error: foo\nwarning: bar\nerror: baz", &config).unwrap();
        assert_eq!(result, Value::String("error: foo\nerror: baz".into()));
    }

    #[test]
    fn test_grep_invert() {
        let t = GrepTransform;
        let config = serde_json::json!({"pattern": "error", "invert": true});
        let result = t.apply("error: foo\nwarning: bar", &config).unwrap();
        assert_eq!(result, Value::String("warning: bar".into()));
    }

    #[test]
    fn test_grep_count() {
        let t = GrepTransform;
        let config = serde_json::json!({"pattern": "error", "count": true});
        let result = t.apply("error: foo\nwarning: bar\nerror: baz", &config).unwrap();
        assert_eq!(result, Value::Number(2.into()));
    }

    #[test]
    fn test_grep_case_insensitive() {
        let t = GrepTransform;
        let config = serde_json::json!({"pattern": "ERROR", "ignore_case": true});
        let result = t.apply("error: foo\nwarning: bar", &config).unwrap();
        assert_eq!(result, Value::String("error: foo".into()));
    }
}
