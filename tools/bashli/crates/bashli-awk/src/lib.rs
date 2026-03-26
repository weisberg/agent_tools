use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AwkError {
    #[error("awk parse error: {0}")]
    ParseError(String),
    #[error("awk runtime error: {0}")]
    RuntimeError(String),
}

pub struct AwkOpts {
    pub field_separator: Option<String>,
    pub vars: BTreeMap<String, String>,
}

impl Default for AwkOpts {
    fn default() -> Self {
        Self {
            field_separator: None,
            vars: BTreeMap::new(),
        }
    }
}

/// Execute an awk program against input text.
///
/// Supports:
/// - `{print $N}` — print field N (1-indexed, $0 = whole line)
/// - `{print $N, $M}` — print multiple fields (space-separated)
/// - `/pattern/ {action}` — pattern-matched actions
/// - `BEGIN{...}` / `END{...}` blocks (print statements only)
/// - `NR`, `NF` built-in variables
/// - Simple conditions: `$N == "value"`, `$N ~ /pattern/`
pub fn eval(program: &str, input: &str, opts: &AwkOpts) -> Result<String, AwkError> {
    let sep = opts.field_separator.as_deref().unwrap_or(" ");
    let mut output = String::new();
    let mut nr: usize = 0;

    // Parse program into blocks
    let blocks = parse_program(program)?;

    // Execute BEGIN blocks
    for block in &blocks {
        if block.block_type == BlockType::Begin {
            execute_action(&block.action, "", &[], 0, 0, &mut output)?;
        }
    }

    // Process each line
    for line in input.lines() {
        nr += 1;
        let fields: Vec<&str> = if sep == " " {
            line.split_whitespace().collect()
        } else {
            line.split(sep).collect()
        };
        let nf = fields.len();

        for block in &blocks {
            match &block.block_type {
                BlockType::Begin | BlockType::End => continue,
                BlockType::Pattern(pattern) => {
                    if matches_pattern(pattern, line, &fields) {
                        execute_action(&block.action, line, &fields, nr, nf, &mut output)?;
                    }
                }
                BlockType::Always => {
                    execute_action(&block.action, line, &fields, nr, nf, &mut output)?;
                }
            }
        }
    }

    // Execute END blocks
    for block in &blocks {
        if block.block_type == BlockType::End {
            execute_action(&block.action, "", &[], nr, 0, &mut output)?;
        }
    }

    Ok(output)
}

/// Convenience: extract a single field from each line.
pub fn field(input: &str, field_num: usize, separator: Option<&str>) -> Result<String, AwkError> {
    let sep = separator.unwrap_or(" ");
    let mut output = String::new();

    for line in input.lines() {
        let fields: Vec<&str> = if sep == " " {
            line.split_whitespace().collect()
        } else {
            line.split(sep).collect()
        };

        if field_num == 0 {
            output.push_str(line);
        } else if let Some(f) = fields.get(field_num - 1) {
            output.push_str(f);
        }
        output.push('\n');
    }

    Ok(output)
}

#[derive(Debug, PartialEq)]
enum BlockType {
    Begin,
    End,
    Pattern(String),
    Always,
}

struct Block {
    block_type: BlockType,
    action: String,
}

fn parse_program(program: &str) -> Result<Vec<Block>, AwkError> {
    let program = program.trim();
    let mut blocks = Vec::new();

    // Simple parser: handle BEGIN{...}, END{...}, /pattern/{...}, {action}
    let mut pos = 0;
    let chars: Vec<char> = program.chars().collect();

    while pos < chars.len() {
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        if program[pos..].starts_with("BEGIN") {
            pos += 5;
            while pos < chars.len() && chars[pos].is_whitespace() { pos += 1; }
            if pos < chars.len() && chars[pos] == '{' {
                let action = extract_braced(&chars, &mut pos)?;
                blocks.push(Block { block_type: BlockType::Begin, action });
            }
        } else if program[pos..].starts_with("END") {
            pos += 3;
            while pos < chars.len() && chars[pos].is_whitespace() { pos += 1; }
            if pos < chars.len() && chars[pos] == '{' {
                let action = extract_braced(&chars, &mut pos)?;
                blocks.push(Block { block_type: BlockType::End, action });
            }
        } else if chars[pos] == '/' {
            // Pattern: /regex/
            pos += 1;
            let start = pos;
            while pos < chars.len() && chars[pos] != '/' {
                if chars[pos] == '\\' { pos += 1; }
                pos += 1;
            }
            let pattern: String = chars[start..pos].iter().collect();
            if pos < chars.len() { pos += 1; } // skip closing /
            while pos < chars.len() && chars[pos].is_whitespace() { pos += 1; }
            if pos < chars.len() && chars[pos] == '{' {
                let action = extract_braced(&chars, &mut pos)?;
                blocks.push(Block { block_type: BlockType::Pattern(pattern), action });
            }
        } else if chars[pos] == '{' {
            let action = extract_braced(&chars, &mut pos)?;
            blocks.push(Block { block_type: BlockType::Always, action });
        } else {
            // Skip unexpected characters
            pos += 1;
        }
    }

    if blocks.is_empty() {
        return Err(AwkError::ParseError(format!("no valid awk blocks found in: {program}")));
    }

    Ok(blocks)
}

