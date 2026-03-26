use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct AwkTransform;

impl TransformFn for AwkTransform {
    fn name(&self) -> &str { "awk" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let program = config.get("program")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TransformError::InvalidConfig("awk".into(), "requires 'program' field".into()))?;

        let opts = bashli_awk::AwkOpts {
            field_separator: config.get("field_separator").and_then(|v| v.as_str()).map(String::from),
            vars: config.get("vars")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
        };

        let result = bashli_awk::eval(program, input, &opts)
            .map_err(|e| TransformError::Awk(e.to_string()))?;
        Ok(Value::String(result.trim_end_matches('\n').to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_awk_print_field() {
        let t = AwkTransform;
        let config = serde_json::json!({"program": "{print $2}"});
        let result = t.apply("hello world\nfoo bar", &config).unwrap();
        assert_eq!(result, Value::String("world\nbar".into()));
    }
}
