use std::{io, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    FileNotFound,
    InvalidDocx,
    InvalidSpec,
    InvalidJob,
    InvalidTarget,
    InvalidOperation,
    RefNotFound,
    ValidationFailed,
    InvariantViolation,
    IdCollision,
    DependencyMissing,
    TemplateNotFound,
    TemplateVarMissing,
    CommitFailed,
    RevalidationFailed,
}

#[derive(Debug, Error)]
pub enum DocliError {
    #[error("Input file does not exist: {path}")]
    FileNotFound { path: PathBuf },
    #[error("{message}")]
    InvalidDocx { message: String },
    #[error("{message}")]
    InvalidSpec { message: String },
    #[error("{message}")]
    InvalidJob { message: String },
    #[error("{message}")]
    InvalidTarget { message: String },
    #[error("{message}")]
    InvalidOperation { message: String },
    #[error("Reference not found: {reference}")]
    RefNotFound { reference: String },
    #[error("{message}")]
    ValidationFailed { message: String },
    #[error("{message}")]
    InvariantViolation { message: String },
    #[error("{message}")]
    IdCollision { message: String },
    #[error("Missing dependency: {dependency}")]
    DependencyMissing { dependency: String },
    #[error("Template not found: {template}")]
    TemplateNotFound { template: String },
    #[error("Template variable missing: {variable}")]
    TemplateVarMissing { variable: String },
    #[error("{message}")]
    CommitFailed { message: String },
    #[error("{message}")]
    RevalidationFailed { message: String },
}

impl DocliError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::FileNotFound { .. } => ErrorCode::FileNotFound,
            Self::InvalidDocx { .. } => ErrorCode::InvalidDocx,
            Self::InvalidSpec { .. } => ErrorCode::InvalidSpec,
            Self::InvalidJob { .. } => ErrorCode::InvalidJob,
            Self::InvalidTarget { .. } => ErrorCode::InvalidTarget,
            Self::InvalidOperation { .. } => ErrorCode::InvalidOperation,
            Self::RefNotFound { .. } => ErrorCode::RefNotFound,
            Self::ValidationFailed { .. } => ErrorCode::ValidationFailed,
            Self::InvariantViolation { .. } => ErrorCode::InvariantViolation,
            Self::IdCollision { .. } => ErrorCode::IdCollision,
            Self::DependencyMissing { .. } => ErrorCode::DependencyMissing,
            Self::TemplateNotFound { .. } => ErrorCode::TemplateNotFound,
            Self::TemplateVarMissing { .. } => ErrorCode::TemplateVarMissing,
            Self::CommitFailed { .. } => ErrorCode::CommitFailed,
            Self::RevalidationFailed { .. } => ErrorCode::RevalidationFailed,
        }
    }

    pub fn context(&self) -> Option<Value> {
        match self {
            Self::FileNotFound { path } => Some(json!({ "path": path })),
            Self::RefNotFound { reference } => Some(json!({ "reference": reference })),
            Self::DependencyMissing { dependency } => Some(json!({ "dependency": dependency })),
            Self::TemplateNotFound { template } => Some(json!({ "template": template })),
            Self::TemplateVarMissing { variable } => Some(json!({ "variable": variable })),
            _ => None,
        }
    }
}

impl From<io::Error> for DocliError {
    fn from(source: io::Error) -> Self {
        if source.kind() == io::ErrorKind::NotFound {
            return Self::FileNotFound {
                path: PathBuf::from(source.to_string()),
            };
        }

        Self::CommitFailed {
            message: source.to_string(),
        }
    }
}

impl From<zip::result::ZipError> for DocliError {
    fn from(source: zip::result::ZipError) -> Self {
        Self::InvalidDocx {
            message: source.to_string(),
        }
    }
}

impl From<roxmltree::Error> for DocliError {
    fn from(source: roxmltree::Error) -> Self {
        Self::InvalidDocx {
            message: source.to_string(),
        }
    }
}
