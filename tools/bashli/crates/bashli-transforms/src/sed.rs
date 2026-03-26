use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct SedTransform;

impl TransformFn for SedTransform {
    fn name(&self) -> &str { "sed" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let commands: Vec<&str> = match config {
            Value::String(s) => vec![s.as_str()],
            Value::Array(arr) => {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect()
            }
            _ => return Err(TransformError::InvalidConfig(
                "sed".into(),
                "requires a command string or array of command strings".into(),
            )),
        };

        let result = bashli_sed::apply(input, &commands)
            .map_err(|e| TransformError::Sed(e.to_string()))?;
        Ok(Value::String(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sed_replace() {
        let t = SedTransform;
        let result = t.apply("hello world", &Value::String("s/world/rust/".into())).unwrap();
        assert_eq!(result, Value::String("hello rust".into()));
    }
}
