use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct HeadTransform;
impl TransformFn for HeadTransform {
    fn name(&self) -> &str { "head" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let n = config.as_u64().ok_or_else(|| {
            TransformError::InvalidConfig("head".into(), "requires a number".into())
        })? as usize;
        let result: String = input.lines().take(n).collect::<Vec<_>>().join("\n");
        Ok(Value::String(result))
    }
}

pub struct TailTransform;
impl TransformFn for TailTransform {
    fn name(&self) -> &str { "tail" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let n = config.as_u64().ok_or_else(|| {
            TransformError::InvalidConfig("tail".into(), "requires a number".into())
        })? as usize;
        let lines: Vec<&str> = input.lines().collect();
        let start = lines.len().saturating_sub(n);
        Ok(Value::String(lines[start..].join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head() {
        let t = HeadTransform;
        let result = t.apply("a\nb\nc\nd\ne", &Value::Number(3.into())).unwrap();
        assert_eq!(result, Value::String("a\nb\nc".into()));
    }

    #[test]
    fn test_tail() {
        let t = TailTransform;
        let result = t.apply("a\nb\nc\nd\ne", &Value::Number(2.into())).unwrap();
        assert_eq!(result, Value::String("d\ne".into()));
    }

    #[test]
    fn test_head_more_than_available() {
        let t = HeadTransform;
        let result = t.apply("a\nb", &Value::Number(10.into())).unwrap();
        assert_eq!(result, Value::String("a\nb".into()));
    }
}
