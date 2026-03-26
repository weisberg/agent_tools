use crate::error::ValidationError;
use crate::spec::*;

/// Validate a TaskSpec before execution.
pub fn validate_task_spec(spec: &TaskSpec) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Validate each step
    for step in &spec.steps {
        validate_step(step, &mut errors);
    }

    // Check for dual budget specification
    if spec.settings.max_output_tokens.is_some() && spec.settings.token_budget.is_some() {
        errors.push(ValidationError::DualBudgetSpec);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_step(step: &Step, errors: &mut Vec<ValidationError>) {
    match step {
        Step::BareCmd(cmd) => {
            check_redirects(cmd, errors);
        }
        Step::Structured(s) => match s {
            StructuredStep::Cmd(cmd_step) => {
                check_redirects(&cmd_step.cmd, errors);
                if let Some(ref name) = cmd_step.capture {
                    check_capture_name(name, errors);
                }
                if let Some(ref on_failure) = cmd_step.on_failure {
                    validate_step(on_failure, errors);
                }
            }
            StructuredStep::Let(let_step) => {
                for name in let_step.bindings.keys() {
                    check_capture_name(name, errors);
                }
            }
            StructuredStep::Assert(_) => {}
            StructuredStep::Write(_) => {}
            StructuredStep::Read(r) => {
                check_capture_name(&r.read.capture, errors);
            }
            StructuredStep::If(if_step) => {
                for step in &if_step.then {
                    validate_step(step, errors);
                }
                if let Some(ref else_steps) = if_step.else_steps {
                    for step in else_steps {
                        validate_step(step, errors);
                    }
                }
            }
            StructuredStep::ForEach(fe) => {
                if let Some(ref name) = fe.capture {
                    check_capture_name(name, errors);
                }
                for step in &fe.steps {
                    validate_step(step, errors);
                }
            }
            StructuredStep::Extension(_) => {}
        },
    }
}

/// Check for redirect operators in a command string.
fn check_redirects(cmd: &str, errors: &mut Vec<ValidationError>) {
    // Patterns that indicate shell redirects
    let redirect_patterns: &[(&str, &str)] = &[
        ("2>&1", "Use \"stderr\": \"merge\" instead"),
        ("2>/dev/null", "Use \"stderr\": \"discard\" instead"),
        ("&>", "Use stdout/stderr fields instead"),
        ("2>>", "Use \"stderr\": {\"file\": {\"path\": ..., \"append\": true}} instead"),
        ("2>", "Use \"stderr\": {\"file\": {\"path\": ...}} instead"),
        (">>", "Use \"stdout\": {\"file\": {\"path\": ..., \"append\": true}} instead"),
    ];

    for (pattern, suggestion) in redirect_patterns {
        if cmd.contains(pattern) {
            errors.push(ValidationError::RedirectDetected(format!(
                "found '{pattern}' in cmd \"{cmd}\". {suggestion}"
            )));
            return;
        }
    }

    // Check for standalone > redirect (but not in quoted strings or as part of other operators)
    // Simple heuristic: look for > not preceded by 2, &, or another >
    check_stdout_redirect(cmd, errors);

    // Check for pipe operator
    if cmd.contains(" | ") || cmd.ends_with(" |") || cmd.starts_with("| ") {
        errors.push(ValidationError::RedirectDetected(format!(
            "found pipe '|' in cmd \"{cmd}\". Use transforms instead of piping"
        )));
    }
}

fn check_stdout_redirect(cmd: &str, errors: &mut Vec<ValidationError>) {
    let bytes = cmd.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'>' {
            // Skip if part of 2>, &>, >>, 2>&1 (already checked above)
            if i > 0 && (bytes[i - 1] == b'2' || bytes[i - 1] == b'&') {
                continue;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                continue; // >>
            }
            // This is a standalone > redirect
            errors.push(ValidationError::RedirectDetected(format!(
                "found '>' redirect in cmd \"{cmd}\". Use \"stdout\": {{\"file\": {{\"path\": ...}}}} instead"
            )));
            return;
        }
    }
}

fn check_capture_name(name: &str, errors: &mut Vec<ValidationError>) {
    if !name.starts_with('$') {
        errors.push(ValidationError::InvalidCaptureName(name.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redirect_detection() {
        let spec = TaskSpec {
            description: None,
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps: vec![Step::BareCmd("ls > output.txt".to_string())],
            summary: None,
        };
        let result = validate_task_spec(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_pipe_detection() {
        let spec = TaskSpec {
            description: None,
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps: vec![Step::BareCmd("ls | grep foo".to_string())],
            summary: None,
        };
        let result = validate_task_spec(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_spec() {
        let spec = TaskSpec {
            description: Some("test".to_string()),
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps: vec![Step::BareCmd("ls -la".to_string())],
            summary: None,
        };
        let result = validate_task_spec(&spec);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stderr_redirect_detection() {
        let spec = TaskSpec {
            description: None,
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps: vec![Step::BareCmd("cmd 2>&1".to_string())],
            summary: None,
        };
        let result = validate_task_spec(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_capture_name() {
        let spec = TaskSpec {
            description: None,
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps: vec![Step::Structured(StructuredStep::Cmd(CmdStep {
                cmd: "echo hello".to_string(),
                capture: Some("RESULT".to_string()), // missing $
                transform: None,
                extract: None,
                stdout: None,
                stderr: None,
                stdin: None,
                timeout_ms: None,
                cwd: None,
                env: None,
                limit: None,
                retry: None,
                on_failure: None,
                verbose: None,
            }))],
            summary: None,
        };
        let result = validate_task_spec(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_dual_budget_rejected() {
        let spec = TaskSpec {
            description: None,
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings {
                max_output_tokens: Some(1000),
                token_budget: Some(TokenBudget {
                    max_tokens: 500,
                    allocation: BudgetAllocation::Equal,
                    overflow: OverflowStrategy::Truncate,
                }),
                ..GlobalSettings::default()
            },
            let_vars: None,
            steps: vec![Step::BareCmd("ls".to_string())],
            summary: None,
        };
        let result = validate_task_spec(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let spec = TaskSpec {
            description: Some("test task".to_string()),
            mode: ExecutionMode::Independent,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps: vec![Step::BareCmd("echo hello".to_string())],
            summary: Some(vec!["$RESULT".to_string()]),
        };
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: TaskSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.mode, ExecutionMode::Independent);
    }
}
