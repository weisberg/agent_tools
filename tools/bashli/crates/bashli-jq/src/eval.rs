use crate::JqError;
use serde_json::Value;

/// Evaluate a jq filter expression against a JSON string.
pub fn eval(expr: &str, input: &str) -> Result<Value, JqError> {
    let value: Value = serde_json::from_str(input)?;
    eval_value(expr, &value)
}

/// Evaluate a jq filter expression against an already-parsed Value.
pub fn eval_value(expr: &str, input: &Value) -> Result<Value, JqError> {
    let expr = expr.trim();

    // Identity
    if expr == "." {
        return Ok(input.clone());
    }

    // Pipe: split on top-level | and chain
    if let Some((left, right)) = split_pipe(expr) {
        let intermediate = eval_value(left, input)?;
        return eval_value(right, &intermediate);
    }

    // Array construction: [expr]
    if expr.starts_with('[') && expr.ends_with(']') {
        let inner = &expr[1..expr.len() - 1].trim();
        if inner.is_empty() {
            return Ok(input.clone());
        }
        if let Value::Array(arr) = input {
            let results: Result<Vec<Value>, _> = arr.iter()
                .map(|item| eval_value(inner, item))
                .collect();
            return Ok(Value::Array(results?));
        }
        let result = eval_value(inner, input)?;
        return Ok(Value::Array(vec![result]));
    }

    // Field access chain: .foo.bar.baz or .foo[0].bar
    if expr.starts_with('.') {
        let path = &expr[1..];
        if path.is_empty() {
            return Ok(input.clone());
        }
        return resolve_path(input, path);
    }

    // Numeric literal
    if let Ok(n) = expr.parse::<i64>() {
        return Ok(serde_json::json!(n));
    }
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(serde_json::json!(n));
    }

    // String literal
    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        return Ok(Value::String(expr[1..expr.len() - 1].to_string()));
    }

    // null/true/false
    match expr {
        "null" => return Ok(Value::Null),
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }

    // Builtin functions
    if expr == "length" {
        return Ok(match input {
            Value::Array(arr) => Value::Number(arr.len().into()),
            Value::Object(obj) => Value::Number(obj.len().into()),
            Value::String(s) => Value::Number(s.len().into()),
            Value::Null => Value::Number(0.into()),
            _ => Value::Null,
        });
    }

    if expr == "keys" || expr == "keys_unsorted" {
        return match input {
            Value::Object(obj) => {
                let mut keys: Vec<String> = obj.keys().cloned().collect();
                if expr == "keys" {
                    keys.sort();
                }
                Ok(Value::Array(keys.into_iter().map(Value::String).collect()))
            }
            Value::Array(arr) => {
                let keys: Vec<Value> = (0..arr.len()).map(|i| Value::Number(i.into())).collect();
                Ok(Value::Array(keys))
            }
            _ => Err(JqError::EvalError("keys requires object or array".into())),
        };
    }

    if expr == "values" {
        return match input {
            Value::Object(obj) => Ok(Value::Array(obj.values().cloned().collect())),
            Value::Array(arr) => Ok(Value::Array(arr.clone())),
            _ => Err(JqError::EvalError("values requires object or array".into())),
        };
    }

    if expr == "type" {
        return Ok(Value::String(type_name(input).to_string()));
    }

    if expr == "not" {
        return Ok(Value::Bool(!is_truthy(input)));
    }

    if expr == "reverse" {
        return match input {
            Value::Array(arr) => {
                let mut reversed = arr.clone();
                reversed.reverse();
                Ok(Value::Array(reversed))
            }
            _ => Err(JqError::EvalError("reverse requires array".into())),
        };
    }

    if expr == "flatten" {
        return match input {
            Value::Array(arr) => {
                let mut flat = Vec::new();
                for item in arr {
                    if let Value::Array(inner) = item {
                        flat.extend(inner.iter().cloned());
                    } else {
                        flat.push(item.clone());
                    }
                }
                Ok(Value::Array(flat))
            }
            _ => Err(JqError::EvalError("flatten requires array".into())),
        };
    }

    if expr == "sort" {
        return match input {
            Value::Array(arr) => {
                let mut sorted = arr.clone();
                sorted.sort_by(|a, b| {
                    let sa = value_sort_key(a);
                    let sb = value_sort_key(b);
                    sa.cmp(&sb)
                });
                Ok(Value::Array(sorted))
            }
            _ => Err(JqError::EvalError("sort requires array".into())),
        };
    }

    if expr == "unique" {
        return match input {
            Value::Array(arr) => {
                let mut unique = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for item in arr {
                    let key = serde_json::to_string(item).unwrap_or_default();
                    if seen.insert(key) {
                        unique.push(item.clone());
                    }
                }
                Ok(Value::Array(unique))
            }
            _ => Err(JqError::EvalError("unique requires array".into())),
        };
    }

    if expr == "first" {
        return match input {
            Value::Array(arr) => Ok(arr.first().cloned().unwrap_or(Value::Null)),
            _ => Err(JqError::EvalError("first requires array".into())),
        };
    }

    if expr == "last" {
        return match input {
            Value::Array(arr) => Ok(arr.last().cloned().unwrap_or(Value::Null)),
            _ => Err(JqError::EvalError("last requires array".into())),
        };
    }

    if expr == "tostring" {
        return Ok(Value::String(match input {
            Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        }));
    }

    if expr == "tonumber" {
        return match input {
            Value::Number(_) => Ok(input.clone()),
            Value::String(s) => {
                if let Ok(n) = s.parse::<i64>() {
                    Ok(serde_json::json!(n))
                } else if let Ok(n) = s.parse::<f64>() {
                    Ok(serde_json::json!(n))
                } else {
                    Err(JqError::EvalError(format!("cannot convert to number: {s}")))
                }
            }
            _ => Err(JqError::EvalError("tonumber requires string or number".into())),
        };
    }

    // select(condition)
    if expr.starts_with("select(") && expr.ends_with(')') {
        let condition = &expr[7..expr.len() - 1];
        let result = eval_value(condition, input)?;
        if is_truthy(&result) {
            return Ok(input.clone());
        } else {
            return Ok(Value::Null);
        }
    }

    // map(expr)
    if expr.starts_with("map(") && expr.ends_with(')') {
        let inner = &expr[4..expr.len() - 1];
        return match input {
            Value::Array(arr) => {
                let results: Result<Vec<Value>, _> = arr.iter()
                    .map(|item| eval_value(inner, item))
                    .collect();
                Ok(Value::Array(results?))
            }
            _ => Err(JqError::EvalError("map requires array input".into())),
        };
    }

    // map_values(expr)
    if expr.starts_with("map_values(") && expr.ends_with(')') {
        let inner = &expr[11..expr.len() - 1];
        return match input {
            Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (k, v) in obj {
                    result.insert(k.clone(), eval_value(inner, v)?);
                }
                Ok(Value::Object(result))
            }
            Value::Array(arr) => {
                let results: Result<Vec<Value>, _> = arr.iter()
                    .map(|item| eval_value(inner, item))
                    .collect();
                Ok(Value::Array(results?))
            }
            _ => Err(JqError::EvalError("map_values requires object or array".into())),
        };
    }

    // add
    if expr == "add" {
        return match input {
            Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok(Value::Null);
                }
                let first = &arr[0];
                match first {
                    Value::Number(_) => {
                        let sum: f64 = arr.iter()
                            .filter_map(|v| v.as_f64())
                            .sum();
                        Ok(serde_json::json!(sum))
                    }
                    Value::String(_) => {
                        let concat: String = arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect();
                        Ok(Value::String(concat))
                    }
                    Value::Array(_) => {
                        let mut result = Vec::new();
                        for item in arr {
                            if let Value::Array(inner) = item {
                                result.extend(inner.iter().cloned());
                            }
                        }
                        Ok(Value::Array(result))
                    }
                    _ => Ok(Value::Null),
                }
            }
            _ => Err(JqError::EvalError("add requires array".into())),
        };
    }

    Err(JqError::CompileError(format!("unsupported jq expression: {expr}")))
}

