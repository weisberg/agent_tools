use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The structured result of executing a TaskSpec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Overall success
    pub ok: bool,
    /// Total wall-clock duration in milliseconds
    pub duration_ms: u64,
    /// Captured variables (filtered by `summary` if specified)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, serde_json::Value>,
    /// Per-step results
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepResult>,
    /// Warnings and non-fatal issues
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// If ok is false, a structured error
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TaskError>,
}

/// Result of executing a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step index (0-based)
    pub index: usize,
    /// What kind of step this was
    pub kind: StepKind,
    /// Exit code (for Cmd steps)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
    /// Stdout (subject to token budget / limit)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// Stderr (if not merged)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// Whether output was truncated
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// Lines truncated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated_lines: Option<usize>,
    /// Notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Variables captured by this step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured: Option<Vec<String>>,
}

impl StepResult {
    /// Create a minimal step result (e.g. for let steps).
    pub fn new(index: usize, kind: StepKind, duration_ms: u64) -> Self {
        Self {
            index,
            kind,
            exit_code: None,
            duration_ms,
            stdout: None,
            stderr: None,
            truncated: false,
            truncated_lines: None,
            note: None,
            captured: None,
        }
    }

    /// Create a step result from an error.
    pub fn from_error(index: usize, kind: StepKind, duration_ms: u64, err: &crate::error::ExecError) -> Self {
        Self {
            index,
            kind,
            exit_code: None,
            duration_ms,
            stdout: None,
            stderr: Some(err.to_string()),
            truncated: false,
            truncated_lines: None,
            note: Some(format!("error: {err}")),
            captured: None,
        }
    }
}

/// The kind of step that was executed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Cmd,
    Let,
    Assert,
    ForEach,
    Write,
    Read,
    If,
    Extension(String),
}

/// Structured error in the task result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    /// Step index where failure occurred
    pub step_index: usize,
    /// Error category
    pub kind: ErrorKind,
    /// Human-readable message
    pub message: String,
}

/// Error category for task errors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    NonZeroExit(i32),
    AssertionFailed,
    Timeout,
    UndefinedVariable(String),
    ParseError,
    IoError,
    ValidationError,
    ExtensionError(String),
}
