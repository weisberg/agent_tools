use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SedError {
    #[error("invalid sed command: {0}")]
    InvalidCommand(String),
    #[error("regex error: {0}")]
    RegexError(#[from] regex::Error),
}

/// A parsed sed substitution command.
#[derive(Debug)]
struct SubCommand {
    pattern: Regex,
    replacement: String,
    global: bool,
}

/// Parse a sed substitution command of the form `s/pattern/replacement/flags`.
///
/// The delimiter is taken from the first character after `s`. Escaped delimiters
/// (`\/` when `/` is the delimiter) are handled correctly.
fn parse_sub_command(cmd: &str) -> Result<SubCommand, SedError> {
    let cmd = cmd.trim();
    if !cmd.starts_with('s') || cmd.len() < 4 {
        return Err(SedError::InvalidCommand(format!(
            "command must start with 's': {cmd}"
        )));
    }

    let delim = cmd.as_bytes()[1] as char;
    // Split the rest by un-escaped delimiters.
    let body = &cmd[2..]; // everything after "s<delim>"
    let parts = split_on_delimiter(body, delim)?;

    if parts.len() < 2 {
        return Err(SedError::InvalidCommand(format!(
            "expected s{delim}pattern{delim}replacement{delim}[flags]: {cmd}"
        )));
    }

    let raw_pattern = unescape_delimiter(&parts[0], delim);
    let replacement = unescape_delimiter(&parts[1], delim);
    let flags_str = if parts.len() > 2 { &parts[2] } else { "" };

    let mut global = false;
    let mut case_insensitive = false;

    for ch in flags_str.chars() {
        match ch {
            'g' => global = true,
            'i' | 'I' => case_insensitive = true,
            _ => {
                return Err(SedError::InvalidCommand(format!(
                    "unsupported flag '{ch}' in command: {cmd}"
                )));
            }
        }
    }

    let regex_pattern = if case_insensitive {
        format!("(?i){raw_pattern}")
    } else {
        raw_pattern.to_string()
    };

    let pattern = Regex::new(&regex_pattern)?;

    Ok(SubCommand {
        pattern,
        replacement,
        global,
    })
}

/// Split `s` on an un-escaped `delim`, returning at most 3 segments
/// (pattern, replacement, flags).
fn split_on_delimiter(s: &str, delim: char) -> Result<Vec<String>, SedError> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next) = chars.peek() {
                if next == delim {
                    // Escaped delimiter -- keep it (un-escaping happens later for replacement).
                    current.push('\\');
                    current.push(chars.next().unwrap());
                    continue;
                }
            }
            current.push(ch);
        } else if ch == delim {
            parts.push(current);
            current = String::new();
            if parts.len() == 3 {
                // Anything remaining is excess -- ignore.
                break;
            }
        } else {
            current.push(ch);
        }
    }

    // The last segment (flags) may not have a trailing delimiter.
    if parts.len() < 3 {
        parts.push(current);
    }

    Ok(parts)
}

/// Remove backslash-escaping of the delimiter in the replacement string.
fn unescape_delimiter(s: &str, delim: char) -> String {
    let escaped = format!("\\{delim}");
    s.replace(&escaped, &delim.to_string())
}

/// Apply a single parsed substitution to a single line.
fn apply_sub(line: &str, sub: &SubCommand) -> String {
    // Convert sed-style back-references ($1 or \1) in replacement.
    // The `regex` crate uses `$1` style, so convert `\1`..`\9` to `$1`..`$9`.
    let replacement = convert_backrefs(&sub.replacement);

    if sub.global {
        sub.pattern.replace_all(line, replacement.as_str()).into_owned()
    } else {
        sub.pattern.replace(line, replacement.as_str()).into_owned()
    }
}

/// Convert sed-style `\1`..`\9` back-references to regex-crate `$1`..`$9`.
fn convert_backrefs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next) = chars.peek() {
                if next.is_ascii_digit() {
                    out.push('$');
                    out.push(chars.next().unwrap());
                    continue;
                }
            }
            out.push(ch);
        } else {
            out.push(ch);
        }
    }
    out
}

