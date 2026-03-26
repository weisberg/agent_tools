// Templatizer — extracts Jinja2 variables from literal HTML content.
// See CLIPLI_SPEC.md §5.4 for full specification.

use crate::model::{TemplateVariable, VarType};
use regex::Regex;
use serde::Deserialize;
use std::io::{BufRead, Write};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of the templatization process.
#[derive(Debug, Clone)]
pub struct TemplatizeResult {
    /// The HTML with dynamic values replaced by Jinja2 `{{ var_name }}` placeholders.
    pub template_html: String,
    /// The extracted variables with inferred types and default values.
    pub variables: Vec<TemplateVariable>,
}

#[derive(Debug, thiserror::Error)]
pub enum TemplatizeError {
    #[error("agent did not respond in time")]
    AgentTimeout,
    #[error("agent response could not be parsed: {0}")]
    InvalidResponse(String),
    #[error("agent returned invalid Jinja2 template: {0}")]
    InvalidTemplate(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// The strategy to use for variable extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Strategy {
    /// Rule-based extraction (fast, deterministic).
    Heuristic,
    /// Pipe to external agent via stdout/stdin JSON protocol.
    Agent,
    /// No extraction — save HTML as-is for manual editing.
    Manual,
}

// ---------------------------------------------------------------------------
// HTML segment splitting
// ---------------------------------------------------------------------------

enum HtmlSegment {
    Tag(String),
    Text(String),
}

/// Split HTML into alternating Text / Tag segments.
/// The result starts with a Text segment (possibly empty), then alternates
/// Tag / Text.  Indices: even = Text, odd = Tag.
fn split_html_into_segments(html: &str) -> Vec<HtmlSegment> {
    // Matches HTML tags including self-closing and comments.
    let tag_re = Regex::new(r"<[^>]*>").expect("static regex");
    let mut segments: Vec<HtmlSegment> = Vec::new();
    let mut last_end = 0usize;

    for m in tag_re.find_iter(html) {
        // Text before this tag
        segments.push(HtmlSegment::Text(html[last_end..m.start()].to_owned()));
        // The tag itself
        segments.push(HtmlSegment::Tag(html[m.start()..m.end()].to_owned()));
        last_end = m.end();
    }
    // Trailing text after last tag
    segments.push(HtmlSegment::Text(html[last_end..].to_owned()));
    segments
}

/// Return true if `tag_str` is an opening `<td` or `<th` tag.
fn is_cell_open_tag(tag_str: &str) -> bool {
    let lower = tag_str.to_lowercase();
    let t = lower.trim_start_matches('<').trim();
    t.starts_with("td") || t.starts_with("th")
}

// ---------------------------------------------------------------------------
// Heuristic strategy
// ---------------------------------------------------------------------------

/// A half-open byte range `[start, end)` that has already been replaced
/// in the current text segment.  Used to avoid overlapping replacements.
#[derive(Debug)]
struct ReplacedSpan {
    start: usize,
    end: usize,
}

impl ReplacedSpan {
    fn overlaps(&self, start: usize, end: usize) -> bool {
        start < self.end && end > self.start
    }
}

struct HeuristicState {
    variables: Vec<TemplateVariable>,
    date_counter: u32,
    currency_counter: u32,
    pct_counter: u32,
    email_counter: u32,
    number_counter: u32,
    quarter_counter: u32,
    field_counter: u32,
}

impl HeuristicState {
    fn new() -> Self {
        Self {
            variables: Vec::new(),
            date_counter: 0,
            currency_counter: 0,
            pct_counter: 0,
            email_counter: 0,
            number_counter: 0,
            quarter_counter: 0,
            field_counter: 0,
        }
    }

    fn next_date(&mut self) -> String {
        self.date_counter += 1;
        format!("date_{}", self.date_counter)
    }
    fn next_currency(&mut self) -> String {
        self.currency_counter += 1;
        format!("currency_{}", self.currency_counter)
    }
    fn next_pct(&mut self) -> String {
        self.pct_counter += 1;
        format!("pct_{}", self.pct_counter)
    }
    fn next_email(&mut self) -> String {
        self.email_counter += 1;
        format!("email_{}", self.email_counter)
    }
    fn next_number(&mut self) -> String {
        self.number_counter += 1;
        format!("number_{}", self.number_counter)
    }
    fn next_quarter(&mut self) -> String {
        self.quarter_counter += 1;
        format!("quarter_{}", self.quarter_counter)
    }
    fn next_field(&mut self) -> String {
        self.field_counter += 1;
        format!("field_{}", self.field_counter)
    }

