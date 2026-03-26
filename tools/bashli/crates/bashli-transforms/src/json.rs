use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct JsonParseTransform;
impl TransformFn for JsonParseTransform {
    fn name(&self) -> &str { "json" }
    fn apply(&self, input: &str, _config: &Value) -> Result<Value, TransformError> {
        serde_json::from_str(input)
            .map_err(|e| TransformError::JsonParse(e.to_string()))
    }
}

pub struct SplitTransform;
impl TransformFn for SplitTransform {
    fn name(&self) -> &str { "split" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let delimiter = config.as_str().ok_or_else(|| {
            TransformError::InvalidConfig("split".into(), "requires a delimiter string".into())
        })?;
        let parts: Vec<Value> = input
            .split(delimiter)
            .map(|s| Value::String(s.to_string()))
            .collect();
        Ok(Value::Array(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_parse() {
        let t = JsonParseTransform;
        let result = t.apply(r#"{"key": "value"}"#, &Value::Null).unwrap();
        assert_eq!(result.get("key").unwrap(), "value");
    }

    #[test]
    fn test_json_parse_invalid() {
        let t = JsonParseTransform;
        assert!(t.apply("not json", &Value::Null).is_err());
    }

    #[test]
    fn test_split() {
        let t = SplitTransform;
        let result = t.apply("a,b,c", &Value::String(",".into())).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[1], Value::String("b".into()));
    }
}
