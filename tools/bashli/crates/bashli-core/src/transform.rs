use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Transform applied to command output before capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    Raw,
    Trim,
    Lines,
    Json,
    CountLines,
    CountBytes,
    CountWords,
    Head(usize),
    Tail(usize),
    Sort(SortSpec),
    Unique,
    Jq(String),
    Sed(SedSpec),
    Awk(AwkSpec),
    Grep(GrepSpec),
    Split(String),
    Pipe(Vec<Transform>),
    Base64Encode,
    Base64Decode,
    CodeBlock(Option<String>),
    Regex(String),
    Sha256,
    Extension {
        name: String,
        #[serde(default)]
        config: serde_json::Value,
    },
}

/// Sort specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SortSpec {
    #[serde(default)]
    pub numeric: bool,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

/// Grep specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepSpec {
    pub pattern: String,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub ignore_case: bool,
    #[serde(default)]
    pub only_matching: bool,
    #[serde(default)]
    pub count: bool,
}

/// Awk specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwkSpec {
    pub program: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_separator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<BTreeMap<String, String>>,
}

/// Sed specification — either a single command string or array of commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SedSpec {
    Single(String),
    Multiple(Vec<String>),
}
