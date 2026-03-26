use bashli_core::VarError;
use serde_json::Value;

use crate::escape::shell_escape;
use crate::store::VarStore;

/// Interpolate variable references in a template string.
///
/// Supported syntax:
/// - `$VAR` — simple variable
/// - `${VAR}` — braced variable
/// - `$VAR.field[2].name` — variable with JSON path
/// - `${VAR.field[2].name}` — braced variable with JSON path
/// - `$ENV.PATH` — dotted root (treated as variable `ENV` with path `PATH`)
/// - `$$` — literal `$`
///
/// When `escape` is true, resolved string values are shell-escaped.
pub fn interpolate(template: &str, store: &VarStore, escape: bool) -> Result<String, VarError> {
    let mut out = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '$' {
            if i + 1 < len && chars[i + 1] == '$' {
                // Escaped dollar: $$
                out.push('$');
                i += 2;
            } else if i + 1 < len && chars[i + 1] == '{' {
                // Braced reference: ${...}
                i += 2; // skip ${
                let start = i;
                while i < len && chars[i] != '}' {
                    i += 1;
                }
                if i >= len {
                    return Err(VarError::InterpolationError(
                        "unclosed ${...} in template".to_string(),
                    ));
                }
                let reference: String = chars[start..i].iter().collect();
                i += 1; // skip }
                let value = store.resolve(&reference)?;
                out.push_str(&value_to_string(&value, escape));
            } else if i + 1 < len && is_var_start(chars[i + 1]) {
                // Unbraced reference: $VAR, $VAR.path, $_SYS
                i += 1; // skip $
                let start = i;
                while i < len && is_var_continue(chars[i]) {
                    i += 1;
                }
                let reference: String = chars[start..i].iter().collect();
                let value = store.resolve(&reference)?;
                out.push_str(&value_to_string(&value, escape));
            } else {
                // Lone $ at end or before non-identifier character — keep literal
                out.push('$');
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }

    Ok(out)
}

/// Characters that can start a variable name (after the `$`).
fn is_var_start(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Characters that can continue a variable reference (including path separators).
fn is_var_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']'
}

/// Convert a JSON value to its string representation for interpolation.
fn value_to_string(value: &Value, escape: bool) -> String {
    let s = match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        // For arrays and objects, use compact JSON.
        _ => value.to_string(),
    };
    if escape {
        shell_escape(&s)
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_store() -> VarStore {
        let mut store = VarStore::new();
        store.set("NAME", json!("alice"));
        store.set("COUNT", json!(42));
        store.set("DATA", json!({"items": [{"id": 1}, {"id": 2}]}));
        store.set("ENV", json!({"PATH": "/usr/bin", "HOME": "/home/user"}));
        store.set("_CWD", json!("/tmp"));
        store.set("DANGER", json!("it's dangerous; rm -rf /"));
        store
    }

    #[test]
    fn plain_text_no_vars() {
        let store = make_store();
        assert_eq!(interpolate("hello world", &store, false).unwrap(), "hello world");
    }

    #[test]
    fn simple_var() {
        let store = make_store();
        assert_eq!(
            interpolate("hi $NAME!", &store, false).unwrap(),
            "hi alice!"
        );
    }

    #[test]
    fn braced_var() {
        let store = make_store();
        assert_eq!(
            interpolate("hi ${NAME}!", &store, false).unwrap(),
            "hi alice!"
        );
    }

    #[test]
    fn numeric_var() {
        let store = make_store();
        assert_eq!(
            interpolate("count=$COUNT", &store, false).unwrap(),
            "count=42"
        );
    }

    #[test]
    fn escaped_dollar() {
        let store = make_store();
        assert_eq!(
            interpolate("price: $$5", &store, false).unwrap(),
            "price: $5"
        );
    }

    #[test]
    fn dotted_path() {
        let store = make_store();
        assert_eq!(
            interpolate("path=$ENV.PATH", &store, false).unwrap(),
            "path=/usr/bin"
        );
    }

    #[test]
    fn complex_path() {
        let store = make_store();
        assert_eq!(
            interpolate("id=${DATA.items[1].id}", &store, false).unwrap(),
            "id=2"
        );
    }

    #[test]
    fn system_var() {
        let store = make_store();
        assert_eq!(
            interpolate("cwd=$_CWD", &store, false).unwrap(),
            "cwd=/tmp"
        );
    }

    #[test]
    fn undefined_var_error() {
        let store = make_store();
        let err = interpolate("$NOPE", &store, false).unwrap_err();
        assert!(matches!(err, VarError::Undefined(_)));
    }

    #[test]
    fn unclosed_brace_error() {
        let store = make_store();
        let err = interpolate("${UNCLOSED", &store, false).unwrap_err();
        assert!(matches!(err, VarError::InterpolationError(_)));
    }

    #[test]
    fn escape_mode() {
        let store = make_store();
        let result = interpolate("cmd $DANGER", &store, true).unwrap();
        assert_eq!(result, "cmd 'it'\\''s dangerous; rm -rf /'");
    }

    #[test]
    fn multiple_vars() {
        let store = make_store();
        assert_eq!(
            interpolate("$NAME has $COUNT items", &store, false).unwrap(),
            "alice has 42 items"
        );
    }

    #[test]
    fn lone_dollar_at_end() {
        let store = make_store();
        assert_eq!(interpolate("cost$", &store, false).unwrap(), "cost$");
    }

    #[test]
    fn dollar_before_space() {
        let store = make_store();
        assert_eq!(interpolate("$ foo", &store, false).unwrap(), "$ foo");
    }

    #[test]
    fn braced_with_path() {
        let store = make_store();
        assert_eq!(
            interpolate("home=${ENV.HOME}", &store, false).unwrap(),
            "home=/home/user"
        );
    }
}
