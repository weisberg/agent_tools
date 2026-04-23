use schemars::JsonSchema;
use serde::Serialize;
use std::io::{self, Write};
use xli_core::{CommitMode, CommitStats, ResponseEnvelope, Status, XliError};

/// Emit a response envelope in JSON or minimal human-readable text form.
///
/// Returns `true` when the envelope carries `Status::Error` so callers can
/// propagate a non-zero exit code without having to inspect the envelope
/// again. (Issue #26)
pub fn emit<T>(envelope: &ResponseEnvelope<T>, human: bool) -> anyhow::Result<bool>
where
    T: Serialize + JsonSchema,
{
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    if human {
        writeln!(handle, "status: {:?}", envelope.status)?;
        writeln!(handle, "command: {}", envelope.command)?;
        if !envelope.errors.is_empty() {
            for error in &envelope.errors {
                writeln!(handle, "error: {error}")?;
            }
        }
        if let Some(output) = &envelope.output {
            serde_json::to_writer_pretty(&mut handle, output)?;
            writeln!(handle)?;
        }
    } else {
        serde_json::to_writer(&mut handle, envelope)?;
        writeln!(handle)?;
    }

    Ok(envelope.status == Status::Error)
}

#[allow(clippy::too_many_arguments)]
pub fn ok_envelope<T>(
    command: &str,
    input: serde_json::Value,
    output: T,
    warnings: Vec<String>,
    needs_recalc: bool,
    commit_mode: CommitMode,
    fingerprint_before: Option<String>,
    fingerprint_after: Option<String>,
    stats: CommitStats,
) -> ResponseEnvelope<T>
where
    T: Serialize + JsonSchema,
{
    ResponseEnvelope {
        status: Status::Ok,
        command: command.to_string(),
        input: Some(input),
        output: Some(output),
        commit_mode,
        fingerprint_before,
        fingerprint_after,
        needs_recalc,
        stats,
        warnings,
        errors: Vec::new(),
        suggested_repairs: Vec::new(),
    }
}

pub fn error_envelope<T>(
    command: &str,
    input: Option<serde_json::Value>,
    error: XliError,
) -> ResponseEnvelope<T>
where
    T: Serialize + JsonSchema,
{
    ResponseEnvelope {
        status: Status::Error,
        command: command.to_string(),
        input,
        output: None,
        commit_mode: CommitMode::None,
        fingerprint_before: None,
        fingerprint_after: None,
        needs_recalc: false,
        stats: CommitStats::default(),
        warnings: Vec::new(),
        errors: vec![error],
        suggested_repairs: Vec::new(),
    }
}
