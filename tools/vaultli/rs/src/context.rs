use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::error::VaultliError;
use crate::index::load_index_records;
use crate::paths::resolve_root;
use crate::search::{resolve_record, search_records, show_record};

pub fn assemble_context(
    root: &Path,
    query: Option<&str>,
    ids: &[String],
    token_budget: Option<i64>,
    include_related: bool,
    include_dependencies: bool,
    limit: Option<usize>,
) -> Result<Map<String, Value>, VaultliError> {
    if token_budget.is_some_and(|budget| budget < 0) {
        return Err(VaultliError::Unsupported(
            "token budget must be greater than or equal to 0".into(),
        ));
    }

    let root = resolve_root(root)?;
    let seed_records = if ids.is_empty() {
        search_records(
            &root,
            query,
            None,
            None,
            None,
            None,
            None,
            &[],
            limit,
            None,
            "asc",
            false,
            false,
        )?
    } else {
        ids.iter()
            .map(|id| show_record(&root, id))
            .collect::<Result<Vec<_>, _>>()?
    };
    let by_id = load_index_records(&root)?
        .into_iter()
        .filter_map(|record| {
            let id = record.get("id").and_then(Value::as_str)?.to_string();
            Some((id, record))
        })
        .collect::<BTreeMap<_, _>>();

    let mut ordered_ids = Vec::new();
    for record in seed_records {
        let Some(doc_id) = record.get("id").and_then(Value::as_str) else {
            continue;
        };
        ordered_ids.push(doc_id.to_string());
        if include_dependencies {
            push_refs(&mut ordered_ids, record.get("depends_on"));
        }
        if include_related {
            push_refs(&mut ordered_ids, record.get("related"));
        }
    }

    let mut records = Vec::new();
    let mut seen = BTreeSet::new();
    let mut used_tokens = 0_i64;
    for doc_id in ordered_ids {
        if seen.contains(&doc_id) {
            continue;
        }
        let Some(record) = by_id.get(&doc_id) else {
            continue;
        };
        let token_count = record.get("tokens").and_then(Value::as_i64).unwrap_or(0);
        if token_budget.is_some_and(|budget| used_tokens + token_count > budget) {
            continue;
        }
        let resolved = resolve_record(&root, &doc_id, true, false)?;
        records.push(json!({
            "id": doc_id,
            "tokens": token_count,
            "file": resolved.get("file").cloned().unwrap_or(Value::Null),
            "record": Value::Object(record.clone()),
            "body": resolved.get("body").cloned().unwrap_or_else(|| Value::String(String::new())),
        }));
        used_tokens += token_count;
        seen.insert(doc_id);
    }

    let mut result = Map::new();
    result.insert("root".into(), Value::String(root.display().to_string()));
    result.insert(
        "query".into(),
        query.map(Value::from).unwrap_or(Value::Null),
    );
    result.insert(
        "token_budget".into(),
        token_budget.map(Value::from).unwrap_or(Value::Null),
    );
    result.insert("used_tokens".into(), Value::from(used_tokens));
    result.insert("count".into(), Value::from(records.len()));
    result.insert("records".into(), Value::Array(records));
    Ok(result)
}

fn push_refs(target: &mut Vec<String>, refs: Option<&Value>) {
    let Some(items) = refs.and_then(Value::as_array) else {
        return;
    };
    target.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
}
