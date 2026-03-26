use bashli_core::*;
use std::io::Read;

/// Parse a TaskSpec from CLI arguments.
pub fn parse_input(
    inline_spec: &Option<String>,
    file_path: &Option<String>,
) -> Result<TaskSpec, InputError> {
    let raw = if let Some(ref spec_str) = inline_spec {
        if spec_str == "-" {
            // Read from stdin
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)
                .map_err(|e| InputError::Io(e.to_string()))?;
            buf
        } else {
            spec_str.clone()
        }
    } else if let Some(ref path) = file_path {
        std::fs::read_to_string(path)
            .map_err(|e| InputError::Io(format!("cannot read {path}: {e}")))?
    } else {
        // Try reading from stdin
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)
            .map_err(|e| InputError::Io(e.to_string()))?;
        if buf.is_empty() {
            return Err(InputError::NoInput);
        }
        buf
    };

    // Try parsing as full TaskSpec
    if let Ok(spec) = serde_json::from_str::<TaskSpec>(&raw) {
        return Ok(spec);
    }

    // Try shorthand: bare array of steps
    if let Ok(steps) = serde_json::from_str::<Vec<Step>>(&raw) {
        return Ok(TaskSpec {
            description: None,
            mode: ExecutionMode::Sequential,
            settings: GlobalSettings::default(),
            let_vars: None,
            steps,
            summary: None,
        });
    }

    // Try shorthand: object with "cmd" field (single command)
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
        if val.get("cmd").is_some() {
            if let Ok(cmd_step) = serde_json::from_value::<CmdStep>(val) {
                return Ok(TaskSpec {
                    description: None,
                    mode: ExecutionMode::Sequential,
                    settings: GlobalSettings::default(),
                    let_vars: None,
                    steps: vec![Step::Structured(StructuredStep::Cmd(cmd_step))],
                    summary: None,
                });
            }
        }
    }

    // Nothing worked
    Err(InputError::Parse(format!(
        "could not parse input as TaskSpec, step array, or single command: {}",
        if raw.len() > 100 { &raw[..100] } else { &raw }
    )))
}

#[derive(Debug)]
pub enum InputError {
    Io(String),
    Parse(String),
    NoInput,
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Parse(e) => write!(f, "parse error: {e}"),
            Self::NoInput => write!(f, "no input provided. Use: bashli '<json>' or bashli -f <file>"),
        }
    }
}
