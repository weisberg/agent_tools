mod eval;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum JqError {
    #[error("jq compile error: {0}")]
    CompileError(String),
    #[error("jq eval error: {0}")]
    EvalError(String),
    #[error("json parse error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub use eval::{eval, eval_to_string, eval_value};
