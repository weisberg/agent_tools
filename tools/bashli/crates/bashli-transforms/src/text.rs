use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct TrimTransform;
impl TransformFn for TrimTransform {
    fn name(&self) -> &str { "trim" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::String(input.trim().to_string()))
    }
}

pub struct LinesTransform;
impl TransformFn for LinesTransform {
    fn name(&self) -> &str { "lines" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        let lines: Vec<Value> = input.lines().map(|l| Value::String(l.to_string())).collect();
        Ok(Value::Array(lines))
    }
}

pub struct CountLinesTransform;
impl TransformFn for CountLinesTransform {
    fn name(&self) -> &str { "count_lines" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::Number(input.lines().count().into()))
    }
}

pub struct CountBytesTransform;
impl TransformFn for CountBytesTransform {
    fn name(&self) -> &str { "count_bytes" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::Number(input.len().into()))
    }
}

pub struct CountWordsTransform;
impl TransformFn for CountWordsTransform {
    fn name(&self) -> &str { "count_words" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        Ok(Value::Number(input.split_whitespace().count().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim() {
        let t = TrimTransform;
        assert_eq!(t.apply("  hello  ", &Value::Null).unwrap(), Value::String("hello".into()));
    }

    #[test]
    fn test_lines() {
        let t = LinesTransform;
        let result = t.apply("a\nb\nc", &Value::Null).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], Value::String("a".into()));
    }

    #[test]
    fn test_count_lines() {
        let t = CountLinesTransform;
        assert_eq!(t.apply("a\nb\nc", &Value::Null).unwrap(), Value::Number(3.into()));
    }

    #[test]
    fn test_count_bytes() {
        let t = CountBytesTransform;
        assert_eq!(t.apply("hello", &Value::Null).unwrap(), Value::Number(5.into()));
    }

    #[test]
    fn test_count_words() {
        let t = CountWordsTransform;
        assert_eq!(t.apply("hello world foo", &Value::Null).unwrap(), Value::Number(3.into()));
    }
}
