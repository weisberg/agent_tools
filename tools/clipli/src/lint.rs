// Template linter — validates templates for variable mismatches and syntax issues.

use regex::Regex;
use serde::Serialize;
use crate::model::TemplateVariable;

// Public types

#[derive(Debug, Clone, Serialize)]
pub struct LintReport {
    pub diagnostics: Vec<LintDiagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LintDiagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub line: Option<usize>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

/// Lint a template HTML string against its schema variables.
pub fn lint(template_html: &str, schema: &[TemplateVariable]) -> LintReport {
    let mut diagnostics = Vec::new();

    // 1. Check for unbalanced {{ }} markers
    check_unbalanced_markers(template_html, &mut diagnostics);

    // 2. Extract all {{ variable_name }} from template
    let template_vars = extract_template_variables(template_html);

    // 3. Check for invalid variable identifiers
    check_invalid_identifiers(template_html, &mut diagnostics);

    // 4. Check for duplicate variable names in schema
    check_duplicate_schema_vars(schema, &mut diagnostics);

    // 5. Variables in template not in schema (warning)
    let schema_names: std::collections::HashSet<&str> = schema.iter().map(|v| v.name.as_str()).collect();
    for (var_name, line_num) in &template_vars {
        if !schema_names.contains(var_name.as_str()) {
            // Skip Jinja2 built-in variables (loop, self, etc.)
            if is_jinja_builtin(var_name) { continue; }
            diagnostics.push(LintDiagnostic {
                severity: Severity::Warning,
                code: "LINT_VAR_NOT_IN_SCHEMA",
                message: format!("variable '{}' used in template but not defined in schema", var_name),
                line: Some(*line_num),
                context: None,
            });
        }
    }

    // 6. Variables in schema not in template (warning)
    let template_var_names: std::collections::HashSet<&str> = template_vars.iter().map(|(n, _)| n.as_str()).collect();
    for var in schema {
        if !template_var_names.contains(var.name.as_str()) {
            diagnostics.push(LintDiagnostic {
                severity: Severity::Warning,
                code: "LINT_SCHEMA_VAR_UNUSED",
                message: format!("variable '{}' defined in schema but not used in template", var.name),
                line: None,
                context: None,
            });
        }
    }

    let error_count = diagnostics.iter().filter(|d| matches!(d.severity, Severity::Error)).count();
    let warning_count = diagnostics.iter().filter(|d| matches!(d.severity, Severity::Warning)).count();

    LintReport { diagnostics, error_count, warning_count }
}

// Implementation helpers

fn check_unbalanced_markers(html: &str, diagnostics: &mut Vec<LintDiagnostic>) {
    // Count {{ and }} independently, check they balance
    // Also check {% and %} for block tags
    for (open, close, name) in [("{{", "}}", "expression"), ("{%", "%}", "block")] {
        let open_count = html.matches(open).count();
        let close_count = html.matches(close).count();
        if open_count != close_count {
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                code: "LINT_UNBALANCED_MARKERS",
                message: format!(
                    "unbalanced Jinja2 {} markers: {} opening '{}' vs {} closing '{}'",
                    name, open_count, open, close_count, close
                ),
                line: None,
                context: None,
            });
        }
    }

    // Also detect specific lines with orphan markers
    for (line_num, line) in html.lines().enumerate() {
        let open_expr = line.matches("{{").count();
        let close_expr = line.matches("}}").count();
        if open_expr != close_expr {
            // Check if it's a multi-line expression (might be OK)
            // Only flag as error if the overall document is already unbalanced
            // (handled above), but provide line-level context
            diagnostics.push(LintDiagnostic {
                severity: Severity::Warning,
                code: "LINT_POSSIBLE_UNBALANCED_LINE",
                message: format!("possible unbalanced expression markers on this line ({} '{{{{' vs {} '}}}}')", open_expr, close_expr),
                line: Some(line_num + 1),
                context: Some(line.trim().chars().take(80).collect()),
            });
        }
    }
}

fn extract_template_variables(html: &str) -> Vec<(String, usize)> {
    // Match {{ var_name }} — simple variables only, skip expressions with | . ( etc.
    let re = Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\}\}").unwrap();
    let mut vars = Vec::new();
    for (line_num, line) in html.lines().enumerate() {
        for cap in re.captures_iter(line) {
            vars.push((cap[1].to_string(), line_num + 1));
        }
    }
    // Also match {{ var_name | filter }} — extract var before the pipe
    let re_filter = Regex::new(r"\{\{\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\|").unwrap();
    for (line_num, line) in html.lines().enumerate() {
        for cap in re_filter.captures_iter(line) {
            let name = cap[1].to_string();
            if !vars.iter().any(|(n, _)| n == &name) {
                vars.push((name, line_num + 1));
            }
        }
    }
    vars
}

