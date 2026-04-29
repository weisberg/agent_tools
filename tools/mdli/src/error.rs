use std::io;

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum MdliError {
    #[error("{message}")]
    User {
        code: &'static str,
        message: String,
        details: Option<Box<Value>>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    #[error("{message}")]
    Invariant {
        code: &'static str,
        message: String,
        details: Option<Box<Value>>,
    },
    #[error("{message}")]
    Io {
        code: &'static str,
        message: String,
        #[source]
        source: io::Error,
    },
}

impl MdliError {
    pub(crate) fn user(code: &'static str, message: impl Into<String>) -> Self {
        Self::User {
            code,
            message: message.into(),
            details: None,
            source: None,
        }
    }

    pub(crate) fn invariant(code: &'static str, message: impl Into<String>) -> Self {
        Self::Invariant {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub(crate) fn io(code: &'static str, message: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            code,
            message: message.into(),
            source,
        }
    }

    pub(crate) fn with_details(mut self, value: Value) -> Self {
        match &mut self {
            Self::User { details, .. } | Self::Invariant { details, .. } => {
                *details = Some(Box::new(value));
            }
            Self::Io { .. } => {}
        }
        self
    }

    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::User { code, .. } | Self::Invariant { code, .. } | Self::Io { code, .. } => code,
        }
    }

    pub(crate) fn message(&self) -> String {
        self.to_string()
    }

    pub(crate) fn details(&self) -> Option<&Value> {
        match self {
            Self::User { details, .. } | Self::Invariant { details, .. } => details.as_deref(),
            Self::Io { .. } => None,
        }
    }

    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::User { .. } => 1,
            Self::Invariant { .. } => 2,
            Self::Io { code, .. } if *code == "E_STALE_PREIMAGE" => 4,
            Self::Io { .. } => 3,
        }
    }
}