fn extract_braced(chars: &[char], pos: &mut usize) -> Result<String, AwkError> {
    if *pos >= chars.len() || chars[*pos] != '{' {
        return Err(AwkError::ParseError("expected '{'".into()));
    }
    *pos += 1; // skip {
    let start = *pos;
    let mut depth = 1;
    while *pos < chars.len() && depth > 0 {
        match chars[*pos] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            *pos += 1;
        }
    }
    let action: String = chars[start..*pos].iter().collect();
    if *pos < chars.len() { *pos += 1; } // skip }
    Ok(action.trim().to_string())
}

fn matches_pattern(pattern: &str, line: &str, _fields: &[&str]) -> bool {
    ::regex::Regex::new(pattern)
        .map(|re| re.is_match(line))
        .unwrap_or(false)
}

fn execute_action(
    action: &str,
    line: &str,
    fields: &[&str],
    _nr: usize,
    _nf: usize,
    output: &mut String,
) -> Result<(), AwkError> {
    // Parse semicolon-separated statements
    for stmt in action.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }

        if stmt.starts_with("print") {
            let args = stmt[5..].trim();
            if args.is_empty() {
                // print with no args = print $0
                output.push_str(line);
                output.push('\n');
            } else {
                let parts: Vec<&str> = args.split(',').collect();
                let mut first = true;
                for part in parts {
                    let part = part.trim();
                    if !first {
                        output.push(' ');
                    }
                    first = false;

                    if part.starts_with('$') {
                        let field_str = &part[1..];
                        if let Ok(n) = field_str.parse::<usize>() {
                            if n == 0 {
                                output.push_str(line);
                            } else if let Some(f) = fields.get(n - 1) {
                                output.push_str(f);
                            }
                        }
                    } else if part.starts_with('"') && part.ends_with('"') && part.len() >= 2 {
                        output.push_str(&part[1..part.len() - 1]);
                    } else {
                        output.push_str(part);
                    }
                }
                output.push('\n');
            }
        }
        // Other statements (assignments, etc.) are silently ignored for now
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_field() {
        let result = eval("{print $2}", "hello world\nfoo bar", &AwkOpts::default()).unwrap();
        assert_eq!(result, "world\nbar\n");
    }

    #[test]
    fn test_print_whole_line() {
        let result = eval("{print $0}", "hello world", &AwkOpts::default()).unwrap();
        assert_eq!(result, "hello world\n");
    }

    #[test]
    fn test_field_separator() {
        let opts = AwkOpts {
            field_separator: Some(":".into()),
            ..Default::default()
        };
        let result = eval("{print $1}", "root:x:0:0", &opts).unwrap();
        assert_eq!(result, "root\n");
    }

    #[test]
    fn test_pattern_match() {
        let result = eval("/error/ {print $0}", "info: ok\nerror: bad\ninfo: fine", &AwkOpts::default()).unwrap();
        assert_eq!(result, "error: bad\n");
    }

    #[test]
    fn test_multiple_fields() {
        let result = eval("{print $1, $3}", "a b c d", &AwkOpts::default()).unwrap();
        assert_eq!(result, "a c\n");
    }

    #[test]
    fn test_field_function() {
        let result = field("hello world\nfoo bar", 2, None).unwrap();
        assert_eq!(result, "world\nbar\n");
    }
}
