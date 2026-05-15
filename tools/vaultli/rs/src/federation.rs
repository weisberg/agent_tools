use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::error::VaultliError;
use crate::paths::resolve_root;
use crate::search::search_records;

pub fn federated_search(
    vaults: &[PathBuf],
    query: Option<&str>,
    limit: Option<usize>,
    per_vault_limit: Option<usize>,
    semantic: bool,
    explain: bool,
    sort: Option<&str>,
    order: &str,
) -> Result<Map<String, Value>, VaultliError> {
    if vaults.is_empty() {
        return Err(VaultliError::Unsupported(
            "at least one vault is required".into(),
        ));
    }

    let mut results = Vec::new();
    let mut vault_summaries = Vec::new();
    for vault in vaults {
        let root = resolve_root(vault)?;
        let matches = search_records(
            &root,
            query,
            None,
            None,
            None,
            None,
            None,
            &[],
            per_vault_limit,
            sort,
            order,
            explain,
            semantic,
        )?;
        let name = root
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| root.display().to_string());
        vault_summaries.push(json!({
            "root": root.display().to_string(),
            "name": name,
            "matches": matches.len(),
        }));
        for mut record in matches {
            if let Some(doc_id) = record.get("id").and_then(Value::as_str) {
                record.insert(
                    "global_id".into(),
                    Value::String(format!("{name}:{doc_id}")),
                );
            }
            record.insert(
                "_vault".into(),
                json!({"root": root.display().to_string(), "name": name}),
            );
            results.push(Value::Object(record));
        }
    }

    if let Some(limit) = limit {
        results.truncate(limit);
    }

    let mut result = Map::new();
    result.insert(
        "query".into(),
        query.map(Value::from).unwrap_or(Value::Null),
    );
    result.insert("total".into(), Value::from(results.len()));
    result.insert("vaults".into(), Value::Array(vault_summaries));
    result.insert("results".into(), Value::Array(results));
    Ok(result)
}
