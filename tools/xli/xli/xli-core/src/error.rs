use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::addressing::AddressError;

/// Structured error codes emitted by XLI commands.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum XliError {
    #[error("Workbook file not found: {path}")]
    FileNotFound { path: String },
    #[error("CLI parse error: {message}")]
    CliParseError { message: String },
    #[error("Write lock is already held for {path}")]
    LockConflict { path: String },
    #[error("Sheet not found: {sheet}")]
    SheetNotFound { sheet: String },
    #[error("Cell reference {address} is outside sheet dimensions")]
    CellRefOutOfBounds {
        address: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimensions: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    #[error("Invalid cell address: {address}")]
    InvalidCellAddress { address: String },
    #[error("Formula parse error in {formula}")]
    FormulaParseError {
        formula: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    #[error("Fingerprint mismatch")]
    FingerprintMismatch { expected: String, actual: String },
    #[error("Template not found: {template}")]
    TemplateNotFound { template: String },
    #[error("Template parameter missing: {parameter}")]
    TemplateParamMissing { parameter: String },
    #[error("Template parameter invalid: {parameter}")]
    TemplateParamInvalid { parameter: String, details: String },
    #[error("Recalculation timed out after {timeout_secs}s")]
    RecalcTimeout { timeout_secs: u64 },
    #[error("Recalculation failed")]
    RecalcFailed { details: String },
    #[error("Write conflict for {target}")]
    WriteConflict {
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    #[error("Spec validation error in {spec}")]
    SpecValidationError { spec: String, details: String },
    #[error("Batch partial failure")]
    BatchPartialFailure { failed_ops: usize, total_ops: usize },
    #[error("Workbook OOXML is corrupt")]
    OoxmlCorrupt { details: String },
}

impl From<AddressError> for XliError {
    fn from(value: AddressError) -> Self {
        match value {
            AddressError::ColumnOutOfBounds { column } => Self::CellRefOutOfBounds {
                address: column,
                dimensions: None,
                details: Some("Column exceeds Excel limits".to_string()),
            },
            AddressError::RowOutOfBounds { row, .. } => Self::CellRefOutOfBounds {
                address: row.to_string(),
                dimensions: None,
                details: Some("Row exceeds Excel limits".to_string()),
            },
            AddressError::EmptyInput => Self::InvalidCellAddress {
                address: String::new(),
            },
            AddressError::InvalidAddress { input } => Self::InvalidCellAddress { address: input },
            AddressError::SheetMismatch { left, right } => Self::SpecValidationError {
                spec: "range".to_string(),
                details: format!("Range spans multiple sheets: {left} vs {right}"),
            },
        }
    }
}
