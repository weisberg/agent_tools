//! Validation and repair helpers for DOCX packages.

pub mod invariants;
pub mod redline;
pub mod repair;
pub mod structural;

pub use invariants::check_invariants;
pub use redline::{check_redline, validate_redlines};
pub use repair::{ensure_xml_space_preserve, repair_durable_id_overflow};
pub use structural::validate_structure;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
    pub severity: ValidationSeverity,
    pub part: Option<String>,
}

impl ValidationIssue {
    pub fn error(code: impl Into<String>, message: impl Into<String>, part: Option<&str>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: ValidationSeverity::Error,
            part: part.map(ToString::to_string),
        }
    }

    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        part: Option<&str>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: ValidationSeverity::Warning,
            part: part.map(ToString::to_string),
        }
    }
}
