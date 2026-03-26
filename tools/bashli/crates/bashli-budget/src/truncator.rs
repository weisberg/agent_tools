/// Estimate the number of tokens in a string (~4 chars per token).
pub fn estimate_tokens(text: &str) -> usize {
    // Rough heuristic: 1 token per 4 characters, minimum 1 if non-empty
    let len = text.len();
    if len == 0 {
        0
    } else {
        (len + 3) / 4 // ceiling division
    }
}

/// Keep the first `max_lines / 2` and last `max_lines / 2` lines, inserting a
/// truncation marker in between. Returns `(truncated_output, lines_dropped)`.
///
/// If the input has fewer than or equal to `max_lines`, it is returned as-is.
pub fn smart_truncate(input: &str, max_lines: usize) -> (String, usize) {
    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();

    if total <= max_lines || max_lines == 0 {
        return (input.to_string(), 0);
    }

    let head_count = max_lines / 2;
    let tail_count = max_lines - head_count;
    let dropped = total - head_count - tail_count;

    let mut result = String::new();
    for line in &lines[..head_count] {
        result.push_str(line);
        result.push('\n');
    }
    result.push_str(&format!("... [truncated {} lines] ...\n", dropped));
    for (i, line) in lines[total - tail_count..].iter().enumerate() {
        result.push_str(line);
        if i < tail_count - 1 {
            result.push('\n');
        }
    }

    (result, dropped)
}

/// Keep the first `max_lines` lines. Returns `(truncated_output, lines_dropped)`.
pub fn head_truncate(input: &str, max_lines: usize) -> (String, usize) {
    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();

    if total <= max_lines {
        return (input.to_string(), 0);
    }

    let dropped = total - max_lines;
    let kept: String = lines[..max_lines].join("\n");

    (kept, dropped)
}

/// Keep the last `max_lines` lines. Returns `(truncated_output, lines_dropped)`.
pub fn tail_truncate(input: &str, max_lines: usize) -> (String, usize) {
    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();

    if total <= max_lines {
        return (input.to_string(), 0);
    }

    let dropped = total - max_lines;
    let kept: String = lines[dropped..].join("\n");

    (kept, dropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        // 5 chars -> ceil(5/4) = 2
        assert_eq!(estimate_tokens("hello"), 2);
    }

    #[test]
    fn test_estimate_tokens_exact_multiple() {
        // 8 chars -> 2 tokens
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn test_estimate_tokens_longer() {
        // 100 chars -> 25 tokens
        let text = "a".repeat(100);
        assert_eq!(estimate_tokens(&text), 25);
    }

    #[test]
    fn test_smart_truncate_no_truncation_needed() {
        let input = "line1\nline2\nline3";
        let (output, dropped) = smart_truncate(input, 5);
        assert_eq!(output, input);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn test_smart_truncate_basic() {
        let input = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10";
        let (output, dropped) = smart_truncate(input, 4);
        // head_count = 2, tail_count = 2, dropped = 6
        assert_eq!(dropped, 6);
        assert!(output.contains("line1\n"));
        assert!(output.contains("line2\n"));
        assert!(output.contains("... [truncated 6 lines] ..."));
        assert!(output.contains("line9"));
        assert!(output.contains("line10"));
    }

    #[test]
    fn test_smart_truncate_odd_max() {
        let input = "a\nb\nc\nd\ne\nf\ng";
        let (output, dropped) = smart_truncate(input, 3);
        // head_count = 1, tail_count = 2, dropped = 4
        assert_eq!(dropped, 4);
        assert!(output.starts_with("a\n"));
        assert!(output.contains("... [truncated 4 lines] ..."));
        assert!(output.contains("f\ng"));
    }

    #[test]
    fn test_head_truncate_no_truncation() {
        let input = "line1\nline2";
        let (output, dropped) = head_truncate(input, 5);
        assert_eq!(output, input);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn test_head_truncate_basic() {
        let input = "line1\nline2\nline3\nline4\nline5";
        let (output, dropped) = head_truncate(input, 2);
        assert_eq!(output, "line1\nline2");
        assert_eq!(dropped, 3);
    }

    #[test]
    fn test_tail_truncate_no_truncation() {
        let input = "line1\nline2";
        let (output, dropped) = tail_truncate(input, 5);
        assert_eq!(output, input);
        assert_eq!(dropped, 0);
    }

    #[test]
    fn test_tail_truncate_basic() {
        let input = "line1\nline2\nline3\nline4\nline5";
        let (output, dropped) = tail_truncate(input, 2);
        assert_eq!(output, "line4\nline5");
        assert_eq!(dropped, 3);
    }

    #[test]
    fn test_smart_truncate_zero_max() {
        let input = "line1\nline2\nline3";
        let (output, dropped) = smart_truncate(input, 0);
        assert_eq!(output, input);
        assert_eq!(dropped, 0);
    }
}
