use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub relative_path: String,
    pub metadata: Map<String, Value>,
    pub body: String,
    pub has_frontmatter: bool,
}

impl ParsedDocument {
    pub fn doc_id(&self) -> Option<&str> {
        self.metadata.get("id").and_then(Value::as_str)
    }

    pub fn is_sidecar(&self) -> bool {
        self.relative_path.ends_with(".md")
            && self
                .relative_path
                .strip_suffix(".md")
                .map(|stem| stem.contains('.'))
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WarningRecord {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "id")]
    pub doc_id: Option<String>,
    pub level: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub root: String,
    pub valid: bool,
    pub issue_count: usize,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexBuildResult {
    pub root: String,
    pub full: bool,
    pub indexed: usize,
    pub updated: usize,
    pub pruned: usize,
    pub skipped: usize,
    pub warnings: Vec<WarningRecord>,
}
