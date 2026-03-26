use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct SortTransform;
impl TransformFn for SortTransform {
    fn name(&self) -> &str { "sort" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let numeric = config.get("numeric").and_then(|v| v.as_bool()).unwrap_or(false);
        let reverse = config.get("reverse").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut lines: Vec<&str> = input.lines().collect();

        if numeric {
            lines.sort_by(|a, b| {
                let na = a.trim().parse::<f64>().unwrap_or(f64::MAX);
                let nb = b.trim().parse::<f64>().unwrap_or(f64::MAX);
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            lines.sort();
        }

        if reverse {
            lines.reverse();
        }

        Ok(Value::String(lines.join("\n")))
    }
}

pub struct UniqueTransform;
impl TransformFn for UniqueTransform {
    fn name(&self) -> &str { "unique" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<&str> = input
            .lines()
            .filter(|line| seen.insert(*line))
            .collect();
        Ok(Value::String(unique.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort() {
        let t = SortTransform;
        let result = t.apply("c\na\nb", &Value::Null).unwrap();
        assert_eq!(result, Value::String("a\nb\nc".into()));
    }

    #[test]
    fn test_sort_numeric() {
        let t = SortTransform;
        let config = serde_json::json!({"numeric": true});
        let result = t.apply("10\n2\n1", &config).unwrap();
        assert_eq!(result, Value::String("1\n2\n10".into()));
    }

    #[test]
    fn test_sort_reverse() {
        let t = SortTransform;
        let config = serde_json::json!({"reverse": true});
        let result = t.apply("a\nc\nb", &config).unwrap();
        assert_eq!(result, Value::String("c\nb\na".into()));
    }

    #[test]
    fn test_unique() {
        let t = UniqueTransform;
        let result = t.apply("a\nb\na\nc\nb", &Value::Null).unwrap();
        assert_eq!(result, Value::String("a\nb\nc".into()));
    }
}
