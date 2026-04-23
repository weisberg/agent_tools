/// JSON-RPC protocol types for CLI ↔ sidecar ↔ add-in communication.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A command sent from CLI → sidecar → add-in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique request ID for correlation.
    pub id: String,
    /// Command method name (e.g. "inspect", "add.slide", "set.text").
    pub method: String,
    /// Command parameters.
    #[serde(default)]
    pub params: Value,
}

/// A response sent from add-in → sidecar → CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Correlates with the request ID.
    pub id: String,
    /// Whether the command succeeded.
    pub success: bool,
    /// Result payload (present when success=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error payload (present when success=false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Request {
    pub fn new(method: &str, params: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.to_string(),
            params,
        }
    }
}
