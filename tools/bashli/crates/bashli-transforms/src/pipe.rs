use bashli_core::transform::Transform;
use serde_json::Value;
use crate::registry::{TransformRegistry, TransformError};

/// Apply a chain of transforms sequentially.
pub fn apply_pipe(
    registry: &TransformRegistry,
    input: &str,
    transforms: &[Transform],
) -> Result<Value, TransformError> {
    let mut current = Value::String(input.to_string());

    for transform in transforms {
        let input_str = value_to_string(&current);
        current = registry.apply(&input_str, transform)?;
    }

    Ok(current)
}

/// Convert a serde_json::Value to a string for the next transform in the pipe.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        // Arrays and objects serialize to JSON
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_trim_then_lines() {
        let registry = TransformRegistry::default_registry();
        let transforms = vec![Transform::Trim, Transform::Lines];
        let result = apply_pipe(&registry, "  a\n  b\n  c  ", &transforms).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn test_pipe_head_then_count() {
        let registry = TransformRegistry::default_registry();
        let transforms = vec![Transform::Head(2), Transform::CountLines];
        let result = apply_pipe(&registry, "a\nb\nc\nd", &transforms).unwrap();
        assert_eq!(result, Value::Number(2.into()));
    }
}
