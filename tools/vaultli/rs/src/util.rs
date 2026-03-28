use serde_json::{Map, Value};
use std::path::PathBuf;

pub(crate) const VAULT_MARKER: &str = ".kbroot";
pub(crate) const INDEX_FILENAME: &str = "INDEX.jsonl";

pub(crate) const FRONTMATTER_FIELD_ORDER: &[&str] = &[
    "id",
    "title",
    "description",
    "tags",
    "category",
    "aliases",
    "author",
    "status",
    "created",
    "updated",
    "source",
    "depends_on",
    "related",
    "tokens",
    "priority",
    "scope",
    "domain",
    "version",
];
pub(crate) const REQUIRED_FIELDS: &[&str] = &["id", "title", "description"];
pub(crate) const LIST_FIELDS: &[&str] = &["tags", "aliases", "depends_on", "related"];
pub(crate) const STRING_FIELDS: &[&str] = &[
    "id",
    "title",
    "description",
    "category",
    "author",
    "status",
    "source",
    "scope",
    "domain",
];
pub(crate) const INTEGER_FIELDS: &[&str] = &["tokens", "priority"];

pub(crate) fn order_metadata(metadata: &Map<String, Value>) -> Map<String, Value> {
    let mut ordered = Map::new();
    for field in FRONTMATTER_FIELD_ORDER {
        if let Some(value) = metadata.get(*field) {
            ordered.insert((*field).to_string(), value.clone());
        }
    }
    for (key, value) in metadata {
        if !ordered.contains_key(key) {
            ordered.insert(key.clone(), value.clone());
        }
    }
    ordered
}

pub(crate) fn map_from_pairs(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_string(), value);
    }
    map
}

pub(crate) fn yaml_scalar(value: &Value) -> String {
    match value {
        Value::String(text) => {
            if text.is_empty()
                || text.contains(':')
                || text.starts_with('[')
                || text.starts_with('{')
                || text.starts_with('#')
            {
                format!("{text:?}")
            } else {
                text.clone()
            }
        }
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".into(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

pub(crate) fn which(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for entry in std::env::split_paths(&path_var) {
        let candidate = entry.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
