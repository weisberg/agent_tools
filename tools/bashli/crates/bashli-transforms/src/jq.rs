use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct JqTransform;

impl TransformFn for JqTransform {
    fn name(&self) -> &str { "jq" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let expr = config.as_str().ok_or_else(|| {
            TransformError::InvalidConfig("jq".into(), "requires a filter expression string".into())
        })?;
        bashli_jq::eval(expr, input)
            .map_err(|e| TransformError::Jq(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jq_identity() {
        let t = JqTransform;
        let result = t.apply(r#"{"a": 1}"#, &Value::String(".".into())).unwrap();
        assert_eq!(result.get("a").unwrap(), 1);
    }

    #[test]
    fn test_jq_field_access() {
        let t = JqTransform;
        let result = t.apply(r#"{"a": "hello"}"#, &Value::String(".a".into())).unwrap();
        assert_eq!(result, Value::String("hello".into()));
    }
}