/// Convenience: evaluate and return as string.
pub fn eval_to_string(expr: &str, input: &str) -> Result<String, JqError> {
    let result = eval(expr, input)?;
    Ok(match result {
        Value::String(s) => s,
        other => serde_json::to_string(&other).unwrap_or_default(),
    })
}

fn resolve_path(root: &Value, path: &str) -> Result<Value, JqError> {
    if path.is_empty() {
        return Ok(root.clone());
    }

    let mut current = root.clone();
    let mut remaining = path;

    while !remaining.is_empty() {
        // Array index: [N]
        if remaining.starts_with('[') {
            if let Some(end) = remaining.find(']') {
                let idx_str = &remaining[1..end];
                remaining = &remaining[end + 1..];
                if remaining.starts_with('.') {
                    remaining = &remaining[1..];
                }
                if idx_str == "" {
                    // .[] — iterate
                    if let Value::Array(arr) = &current {
                        if remaining.is_empty() {
                            return Ok(Value::Array(arr.clone()));
                        }
                        let results: Result<Vec<Value>, _> = arr.iter()
                            .map(|item| resolve_path(item, remaining))
                            .collect();
                        return Ok(Value::Array(results?));
                    }
                    return Err(JqError::EvalError("cannot iterate non-array".into()));
                }
                if let Ok(idx) = idx_str.parse::<usize>() {
                    current = current.get(idx).cloned().unwrap_or(Value::Null);
                } else {
                    return Err(JqError::EvalError(format!("invalid array index: {idx_str}")));
                }
                continue;
            }
        }

        // Field name
        let (field, rest) = match remaining.find(|c: char| c == '.' || c == '[') {
            Some(pos) => {
                let field = &remaining[..pos];
                let rest = if remaining.as_bytes()[pos] == b'.' {
                    &remaining[pos + 1..]
                } else {
                    &remaining[pos..]
                };
                (field, rest)
            }
            None => (remaining, ""),
        };

        if !field.is_empty() {
            current = current.get(field).cloned().unwrap_or(Value::Null);
        }
        remaining = rest;
    }

    Ok(current)
}