/// Apply one or more sed substitution commands to input text.
/// Commands use standard sed syntax: `s/pattern/replacement/flags`
///
/// Each command is applied line-by-line. Multiple commands are applied
/// sequentially (the output of one command feeds the next).
pub fn apply(input: &str, commands: &[&str]) -> Result<String, SedError> {
    let subs: Vec<SubCommand> = commands
        .iter()
        .map(|c| parse_sub_command(c))
        .collect::<Result<Vec<_>, _>>()?;

    let lines: Vec<&str> = input.lines().collect();
    let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    for sub in &subs {
        for line in result_lines.iter_mut() {
            *line = apply_sub(line, sub);
        }
    }

    // Preserve trailing newline if the input had one.
    let mut output = result_lines.join("\n");
    if input.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

/// Apply a single sed command (convenience wrapper).
pub fn replace(input: &str, command: &str) -> Result<String, SedError> {
    apply(input, &[command])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_replace() {
        let result = replace("hello world", "s/world/rust/").unwrap();
        assert_eq!(result, "hello rust");
    }

    #[test]
    fn test_global_flag() {
        let result = replace("aaa bbb aaa", "s/aaa/ccc/g").unwrap();
        assert_eq!(result, "ccc bbb ccc");
    }

    #[test]
    fn test_without_global_flag() {
        let result = replace("aaa bbb aaa", "s/aaa/ccc/").unwrap();
        assert_eq!(result, "ccc bbb aaa");
    }

    #[test]
    fn test_case_insensitive() {
        let result = replace("Hello World", "s/hello/goodbye/i").unwrap();
        assert_eq!(result, "goodbye World");
    }

    #[test]
    fn test_case_insensitive_global() {
        let result = replace("Hello hello HELLO", "s/hello/bye/gi").unwrap();
        assert_eq!(result, "bye bye bye");
    }

    #[test]
    fn test_multiline() {
        let input = "foo bar\nbaz bar\n";
        let result = replace(input, "s/bar/qux/").unwrap();
        assert_eq!(result, "foo qux\nbaz qux\n");
    }

    #[test]
    fn test_multiple_commands() {
        let result = apply("hello world", &["s/hello/goodbye/", "s/world/rust/"]).unwrap();
        assert_eq!(result, "goodbye rust");
    }

    #[test]
    fn test_regex_pattern() {
        let result = replace("abc 123 def 456", r"s/[0-9]+/NUM/g").unwrap();
        assert_eq!(result, "abc NUM def NUM");
    }

    #[test]
    fn test_escaped_delimiter() {
        // Replace a/b with x (delimiter is /, so a\/b must be used)
        let result = replace("a/b c", r"s/a\/b/x/").unwrap();
        assert_eq!(result, "x c");
    }

    #[test]
    fn test_alternate_delimiter() {
        let result = replace("hello world", "s|world|rust|").unwrap();
        assert_eq!(result, "hello rust");
    }

    #[test]
    fn test_backreference() {
        let result = replace("hello world", r"s/(\w+) (\w+)/\2 \1/").unwrap();
        assert_eq!(result, "world hello");
    }

    #[test]
    fn test_empty_replacement() {
        let result = replace("hello world", "s/world//").unwrap();
        assert_eq!(result, "hello ");
    }

    #[test]
    fn test_invalid_command() {
        let result = replace("hello", "x/a/b/");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_regex() {
        let result = replace("hello", "s/[invalid/replacement/");
        assert!(result.is_err());
    }

    #[test]
    fn test_preserves_trailing_newline() {
        let result = replace("hello\n", "s/hello/bye/").unwrap();
        assert_eq!(result, "bye\n");
    }

    #[test]
    fn test_no_trailing_newline() {
        let result = replace("hello", "s/hello/bye/").unwrap();
        assert_eq!(result, "bye");
    }
}
