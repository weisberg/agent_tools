use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::XliError;

/// Standard command status values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Error,
    IssuesFound,
}

/// Mutation execution mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitMode {
    Atomic,
    DryRun,
    None,
}

/// Shared transaction metrics returned in response envelopes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitStats {
    pub elapsed_ms: u64,
    pub file_size_before: u64,
    pub file_size_after: u64,
}

/// Deterministic follow-up repair the caller can apply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepairSuggestion {
    pub action: String,
    pub suggestion: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_range: Option<String>,
}

/// Standard response envelope for every XLI command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResponseEnvelope<T>
where
    T: Serialize + JsonSchema,
{
    pub status: Status,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<T>,
    pub commit_mode: CommitMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint_after: Option<String>,
    pub needs_recalc: bool,
    pub stats: CommitStats,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<XliError>,
    #[serde(default)]
    pub suggested_repairs: Vec<RepairSuggestion>,
}