fn split_pipe(expr: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    let mut in_string = false;
    let bytes = expr.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if b == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'|' if depth == 0 => {
                return Some((expr[..i].trim(), expr[i + 1..].trim()));
            }
            _ => {}
        }
    }
    None
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        _ => true,
    }
}

fn value_sort_key(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let result = eval(".", r#"{"a": 1}"#).unwrap();
        assert_eq!(result, serde_json::json!({"a": 1}));
    }

    #[test]
    fn test_field_access() {
        let result = eval(".name", r#"{"name": "bashli"}"#).unwrap();
        assert_eq!(result, Value::String("bashli".into()));
    }

    #[test]
    fn test_nested_access() {
        let result = eval(".a.b", r#"{"a": {"b": 42}}"#).unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    #[test]
    fn test_array_index() {
        let result = eval(".[1]", r#"["a", "b", "c"]"#).unwrap();
        assert_eq!(result, Value::String("b".into()));
    }

    #[test]
    fn test_pipe() {
        let result = eval(".a | .b", r#"{"a": {"b": "hello"}}"#).unwrap();
        assert_eq!(result, Value::String("hello".into()));
    }

    #[test]
    fn test_length() {
        let result = eval(". | length", r#"[1, 2, 3]"#).unwrap();
        assert_eq!(result, serde_json::json!(3));
    }

    #[test]
    fn test_keys() {
        let result = eval("keys", r#"{"b": 1, "a": 2}"#).unwrap();
        assert_eq!(result, serde_json::json!(["a", "b"]));
    }

    #[test]
    fn test_map() {
        let result = eval("map(.x)", r#"[{"x": 1}, {"x": 2}]"#).unwrap();
        assert_eq!(result, serde_json::json!([1, 2]));
    }

    #[test]
    fn test_missing_field() {
        let result = eval(".missing", r#"{"a": 1}"#).unwrap();
        assert_eq!(result, Value::Null);
    }
}