fn check_invalid_identifiers(html: &str, diagnostics: &mut Vec<LintDiagnostic>) {
    // Find all {{ ... }} contents and check for invalid identifiers
    let re = Regex::new(r"\{\{(.*?)\}\}").unwrap();
    let ident_re = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap();

    for (line_num, line) in html.lines().enumerate() {
        for cap in re.captures_iter(line) {
            let content = cap[1].trim();
            // Skip expressions with operators, filters, method calls
            if content.contains('|') || content.contains('.') || content.contains('(') {
                continue;
            }
            // Skip empty
            if content.is_empty() { continue; }
            // Check if it's a valid identifier
            if !ident_re.is_match(content) {
                diagnostics.push(LintDiagnostic {
                    severity: Severity::Error,
                    code: "LINT_INVALID_IDENTIFIER",
                    message: format!("invalid variable identifier: '{}'", content),
                    line: Some(line_num + 1),
                    context: Some(format!("{{{{ {} }}}}", content)),
                });
            }
        }
    }
}

fn check_duplicate_schema_vars(schema: &[TemplateVariable], diagnostics: &mut Vec<LintDiagnostic>) {
    let mut seen = std::collections::HashSet::new();
    for var in schema {
        if !seen.insert(&var.name) {
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                code: "LINT_DUPLICATE_SCHEMA_VAR",
                message: format!("duplicate variable '{}' in schema", var.name),
                line: None,
                context: None,
            });
        }
    }
}

fn is_jinja_builtin(name: &str) -> bool {
    matches!(
        name,
        "loop" | "self" | "true" | "false" | "none" | "True" | "False" | "None"
    )
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::VarType;

    fn make_var(name: &str) -> TemplateVariable {
        TemplateVariable {
            name: name.to_string(),
            var_type: VarType::String,
            default_value: None,
            description: None,
        }
    }

    #[test]
    fn lint_clean_template() {
        let html = "<p>{{ title }}</p><p>{{ body }}</p>";
        let schema = vec![make_var("title"), make_var("body")];
        let report = lint(html, &schema);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);
    }

    #[test]
    fn lint_var_in_template_not_schema() {
        let html = "<p>{{ title }}</p><p>{{ subtitle }}</p>";
        let schema = vec![make_var("title")];
        let report = lint(html, &schema);
        assert!(report.warning_count > 0);
        assert!(report.diagnostics.iter().any(|d| d.code == "LINT_VAR_NOT_IN_SCHEMA"));
    }

    #[test]
    fn lint_var_in_schema_not_template() {
        let html = "<p>{{ title }}</p>";
        let schema = vec![make_var("title"), make_var("unused_var")];
        let report = lint(html, &schema);
        assert!(report.warning_count > 0);
        assert!(report.diagnostics.iter().any(|d| d.code == "LINT_SCHEMA_VAR_UNUSED"));
    }

    #[test]
    fn lint_duplicate_schema_vars() {
        let html = "<p>{{ title }}</p>";
        let schema = vec![make_var("title"), make_var("title")];
        let report = lint(html, &schema);
        assert!(report.error_count > 0);
        assert!(report.diagnostics.iter().any(|d| d.code == "LINT_DUPLICATE_SCHEMA_VAR"));
    }

    #[test]
    fn lint_unbalanced_expression_markers() {
        let html = "<p>{{ title }</p>";
        let schema = vec![];
        let report = lint(html, &schema);
        assert!(report.error_count > 0);
        assert!(report.diagnostics.iter().any(|d| d.code == "LINT_UNBALANCED_MARKERS"));
    }

    #[test]
    fn lint_jinja_builtins_not_flagged() {
        let html = "{% if loop.index is odd %}{{ title }}{% endif %}";
        let schema = vec![make_var("title")];
        let report = lint(html, &schema);
        // "loop" should not trigger LINT_VAR_NOT_IN_SCHEMA
        assert!(!report.diagnostics.iter().any(|d| d.message.contains("loop")));
    }

    #[test]
    fn lint_filter_expressions_extract_var() {
        let html = "<p>{{ name | default('World') }}</p>";
        let schema = vec![make_var("name")];
        let report = lint(html, &schema);
        assert_eq!(report.warning_count, 0);
    }
}
