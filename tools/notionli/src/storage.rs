use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::context::Context;
use crate::error::NotionliError;
use crate::resolve::ResolvedTarget;
use crate::util::{now, object_id, object_title, slugify, sql_escape, sql_nullable};

pub(crate) fn sqlite_exec(db: &Path, sql: &str) -> Result<(), NotionliError> {
    let status = Command::new("sqlite3")
        .arg(db)
        .arg(sql)
        .stdout(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(NotionliError::Io(std::io::Error::other(
            "sqlite3 command failed",
        )))
    }
}

pub(crate) fn sqlite_query_json(db: &Path, sql: &str) -> Result<Vec<Value>, NotionliError> {
    let output = Command::new("sqlite3")
        .arg("-json")
        .arg(db)
        .arg(sql)
        .output()?;
    if !output.status.success() {
        return Err(NotionliError::Io(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).to_string(),
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&text)?)
}

pub(crate) fn alias_set(
    ctx: &Context,
    name: &str,
    object_type: &str,
    object_id: &str,
    reference: &str,
    title: Option<&str>,
    url: Option<&str>,
) -> Result<(), NotionliError> {
    sqlite_exec(
        &ctx.db_path,
        &format!(
            "INSERT OR REPLACE INTO aliases (name, object_type, object_id, reference, title, url, updated_at) VALUES ('{}','{}','{}','{}',{}, {}, '{}')",
            sql_escape(name),
            sql_escape(object_type),
            sql_escape(object_id),
            sql_escape(reference),
            sql_nullable(title),
            sql_nullable(url),
            now()
        ),
    )
}

pub(crate) fn alias_get(
    ctx: &Context,
    name: &str,
) -> Result<Option<ResolvedTarget>, NotionliError> {
    let rows = sqlite_query_json(
        &ctx.db_path,
        &format!("SELECT * FROM aliases WHERE name = '{}'", sql_escape(name)),
    )?;
    Ok(rows.into_iter().next().map(|row| ResolvedTarget {
        object_type: row
            .get("object_type")
            .and_then(Value::as_str)
            .unwrap_or("page")
            .to_string(),
        id: row
            .get("object_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        alias: Some(name.to_string()),
        slug: None,
        title: row.get("title").and_then(Value::as_str).map(str::to_string),
        url: row.get("url").and_then(Value::as_str).map(str::to_string),
        confidence: 1.0,
    }))
}

pub(crate) fn cache_object(ctx: &Context, object: &Value) -> Result<(), NotionliError> {
    let Some(id) = object_id(object) else {
        return Ok(());
    };
    let object_type = object
        .get("object")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let title = object_title(object);
    let url = object
        .get("url")
        .and_then(Value::as_str)
        .map(str::to_string);
    let slug = title.as_ref().map(|title| slugify(title));
    let raw = serde_json::to_string(object)?;
    sqlite_exec(
        &ctx.db_path,
        &format!(
            "INSERT OR REPLACE INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('{}','{}',{},{},{},'{}','{}')",
            sql_escape(object_type),
            sql_escape(&id),
            sql_nullable(slug.as_deref()),
            sql_nullable(title.as_deref()),
            sql_nullable(url.as_deref()),
            sql_escape(&raw),
            now()
        ),
    )?;
    sqlite_exec(
        &ctx.db_path,
        &format!(
            "INSERT INTO objects_fts (object_id, object_type, slug, title, raw_json) VALUES ('{}','{}',{},{},'{}')",
            sql_escape(&id),
            sql_escape(object_type),
            sql_nullable(slug.as_deref()),
            sql_nullable(title.as_deref()),
            sql_escape(&raw),
        ),
    )
}

pub(crate) fn object_by_slug_or_title(
    ctx: &Context,
    query: &str,
) -> Result<Option<ResolvedTarget>, NotionliError> {
    let rows = sqlite_query_json(
        &ctx.db_path,
        &format!(
            "SELECT object_type, object_id, slug, title, url FROM objects WHERE slug = '{}' OR title = '{}' OR object_id = '{}' ORDER BY updated_at DESC LIMIT 2",
            sql_escape(query),
            sql_escape(query),
            sql_escape(query),
        ),
    )?;
    if rows.len() > 1 && !ctx.pick_first {
        return Err(NotionliError::Ambiguous {
            message: format!("Found multiple cached objects matching '{query}'."),
            candidates: rows,
        });
    }
    Ok(rows.into_iter().next().map(|row| ResolvedTarget {
        object_type: row
            .get("object_type")
            .and_then(Value::as_str)
            .unwrap_or("page")
            .to_string(),
        id: row
            .get("object_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        alias: None,
        slug: row.get("slug").and_then(Value::as_str).map(str::to_string),
        title: row.get("title").and_then(Value::as_str).map(str::to_string),
        url: row.get("url").and_then(Value::as_str).map(str::to_string),
        confidence: 0.86,
    }))
}

pub(crate) fn state_set(ctx: &Context, key: &str, value: &str) -> Result<(), NotionliError> {
    sqlite_exec(
        &ctx.db_path,
        &format!(
            "INSERT OR REPLACE INTO state (key, value, updated_at) VALUES ('{}','{}','{}')",
            sql_escape(key),
            sql_escape(value),
            now()
        ),
    )
}

pub(crate) fn state_get(ctx: &Context, key: &str) -> Result<Option<String>, NotionliError> {
    let rows = sqlite_query_json(
        &ctx.db_path,
        &format!("SELECT value FROM state WHERE key = '{}'", sql_escape(key)),
    )?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|row| row.get("value").and_then(Value::as_str).map(str::to_string)))
}

