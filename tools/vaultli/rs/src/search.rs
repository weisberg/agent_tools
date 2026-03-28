use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

use crate::error::VaultliError;
use crate::index::load_index_records;
use crate::util::which;

pub fn show_record(root: &std::path::Path, doc_id: &str) -> Result<Map<String, Value>, VaultliError> {
    for record in load_index_records(root)? {
        if record.get("id").and_then(Value::as_str) == Some(doc_id) {
            return Ok(record);
        }
    }
    Err(VaultliError::IdNotFound(doc_id.to_string()))
}

pub fn search_records(
    root: &std::path::Path,
    query: Option<&str>,
    jq_filter: Option<&str>,
) -> Result<Vec<Map<String, Value>>, VaultliError> {
    let mut records = load_index_records(root)?;
    if let Some(query) = query {
        let needle = query.to_lowercase();
        records.retain(|record| {
            serde_json::to_string(record)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle)
        });
    }

    if let Some(filter) = jq_filter {
        let jq_path = which("jq").ok_or(VaultliError::JqUnavailable)?;
        let payload = records
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        let mut child = Command::new(jq_path)
            .arg("-c")
            .arg(filter)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(payload.as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(VaultliError::JqFilterFailed(message));
        }
        let mut filtered = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)?;
            match value {
                Value::Object(map) => filtered.push(map),
                _ => return Err(VaultliError::JqFilterInvalid),
            }
        }
        records = filtered;
    }
    Ok(records)
}
