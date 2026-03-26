use serde_json::Value;
use crate::registry::{TransformFn, TransformError};

pub struct CodeBlockTransform;
impl TransformFn for CodeBlockTransform {
    fn name(&self) -> &str { "code_block" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let lang = config.as_str().unwrap_or("");
        let result = format!("```{lang}\n{input}\n```");
        Ok(Value::String(result))
    }
}

pub struct RegexTransform;
impl TransformFn for RegexTransform {
    fn name(&self) -> &str { "regex" }
    fn apply(&self, input: &str, config: &Value) -> Result<Value, TransformError> {
        let pattern = config.as_str().ok_or_else(|| {
            TransformError::InvalidConfig("regex".into(), "requires a pattern string".into())
        })?;
        let re = regex::Regex::new(pattern)?;
        let captures: Vec<Value> = re
            .captures_iter(input)
            .map(|cap| {
                if cap.len() == 1 {
                    Value::String(cap[0].to_string())
                } else {
                    let groups: Vec<Value> = cap
                        .iter()
                        .skip(1)
                        .map(|m| {
                            m.map(|m| Value::String(m.as_str().to_string()))
                                .unwrap_or(Value::Null)
                        })
                        .collect();
                    Value::Array(groups)
                }
            })
            .collect();
        Ok(Value::Array(captures))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_block() {
        let t = CodeBlockTransform;
        let result = t.apply("let x = 1;", &Value::String("rust".into())).unwrap();
        assert_eq!(result, Value::String("```rust\nlet x = 1;\n```".into()));
    }

    #[test]
    fn test_code_block_no_lang() {
        let t = CodeBlockTransform;
        let result = t.apply("hello", &Value::Null).unwrap();
        assert_eq!(result, Value::String("```\nhello\n```".into()));
    }

    #[test]
    fn test_regex_captures() {
        let t = RegexTransform;
        let result = t.apply("error[E0432] error[E0599]", &Value::String(r"error\[(E\d+)\]".into())).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }
}
