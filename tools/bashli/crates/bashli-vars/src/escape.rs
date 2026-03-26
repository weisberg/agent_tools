/// Shell-escape a string value by wrapping it in single quotes.
///
/// Any internal single quotes are escaped using the `'\''` idiom:
/// end the current single-quoted segment, insert a backslash-escaped
/// single quote, then re-open a new single-quoted segment.
pub fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            // end current quote, escaped literal quote, restart quote
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_string() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn empty_string() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn string_with_spaces() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn string_with_single_quote() {
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn string_with_multiple_single_quotes() {
        assert_eq!(shell_escape("'a'b'"), "''\\''a'\\''b'\\'''");
    }

    #[test]
    fn string_with_special_chars() {
        assert_eq!(shell_escape("$HOME && rm -rf /"), "'$HOME && rm -rf /'");
    }

    #[test]
    fn string_with_double_quotes() {
        assert_eq!(shell_escape(r#"say "hi""#), r#"'say "hi"'"#);
    }

    #[test]
    fn string_with_newline() {
        assert_eq!(shell_escape("line1\nline2"), "'line1\nline2'");
    }
}
