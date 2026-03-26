use thiserror::Error;

/// Errors during task spec validation.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("redirect detected in cmd: {0}")]
    RedirectDetected(String),

    #[error("invalid capture name '{0}': must start with '$'")]
    InvalidCaptureName(String),

    #[error("invalid spec: {0}")]
    InvalidSpec(String),

    #[error("unsupported step type: {0} (not yet implemented)")]
    UnsupportedStepType(String),

    #[error("both max_output_tokens and token_budget are set; use only one")]
    DualBudgetSpec,

    #[error("invalid summary reference '{0}': variable not captured by any step")]
    InvalidSummaryRef(String),
}

/// Errors during task execution.
#[derive(Debug, Error)]
pub enum ExecError {
    #[error("command failed with exit code {0}")]
    NonZeroExit(i32),

    #[error("command timed out after {0}ms")]
    Timeout(u64),

    #[error("undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("variable error: {0}")]
    VarError(String),

    #[error("transform error: {0}")]
    TransformError(String),

    #[error("extraction error: {0}")]
    ExtractionError(String),

    #[error("assertion failed: {0}")]
    AssertionFailed(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("budget exhausted")]
    BudgetExhausted,

    #[error("step type not yet supported: {0}")]
    NotYetSupported(String),

    #[error("extension error ({kind}): {message}")]
    ExtensionError { kind: String, message: String },
}

/// Errors during JSON path resolution.
#[derive(Debug, Error)]
pub enum VarError {
    #[error("undefined variable: {0}")]
    Undefined(String),

    #[error("cannot resolve path '{path}' on value")]
    InvalidPath { path: String },

    #[error("array index out of bounds: {index}")]
    IndexOutOfBounds { index: usize },

    #[error("interpolation error: {0}")]
    InterpolationError(String),
}