    fn add_var(&mut self, name: &str, var_type: VarType, original: &str) {
        self.variables.push(TemplateVariable {
            name: name.to_owned(),
            var_type,
            default_value: Some(serde_json::Value::String(original.to_owned())),
            description: None,
        });
    }
}

// Structural labels to skip in Pass 7.
const STRUCTURAL_LABELS: &[&str] = &[
    "Total", "Name", "Date", "Amount", "Value", "Count", "Type", "Status", "ID",
    "No.", "#", "N/A", "-", "Yes", "No",
];

/// Apply all detection passes to a single text segment, returning the
/// rewritten text and appending discovered variables to `state`.
fn process_text_segment(text: &str, state: &mut HeuristicState, in_cell: bool) -> String {
    if text.is_empty() {
        return text.to_owned();
    }

    // We accumulate (start, end, replacement_name) for all matched spans,
    // then do a single left-to-right substitution pass.  Spans must not overlap.
    // We collect them per-pass in order, checking against already-claimed spans.

    let mut claimed: Vec<ReplacedSpan> = Vec::new();
    // (byte_start, byte_end, var_name, var_type, original)
    let mut replacements: Vec<(usize, usize, String, VarType, String)> = Vec::new();

    // Helper: try to claim [start, end).  Returns false if overlaps.
    let try_claim = |claimed: &mut Vec<ReplacedSpan>, start: usize, end: usize| -> bool {
        if claimed.iter().any(|s| s.overlaps(start, end)) {
            return false;
        }
        claimed.push(ReplacedSpan { start, end });
        true
    };

    // --- Pass 1: Dates ---
    {
        let iso_re = Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap();
        let us_re = Regex::new(r"\b\d{1,2}/\d{1,2}/\d{2,4}\b").unwrap();
        let written_re = Regex::new(
            r"\b(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\.?\s+\d{1,2},?\s+\d{4}\b",
        )
        .unwrap();

        for re in &[&iso_re, &us_re, &written_re] {
            for m in re.find_iter(text) {
                if try_claim(&mut claimed, m.start(), m.end()) {
                    let name = state.next_date();
                    replacements.push((m.start(), m.end(), name, VarType::Date, m.as_str().to_owned()));
                }
            }
        }
    }

    // --- Pass 2: Currency ---
    {
        let currency_re = Regex::new(r"[\$€£][\d,]+(?:\.\d{2})?").unwrap();
        for m in currency_re.find_iter(text) {
            if try_claim(&mut claimed, m.start(), m.end()) {
                let name = state.next_currency();
                replacements.push((m.start(), m.end(), name, VarType::Currency, m.as_str().to_owned()));
            }
        }
    }

    // --- Pass 3: Percentages ---
    {
        let pct_re = Regex::new(r"\b\d+(?:\.\d+)?%").unwrap();
        for m in pct_re.find_iter(text) {
            if try_claim(&mut claimed, m.start(), m.end()) {
                let name = state.next_pct();
                replacements.push((m.start(), m.end(), name, VarType::Percentage, m.as_str().to_owned()));
            }
        }
    }

    // --- Pass 4: Email addresses ---
    {
        let email_re =
            Regex::new(r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b").unwrap();
        for m in email_re.find_iter(text) {
            if try_claim(&mut claimed, m.start(), m.end()) {
                let name = state.next_email();
                replacements.push((m.start(), m.end(), name, VarType::String, m.as_str().to_owned()));
            }
        }
    }

    // --- Pass 5: Large numbers (comma-separated, 4+ digits) ---
    {
        let num_re = Regex::new(r"\b\d{1,3}(?:,\d{3})+\b").unwrap();
        for m in num_re.find_iter(text) {
            if try_claim(&mut claimed, m.start(), m.end()) {
                let name = state.next_number();
                replacements.push((m.start(), m.end(), name, VarType::Number, m.as_str().to_owned()));
            }
        }
    }

    // --- Pass 6: Quarters ---
    {
        let q_re = Regex::new(r"\bQ[1-4]\s*\d{4}\b").unwrap();
        for m in q_re.find_iter(text) {
            if try_claim(&mut claimed, m.start(), m.end()) {
                let name = state.next_quarter();
                replacements.push((m.start(), m.end(), name, VarType::String, m.as_str().to_owned()));
            }
        }
    }

    // --- Pass 7: Remaining cell text ---
    if in_cell && claimed.is_empty() {
        let trimmed = text.trim();
        // Skip short or purely structural labels
        if trimmed.chars().count() > 2
            && !STRUCTURAL_LABELS
                .iter()
                .any(|&label| label.eq_ignore_ascii_case(trimmed))
        {
            // Replace the full trimmed span within the text
            if let Some(start) = text.find(trimmed) {
                let end = start + trimmed.len();
                if try_claim(&mut claimed, start, end) {
                    let name = state.next_field();
                    replacements.push((
                        start,
                        end,
                        name,
                        VarType::String,
                        trimmed.to_owned(),
                    ));
                }
            }
        }
    }

    // Register vars (in order of start position for deterministic naming)
    replacements.sort_by_key(|(start, _, _, _, _)| *start);

    // Record variable metadata
    for (_, _, ref name, ref vt, ref original) in &replacements {
        state.add_var(name, vt.clone(), original);
    }

    if replacements.is_empty() {
        return text.to_owned();
    }

    // Build the output string by walking the replacements in order.
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start, end, name, _, _) in &replacements {
        if *start > cursor {
            output.push_str(&text[cursor..*start]);
        }
        output.push_str(&format!("{{{{ {} }}}}", name));
        cursor = *end;
    }
    if cursor < text.len() {
        output.push_str(&text[cursor..]);
    }
    output
}

/// Rule-based extraction: fast, deterministic, no external calls.
pub fn heuristic(html: &str) -> TemplatizeResult {
    let segments = split_html_into_segments(html);
    let mut state = HeuristicState::new();
    let mut output = String::with_capacity(html.len());

    // Track whether the immediately preceding tag was a <td> or <th> opener.
    let mut in_cell = false;

    for segment in &segments {
        match segment {
            HtmlSegment::Tag(tag) => {
                in_cell = is_cell_open_tag(tag);
                output.push_str(tag);
            }
            HtmlSegment::Text(text) => {
                let replaced = process_text_segment(text, &mut state, in_cell);
                // After emitting text, we're no longer "directly after" a cell tag.
                in_cell = false;
                output.push_str(&replaced);
            }
        }
    }

    TemplatizeResult {
        template_html: output,
        variables: state.variables,
    }
}

// ---------------------------------------------------------------------------
// Agent strategy
// ---------------------------------------------------------------------------

/// Agent response types for stdin parsing.
#[derive(Debug, Deserialize)]
struct AgentResponse {
    template: String,
    variables: Vec<AgentVariable>,
}

#[derive(Debug, Deserialize)]
struct AgentVariable {
    name: String,
    #[serde(rename = "type")]
    var_type: String,
    default_value: Option<serde_json::Value>,
    description: Option<String>,
}

fn map_var_type(s: &str) -> VarType {
    match s.to_lowercase().as_str() {
        "string" | "text" => VarType::String,
        "number" | "integer" => VarType::Number,
        "currency" | "money" => VarType::Currency,
        "percentage" | "percent" => VarType::Percentage,
        "date" => VarType::Date,
        "boolean" | "bool" => VarType::Boolean,
        "list" | "array" => VarType::List,
        _ => VarType::String,
    }
}

fn validate_template(template: &str) -> Result<(), TemplatizeError> {
    // Basic check: count {{ and }} — they must balance.
    let open_count = template.matches("{{").count();
    let close_count = template.matches("}}").count();
    if open_count != close_count {
        return Err(TemplatizeError::InvalidTemplate(format!(
            "unbalanced Jinja2 delimiters: {} opening '{{{{' vs {} closing '}}}}'",
            open_count, close_count
        )));
    }
    Ok(())
}

fn validate_var_name(name: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap();
    re.is_match(name)
}

/// Pipe to external agent via stdout/stdin JSON protocol.
pub fn agent(html: &str, source_app: Option<&str>) -> Result<TemplatizeResult, TemplatizeError> {
    let app_name = source_app.unwrap_or("the source application");
    let prompt = format!(
        "Identify dynamic content in this HTML captured from {}. \
         Replace dynamic values with Jinja2 variables using descriptive names. \
         Keep all inline CSS intact. \
         Return JSON with keys: template (the templatized HTML string), \
         variables (array of {{name, type, default_value, description}}).",
        app_name
    );

    let payload = serde_json::json!({
        "action": "templatize",
        "html": html,
        "prompt": prompt,
    });

    // Write JSON payload to stdout (agent protocol).
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{}", serde_json::to_string(&payload).map_err(|e| {
        TemplatizeError::InvalidResponse(format!("failed to serialize payload: {}", e))
    })?)?;
    out.flush()?;
    drop(out);

    // Read one line of JSON from stdin.
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|_| TemplatizeError::AgentTimeout)?;

    if line.trim().is_empty() {
        return Err(TemplatizeError::AgentTimeout);
    }

    // Parse the response.
    let resp: AgentResponse = serde_json::from_str(line.trim()).map_err(|e| {
        TemplatizeError::InvalidResponse(format!("JSON parse error: {}", e))
    })?;

    // Validate template.
    validate_template(&resp.template)?;

    // Validate and convert variables.
    let mut variables: Vec<TemplateVariable> = Vec::with_capacity(resp.variables.len());
    for av in resp.variables {
        if !validate_var_name(&av.name) {
            return Err(TemplatizeError::InvalidResponse(format!(
                "invalid variable name: {:?}",
                av.name
            )));
        }
        variables.push(TemplateVariable {
            name: av.name,
            var_type: map_var_type(&av.var_type),
            default_value: av.default_value,
            description: av.description,
        });
    }

    Ok(TemplatizeResult {
        template_html: resp.template,
        variables,
    })
}

