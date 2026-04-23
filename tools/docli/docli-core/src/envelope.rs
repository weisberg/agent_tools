use std::time::Instant;

use serde::Serialize;
use serde_json::Value;

use crate::error::{DocliError, ErrorCode};

#[derive(Serialize)]
#[serde(untagged)]
pub enum Envelope<T: Serialize> {
    Ok(OkEnvelope<T>),
    Err(ErrEnvelope),
}

#[derive(Serialize)]
pub struct OkEnvelope<T: Serialize> {
    pub ok: bool,
    pub command: String,
    pub data: T,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct ErrEnvelope {
    pub ok: bool,
    pub command: String,
    pub error: ErrorDetail,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

impl ErrorDetail {
    pub fn from_error(error: &DocliError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
            context: error.context(),
        }
    }
}

pub struct EnvelopeBuilder {
    command: String,
    warnings: Vec<String>,
    started_at: Instant,
}

impl EnvelopeBuilder {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            warnings: Vec::new(),
            started_at: Instant::now(),
        }
    }

    pub fn warn(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn ok<T: Serialize>(self, data: T) -> Envelope<T> {
        Envelope::Ok(OkEnvelope {
            ok: true,
            command: self.command,
            data,
            warnings: self.warnings,
            elapsed_ms: self.started_at.elapsed().as_millis() as u64,
        })
    }

    pub fn err<T: Serialize>(self, error: &DocliError) -> Envelope<T> {
        Envelope::Err(ErrEnvelope {
            ok: false,
            command: self.command,
            error: ErrorDetail::from_error(error),
            warnings: self.warnings,
            elapsed_ms: self.started_at.elapsed().as_millis() as u64,
        })
    }
}
