use bashli_core::VarError;
use serde_json::Value;

/// Resolve a dot-notation / bracket-index path against a JSON value.
///
/// Supported syntax:
/// - `field` — object key lookup
/// - `field.nested` — chained object key lookup
/// - `field[0]` — array index
/// - `field[0].name` — mixed
///
/// An empty path returns the root value as-is.
pub fn resolve_path(root: &Value, path: &str) -> Result<Value, VarError> {
    if path.is_empty() {
        return Ok(root.clone());
    }

    let segments = parse_segments(path)?;
    let mut current = root;

    for seg in &segments {
        match seg {
            Segment::Key(key) => match current {
                Value::Object(map) => {
                    current = map.get(key.as_str()).ok_or_else(|| VarError::InvalidPath {
                        path: path.to_string(),
                    })?;
                }
                _ => {
                    return Err(VarError::InvalidPath {
                        path: path.to_string(),
                    });
                }
            },
            Segment::Index(idx) => match current {
                Value::Array(arr) => {
                    current = arr.get(*idx).ok_or(VarError::IndexOutOfBounds { index: *idx })?;
                }
                _ => {
                    return Err(VarError::InvalidPath {
                        path: path.to_string(),
                    });
                }
            },
        }
    }

    Ok(current.clone())
}

#[derive(Debug)]
enum Segment {
    Key(String),
    Index(usize),
}

/// Parse a path string like `foo.bar[2].baz` into segments.
fn parse_segments(path: &str) -> Result<Vec<Segment>, VarError> {
    let mut segments = Vec::new();
    let mut chars = path.chars().peekable();
    let mut buf = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '.' => {
                chars.next();
                if !buf.is_empty() {
                    segments.push(Segment::Key(std::mem::take(&mut buf)));
                }
            }
            '[' => {
                chars.next();
                if !buf.is_empty() {
                    segments.push(Segment::Key(std::mem::take(&mut buf)));
                }
                // Read digits until ']'
                let mut idx_str = String::new();
                loop {
                    match chars.next() {
                        Some(']') => break,
                        Some(d) if d.is_ascii_digit() => idx_str.push(d),
                        _ => {
                            return Err(VarError::InvalidPath {
                                path: path.to_string(),
                            });
                        }
                    }
                }
                let idx: usize = idx_str.parse().map_err(|_| VarError::InvalidPath {
                    path: path.to_string(),
                })?;
                segments.push(Segment::Index(idx));
            }
            _ => {
                chars.next();
                buf.push(ch);
            }
        }
    }

    if !buf.is_empty() {
        segments.push(Segment::Key(buf));
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_path_returns_root() {
        let val = json!({"a": 1});
        assert_eq!(resolve_path(&val, "").unwrap(), val);
    }

    #[test]
    fn simple_key() {
        let val = json!({"name": "alice"});
        assert_eq!(resolve_path(&val, "name").unwrap(), json!("alice"));
    }

    #[test]
    fn nested_key() {
        let val = json!({"a": {"b": {"c": 42}}});
        assert_eq!(resolve_path(&val, "a.b.c").unwrap(), json!(42));
    }

    #[test]
    fn array_index() {
        let val = json!({"items": [10, 20, 30]});
        assert_eq!(resolve_path(&val, "items[1]").unwrap(), json!(20));
    }

    #[test]
    fn mixed_path() {
        let val = json!({"users": [{"name": "bob"}, {"name": "carol"}]});
        assert_eq!(
            resolve_path(&val, "users[1].name").unwrap(),
            json!("carol")
        );
    }

    #[test]
    fn index_out_of_bounds() {
        let val = json!({"arr": [1]});
        let err = resolve_path(&val, "arr[5]").unwrap_err();
        assert!(matches!(err, VarError::IndexOutOfBounds { index: 5 }));
    }

    #[test]
    fn invalid_key_on_non_object() {
        let val = json!(42);
        let err = resolve_path(&val, "foo").unwrap_err();
        assert!(matches!(err, VarError::InvalidPath { .. }));
    }

    #[test]
    fn missing_key() {
        let val = json!({"a": 1});
        let err = resolve_path(&val, "b").unwrap_err();
        assert!(matches!(err, VarError::InvalidPath { .. }));
    }

    #[test]
    fn root_array_index() {
        let val = json!([10, 20, 30]);
        assert_eq!(resolve_path(&val, "[2]").unwrap(), json!(30));
    }
}
