use serde::{Deserialize, Serialize};

/// Extraction method for pulling subvariables from output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extraction {
    /// Apply a jaq filter expression
    Jq(String),
    /// Regex with named capture groups
    Regex(String),
    /// Count lines matching a pattern
    CountMatching(String),
    /// First line matching a pattern
    FirstMatching(String),
    /// All lines matching a pattern (as JSON array)
    AllMatching(String),
    /// Specific line number (0-indexed)
    Line(usize),
    /// A range of lines [start, end)
    LineRange(usize, usize),
    /// Extension point
    Extension {
        name: String,
        #[serde(default)]
        config: serde_json::Value,
    },
}
