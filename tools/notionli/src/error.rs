use std::io;

use serde_json::{json, Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub(crate) enum NotionliError {
    #[error("{message}")]
    Usage { message: String },
    #[error("{message}")]
    Auth { message: String },
    #[error("{message}")]
    Permission { message: String },
    #[error("{message}")]
    NotFound { message: String },
    #[error("{message}")]
    Ambiguous {
        message: String,
        candidates: Vec<Value>,
    },
    #[error("{message}")]
    Validation { message: String },
    #[error("{message}")]
    Conflict {
        message: String,
        current_last_edited_time: Option<String>,
    },
    #[error("{message}")]
    RateLimited {
        message: String,
        retry_after_ms: Option<u64>,
    },
    #[error("{message}")]
    Network { message: String },
    #[error("{message}")]
    Partial { message: String },
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl NotionliError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Usage { .. } => "usage_error",
            Self::Auth { .. } => "auth_error",
            Self::Permission { .. } => "permission_denied",
            Self::NotFound { .. } => "object_not_found",
            Self::Ambiguous { .. } => "ambiguous_object",
            Self::Validation { .. } => "validation_error",
            Self::Conflict { .. } => "edit_conflict",
            Self::RateLimited { .. } => "rate_limited",
            Self::Network { .. } => "network_or_api_error",
            Self::Partial { .. } => "partial_failure",
            Self::Io(_) => "io_error",
            Self::Json(_) => "json_error",
        }
    }

    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            Self::Usage { .. } => 1,
            Self::Auth { .. } => 2,
            Self::Permission { .. } => 3,
            Self::NotFound { .. } => 4,
            Self::Ambiguous { .. } => 5,
            Self::Validation { .. } => 6,
            Self::Conflict { .. } => 7,
            Self::RateLimited { .. } => 8,
            Self::Network { .. } => 9,
            Self::Partial { .. } => 10,
            Self::Io(_) | Self::Json(_) => 1,
        }
    }

    pub(crate) fn suggested_fix(&self) -> Option<&'static str> {
        match self {
            Self::Auth { .. } => Some("Run `notionli auth login` for OAuth, set NOTION_API_KEY, create ~/.config/NOTION_API_KEY, pass --token-cmd, or run `notionli auth token set`."),
            Self::Permission { .. } => Some("Check that the Notion integration has been shared into the target page, database, or data source."),
            Self::NotFound { .. } => Some("Verify the ID/alias and confirm the target is shared with the integration."),
            Self::Ambiguous { .. } => Some("Pass a more specific target or use --pick-first."),
            Self::Validation { .. } => Some("Correct the input and retry. Writes require --apply to commit."),
            Self::Conflict { .. } => Some("Fetch the current object, merge changes, then retry with the current last_edited_time."),
            Self::RateLimited { .. } => Some("Retry after the requested delay, or increase --retry for transient rate limits."),
            _ => None,
        }
    }

    pub(crate) fn extra(&self) -> Map<String, Value> {
        let mut map = Map::new();
        match self {
            Self::Ambiguous { candidates, .. } => {
                map.insert("candidates".into(), Value::Array(candidates.clone()));
            }
            Self::Conflict {
                current_last_edited_time: Some(ts),
                ..
            } => {
                map.insert("current_last_edited_time".into(), Value::String(ts.clone()));
            }
            Self::RateLimited {
                retry_after_ms: Some(ms),
                ..
            } => {
                map.insert("retry_after_ms".into(), json!(ms));
            }
            _ => {}
        }
        map
    }
}
