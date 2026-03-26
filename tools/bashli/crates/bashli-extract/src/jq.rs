use serde_json::Value;
use crate::registry::{ExtractorFn, ExtractionError};

pub struct JqExtractor;
impl ExtractorFn for JqExtractor {
    fn name(&self) -> &str { "jq" }
    fn extract(&self, input: &str, config: &Value) -> Result<Value, ExtractionError> {
        let expr = config.as_str().ok_or_else(|| {
            ExtractionError::InvalidConfig("jq".into(), "requires a filter expression string".into())
        })?;
        bashli_jq::eval(expr, input)
            .map_err(|e| ExtractionError::Jq(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jq_extraction() {
        let e = JqExtractor;
        let result = e.extract(
            r#"{"name": "bashli", "version": "1.0"}"#,
            &Value::String(".name".into()),
        ).unwrap();
        assert_eq!(result, Value::String("bashli".into()));
    }
}
