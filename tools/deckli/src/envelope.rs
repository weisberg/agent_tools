/// JSON response envelope used for all CLI output.
use serde_json::{json, Value};
use std::time::Instant;

/// Wrap a successful result in the standard envelope.
pub fn success_envelope(command: &str, result: Value, start: Instant) -> Value {
    json!({
        "success": true,
        "command": command,
        "result": result,
        "timing_ms": start.elapsed().as_millis() as u64,
    })
}

/// Wrap an error in the standard envelope.
pub fn error_envelope(command: &str, err: &dyn std::fmt::Display) -> Value {
    json!({
        "success": false,
        "command": command,
        "error": {
            "message": err.to_string(),
        }
    })
}

/// Wrap an error with a recovery suggestion.
pub fn error_with_suggestion(command: &str, code: &str, message: &str, suggestion: &str) -> Value {
    json!({
        "success": false,
        "command": command,
        "error": {
            "code": code,
            "message": message,
            "suggestion": suggestion,
        }
    })
}
