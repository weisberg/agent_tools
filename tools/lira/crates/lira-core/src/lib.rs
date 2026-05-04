use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error_code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub suggestions: Vec<Suggestion>,
}

impl ErrorEnvelope {
    pub fn new(error_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error_code: error_code.into(),
            message: message.into(),
            details: None,
            suggestions: Vec::new(),
        }
    }

    pub fn with_suggestion(
        mut self,
        command: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            command: command.into(),
            reason: reason.into(),
        });
        self
    }
}
