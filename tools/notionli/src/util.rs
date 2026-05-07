use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};

use crate::error::NotionliError;

pub(crate) fn object_id(value: &Value) -> Option<String> {
    value.get("id").and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn object_title(value: &Value) -> Option<String> {
    let props = value.get("properties")?.as_object()?;
    for property in props.values() {
        if let Some(arr) = property.get("title").and_then(Value::as_array) {
            let title = arr
                .iter()
                .filter_map(|item| item.get("plain_text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    value
        .get("title")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("plain_text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .filter(|s| !s.is_empty())
}

pub(crate) fn split_assignment(input: &str) -> Result<(String, String), NotionliError> {
    let Some((key, value)) = input.split_once('=') else {
        return Err(NotionliError::Validation {
            message: format!("Expected KEY=VALUE assignment, got {input}"),
        });
    };
    Ok((key.trim().to_string(), unquote(value.trim())))
}

pub(crate) fn unquote(value: &str) -> String {
    value.trim_matches('"').trim_matches('\'').to_string()
}

pub(crate) fn sql_escape(value: &str) -> String {
    value.replace('\'', "''")
}

pub(crate) fn sql_nullable(value: Option<&str>) -> String {
    value
        .map(|v| format!("'{}'", sql_escape(v)))
        .unwrap_or_else(|| "NULL".into())
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn operation_id() -> String {
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    let nanos = Utc::now().timestamp_subsec_nanos();
    format!("op_{ts}_{:04x}", nanos & 0xffff)
}

pub(crate) fn approx_tokens(value: &Value) -> usize {
    serde_json::to_string(value)
        .map(|s| (s.len() / 4).max(1))
        .unwrap_or(1)
}

pub(crate) fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", shell_escape(name)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub(crate) fn run_shell_capture(cmd: &str) -> Result<String, NotionliError> {
    let output = Command::new("sh").arg("-c").arg(cmd).output()?;
    if !output.status.success() {
        return Err(NotionliError::Auth {
            message: "token-cmd failed".into(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn default_home() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("notionli")
}

pub(crate) fn default_config_home() -> PathBuf {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("notionli");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".config").join("notionli");
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("notionli")
}

pub(crate) fn ensure_home(requested: PathBuf) -> Result<PathBuf, NotionliError> {
    match fs::create_dir_all(&requested) {
        Ok(()) => Ok(requested),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            let local = env::current_dir()?.join(".notionli");
            fs::create_dir_all(&local)?;
            Ok(local)
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn extract_notion_id(url: &str) -> Option<String> {
    let compact = url
        .rsplit(['/', '?', '#'])
        .find(|part| part.len() >= 32)
        .unwrap_or(url);
    let hex = compact
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if hex.len() >= 32 {
        Some(normalize_uuidish(&hex[hex.len() - 32..]))
    } else {
        None
    }
}

pub(crate) fn normalize_uuidish(input: &str) -> String {
    let clean = input.trim().trim_matches('/').to_string();
    let hex = clean
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if clean.contains('-') || hex.len() != 32 {
        return clean;
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub(crate) fn looks_like_uuid(value: &str) -> bool {
    let hex = value.chars().filter(|ch| ch.is_ascii_hexdigit()).count();
    hex == 32
}

pub(crate) fn looks_like_date(value: &str) -> bool {
    value.len() == 10
        && value.chars().nth(4) == Some('-')
        && value.chars().nth(7) == Some('-')
        && value
            .chars()
            .enumerate()
            .all(|(i, ch)| i == 4 || i == 7 || ch.is_ascii_digit())
}

pub(crate) fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub(crate) fn api_message(value: &Value) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
        })
        .unwrap_or("Notion API request failed")
        .to_string()
}

pub(crate) fn list_named_files(dir: &Path) -> Result<Value, NotionliError> {
    fs::create_dir_all(dir)?;
    let mut items = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            items.push(json!({
                "name": entry.path().file_stem().and_then(OsStr::to_str).unwrap_or_default(),
                "path": entry.path(),
            }));
        }
    }
    Ok(json!({ "items": items }))
}
