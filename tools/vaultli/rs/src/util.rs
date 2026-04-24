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

pub(crate) const DOMAIN_CANDIDATES: &[&str] = &[
    "experimentation",
    "marketing-analytics",
    "infrastructure",
    "tooling",
    "finance",
    "management",
];

pub(crate) const COMMON_TAGS: &[&str] = &[
    "md",
    "sql",
    "j2",
    "jinja",
    "jinja2",
    "json",
    "yaml",
    "yml",
    "txt",
    "docs",
    "doc",
    "templates",
    "template",
    "queries",
    "query",
    "skills",
    "runbooks",
];

/// Returns metadata in schema order with unknown fields appended.
///
/// Null-valued entries and the derived `file`/`hash` keys are omitted, matching
/// the Python reference implementation.
pub(crate) fn order_metadata(metadata: &Map<String, Value>) -> Map<String, Value> {
    let mut ordered = Map::new();
    for field in FRONTMATTER_FIELD_ORDER {
        if let Some(value) = metadata.get(*field) {
            if value.is_null() {
                continue;
            }
            ordered.insert((*field).to_string(), value.clone());
        }
    }
    for (key, value) in metadata {
        if key == "file" || key == "hash" {
            continue;
        }
        if value.is_null() {
            continue;
        }
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

/// Serialize a JSON value with sorted keys, matching Python's
/// `json.dumps(..., sort_keys=True)` semantics used by search.
pub(crate) fn to_sorted_json_string(value: &Value) -> String {
    let mut buf = Vec::new();
    let mut ser =
        serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::CompactFormatter);
    let sorted = sort_keys(value);
    serde::Serialize::serialize(&sorted, &mut ser).unwrap();
    String::from_utf8(buf).unwrap()
}

fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), sort_keys(&map[key]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        other => other.clone(),
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

#[cfg(test)]
mod tests {
    use super::{order_metadata, to_sorted_json_string};
    use serde_json::{json, Map, Value};

    #[test]
    fn orders_known_fields_first_and_skips_file_hash() {
        let mut raw = Map::new();
        raw.insert("hash".into(), Value::String("abc".into()));
        raw.insert("category".into(), Value::String("note".into()));
        raw.insert("custom".into(), Value::String("x".into()));
        raw.insert("id".into(), Value::String("docs/guide".into()));
        raw.insert("file".into(), Value::String("docs/guide.md".into()));
        let ordered = order_metadata(&raw);
        let keys: Vec<&String> = ordered.keys().collect();
        assert_eq!(keys, vec!["id", "category", "custom"]);
        assert!(!ordered.contains_key("file"));
        assert!(!ordered.contains_key("hash"));
    }

    #[test]
    fn skips_null_values() {
        let mut raw = Map::new();
        raw.insert("id".into(), Value::String("a".into()));
        raw.insert("source".into(), Value::Null);
        let ordered = order_metadata(&raw);
        assert!(!ordered.contains_key("source"));
    }

    #[test]
    fn sort_keys_emits_stable_order() {
        let value = json!({"b": 1, "a": {"d": 1, "c": 2}});
        assert_eq!(
            to_sorted_json_string(&value),
            r#"{"a":{"c":2,"d":1},"b":1}"#
        );
    }
}