pub(crate) fn config_set(ctx: &Context, key: &str, value: &str) -> Result<(), NotionliError> {
    sqlite_exec(
        &ctx.db_path,
        &format!(
            "INSERT OR REPLACE INTO config (key, value, updated_at) VALUES ('{}','{}','{}')",
            sql_escape(key),
            sql_escape(value),
            now()
        ),
    )
}

pub(crate) fn config_get(ctx: &Context, key: &str) -> Result<Option<String>, NotionliError> {
    let rows = sqlite_query_json(
        &ctx.db_path,
        &format!("SELECT value FROM config WHERE key = '{}'", sql_escape(key)),
    )?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|row| row.get("value").and_then(Value::as_str).map(str::to_string)))
}

pub(crate) fn log_operation(
    ctx: &Context,
    operation_id: &str,
    command: &str,
    receipt: &Value,
    inverse: Option<String>,
) -> Result<(), NotionliError> {
    sqlite_exec(
        &ctx.db_path,
        &format!(
            "INSERT OR REPLACE INTO oplog (operation_id, command, target, receipt_json, inverse_command, created_at, status) VALUES ('{}','{}','{}','{}',{},'{}','complete')",
            sql_escape(operation_id),
            sql_escape(command),
            sql_escape(&receipt.get("target").cloned().unwrap_or(Value::Null).to_string()),
            sql_escape(&receipt.to_string()),
            sql_nullable(inverse.as_deref()),
            now(),
        ),
    )?;
    let audit = json!({
        "operation_id": operation_id,
        "timestamp": now(),
        "profile": ctx.profile,
        "actor": "agent",
        "command": command,
        "objects_touched": [receipt.get("target").cloned().unwrap_or(Value::Null)],
        "changes": receipt.get("changes").cloned().unwrap_or(Value::Array(Vec::new())),
        "undo_command": receipt.get("undo").and_then(|u| u.get("command")).cloned().unwrap_or(Value::Null),
    });
    let audit_path = ctx.profile_dir.join("audit.log");
    let mut existing = fs::read_to_string(&audit_path).unwrap_or_default();
    existing.push_str(&serde_json::to_string(&audit)?);
    existing.push('\n');
    fs::write(audit_path, existing)?;
    Ok(())
}