// ---------------------------------------------------------------------------
// Manual strategy
// ---------------------------------------------------------------------------

/// No extraction — save HTML as-is for manual editing.
pub fn manual(html: &str) -> TemplatizeResult {
    TemplatizeResult {
        template_html: html.to_owned(),
        variables: vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Pass 2: Currency ---

    #[test]
    fn detects_currency() {
        let html = "<td>$4,200,000</td>";
        let result = heuristic(html);
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::Currency));
        assert!(result.template_html.contains("{{ currency_1 }}"));
        assert_eq!(
            result.variables[0].default_value,
            Some(serde_json::Value::String("$4,200,000".to_string()))
        );
    }

    // --- Pass 1: ISO date ---

    #[test]
    fn detects_iso_date() {
        let html = "<td>2024-03-15</td>";
        let result = heuristic(html);
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::Date));
        assert!(result.template_html.contains("{{ date_1 }}"));
        assert_eq!(
            result.variables[0].default_value,
            Some(serde_json::Value::String("2024-03-15".to_string()))
        );
    }

    // --- Pass 3: Percentage ---

    #[test]
    fn detects_percentage() {
        let html = "<p>Growth was 12.5% this quarter.</p>";
        let result = heuristic(html);
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::Percentage));
        assert!(result.template_html.contains("{{ pct_1 }}"));
    }

    // --- Pass 6: Quarter ---

    #[test]
    fn detects_quarter() {
        let html = "<td>Q3 2024</td>";
        let result = heuristic(html);
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::String));
        assert!(result.template_html.contains("{{ quarter_1 }}"));
        assert_eq!(
            result.variables[0].default_value,
            Some(serde_json::Value::String("Q3 2024".to_string()))
        );
    }

    // --- Round-trip: default values match originals ---

    #[test]
    fn round_trip_default_values() {
        let html = "<td>$1,234</td><td>15%</td>";
        let result = heuristic(html);
        // All variables must have a default value.
        for var in &result.variables {
            assert!(var.default_value.is_some());
        }
        // After rendering with defaults, originals are restored.
        // We can't call the renderer here, so verify structural properties.
        assert!(
            !result.template_html.contains("$1,234")
                || result.variables.iter().any(|v| v.default_value
                    == Some(serde_json::json!("$1,234")))
        );
    }

    // --- No replacements inside HTML tags/attributes ---

    #[test]
    fn doesnt_replace_in_tags() {
        let html = r#"<a href="2024-01-01.html">January 1, 2024</a>"#;
        let result = heuristic(html);
        // href attribute must be untouched.
        assert!(result.template_html.contains(r#"href="2024-01-01.html""#));
        // The visible text (written month date) should be templatized.
        assert_eq!(result.variables.len(), 1);
    }

    // --- Manual strategy: pass-through ---

    #[test]
    fn manual_passthrough() {
        let html = "<p>Hello $100</p>";
        let result = manual(html);
        assert_eq!(result.template_html, html);
        assert!(result.variables.is_empty());
    }

    // --- Variable names are unique across multiple detections ---

    #[test]
    fn unique_variable_names() {
        let html = "<td>$100</td><td>$200</td><td>$300</td>";
        let result = heuristic(html);
        let names: Vec<&str> = result.variables.iter().map(|v| v.name.as_str()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "variable names must be unique");
    }

    // --- Currency does not fire inside attribute values ---

    #[test]
    fn currency_not_in_attribute() {
        let html = r#"<td data-value="$500">$500</td>"#;
        let result = heuristic(html);
        // Only the text node value should be replaced; attribute untouched.
        assert!(result.template_html.contains(r#"data-value="$500""#));
        assert_eq!(result.variables.len(), 1);
    }

    // --- Email detection ---

    #[test]
    fn detects_email() {
        let html = "<td>user@example.com</td>";
        let result = heuristic(html);
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::String));
        assert!(result.template_html.contains("{{ email_1 }}"));
    }

    // --- Large number detection ---

    #[test]
    fn detects_large_number() {
        let html = "<td>1,234,567</td>";
        let result = heuristic(html);
        // Could be currency_1 if preceded by symbol, but here it is a bare number.
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::Number));
        assert!(result.template_html.contains("{{ number_1 }}"));
    }

    // --- Written-month date ---

    #[test]
    fn detects_written_month_date() {
        let html = "<p>Published on March 15, 2024.</p>";
        let result = heuristic(html);
        assert_eq!(result.variables.len(), 1);
        assert!(matches!(result.variables[0].var_type, VarType::Date));
        assert!(result.template_html.contains("{{ date_1 }}"));
    }

    // --- Pass 7: field for plain cell text ---

    #[test]
    fn detects_cell_field() {
        let html = "<table><tr><td>Acme Corporation</td></tr></table>";
        let result = heuristic(html);
        // "Acme Corporation" is long, non-structural — should become field_1.
        assert_eq!(result.variables.len(), 1);
        assert!(result.template_html.contains("{{ field_1 }}"));
    }

    // --- Structural labels are NOT replaced ---

    #[test]
    fn skips_structural_label() {
        let html = "<td>Total</td>";
        let result = heuristic(html);
        assert!(result.variables.is_empty());
        assert!(result.template_html.contains("Total"));
    }

    // --- validate_template ---

    #[test]
    fn validate_template_balanced() {
        assert!(validate_template("hello {{ name }} world").is_ok());
        assert!(validate_template("{{ a }} {{ b }}").is_ok());
    }

    #[test]
    fn validate_template_unbalanced() {
        assert!(validate_template("{{ name }").is_err());
        assert!(validate_template("name }}").is_err());
    }

    // --- map_var_type ---

    #[test]
    fn map_var_type_all_variants() {
        assert!(matches!(map_var_type("string"), VarType::String));
        assert!(matches!(map_var_type("text"), VarType::String));
        assert!(matches!(map_var_type("number"), VarType::Number));
        assert!(matches!(map_var_type("integer"), VarType::Number));
        assert!(matches!(map_var_type("currency"), VarType::Currency));
        assert!(matches!(map_var_type("money"), VarType::Currency));
        assert!(matches!(map_var_type("percentage"), VarType::Percentage));
        assert!(matches!(map_var_type("percent"), VarType::Percentage));
        assert!(matches!(map_var_type("date"), VarType::Date));
        assert!(matches!(map_var_type("boolean"), VarType::Boolean));
        assert!(matches!(map_var_type("bool"), VarType::Boolean));
        assert!(matches!(map_var_type("list"), VarType::List));
        assert!(matches!(map_var_type("array"), VarType::List));
        assert!(matches!(map_var_type("unknown_xyz"), VarType::String));
    }
}
