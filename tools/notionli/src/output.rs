use std::time::Instant;

use chrono::Utc;
use serde_json::{json, Map, Value};

use crate::cli::Cli;
use crate::context::Context;
use crate::error::NotionliError;
use crate::util::approx_tokens;

#[derive(Clone)]
pub(crate) struct OutputOptions {
    json: bool,
    jsonl: bool,
    format: Option<String>,
    quiet: bool,
}

impl OutputOptions {
    pub(crate) fn from_cli(cli: &Cli) -> Self {
        Self {
            json: cli.json,
            jsonl: cli.jsonl,
            format: cli.format.clone(),
            quiet: cli.quiet,
        }
    }

    fn from_context(ctx: &Context) -> Self {
        Self {
            json: ctx.json,
            jsonl: ctx.jsonl,
            format: ctx.format.clone(),
            quiet: ctx.quiet,
        }
    }

    fn format_name(&self) -> Option<String> {
        self.format.as_deref().map(str::to_ascii_lowercase)
    }
}

pub(crate) fn exit_ok(mut value: Value, command: &str, ctx: &Context) -> ! {
    if !value.is_object() {
        value = json!({ "data": value });
    }
    if let Some(map) = value.as_object_mut() {
        let approx = approx_tokens(&Value::Object(map.clone()));
        map.entry("ok").or_insert(Value::Bool(true));
        map.entry("command")
            .or_insert(Value::String(command.into()));
        map.entry("_meta").or_insert_with(|| {
            json!({
                "approx_tokens": approx,
                "elapsed_ms": ctx.started_at.elapsed().as_millis() as u64,
            })
        });
    }
    print_success(&value, &OutputOptions::from_context(ctx));
    std::process::exit(0);
}

fn print_success(value: &Value, output: &OutputOptions) {
    let format = output.format_name();
    if output.quiet || matches!(format.as_deref(), Some("quiet")) {
        if let Some(id) = primary_id(value) {
            println!("{id}");
        }
        return;
    }
    if output.jsonl || matches!(format.as_deref(), Some("jsonl") | Some("ndjson")) {
        print_jsonl(value);
        return;
    }
    if matches!(format.as_deref(), Some("table")) {
        if let Some(table) = render_table(value) {
            println!("{table}");
            return;
        }
    }
    if output.json
        || matches!(
            format.as_deref(),
            Some("json") | Some("compact") | Some("agent") | Some("agent-safe")
        )
    {
        println!("{}", serde_json::to_string(value).unwrap());
        return;
    }
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

fn primary_id(value: &Value) -> Option<&str> {
    value
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| value.get("object_id").and_then(Value::as_str))
        .or_else(|| value.get("operation_id").and_then(Value::as_str))
        .or_else(|| value.get("file_upload_id").and_then(Value::as_str))
        .or_else(|| value.get("webhook_id").and_then(Value::as_str))
        .or_else(|| nested_id(value, "result"))
        .or_else(|| nested_id(value, "target"))
        .or_else(|| nested_id(value, "page"))
        .or_else(|| nested_id(value, "block"))
        .or_else(|| nested_id(value, "row"))
        .or_else(|| nested_id(value, "data"))
        .or_else(|| nested_id(value, "receipt"))
}

fn nested_id<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|nested| {
        nested
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| nested.get("object_id").and_then(Value::as_str))
            .or_else(|| nested.get("operation_id").and_then(Value::as_str))
            .or_else(|| nested.get("file_upload_id").and_then(Value::as_str))
            .or_else(|| nested.get("webhook_id").and_then(Value::as_str))
    })
}

fn print_jsonl(value: &Value) {
    if let Some(items) = stream_items(value) {
        for item in items {
            println!("{}", serde_json::to_string(item).unwrap());
        }
        return;
    }
    println!("{}", serde_json::to_string(value).unwrap());
}

fn stream_items(value: &Value) -> Option<&Vec<Value>> {
    for key in [
        "results",
        "items",
        "entries",
        "rows",
        "objects",
        "changes",
        "operations",
        "receipts",
        "errors",
        "tools",
        "commands",
    ] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            return Some(items);
        }
    }
    value.get("data").and_then(Value::as_array)
}

fn render_table(value: &Value) -> Option<String> {
    let rows = stream_items(value)?;
    if rows.is_empty() {
        return Some(String::new());
    }
    let mut columns = Vec::new();
    for row in rows.iter().filter_map(Value::as_object) {
        for key in row.keys() {
            if columns.len() >= 8 {
                break;
            }
            if !columns.contains(key) {
                columns.push(key.clone());
            }
        }
    }
    if columns.is_empty() {
        return None;
    }
    let mut widths = columns
        .iter()
        .map(|column| column.len())
        .collect::<Vec<_>>();
    let rendered_rows = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .enumerate()
                .map(|(index, column)| {
                    let cell = row
                        .get(column)
                        .map(table_cell)
                        .unwrap_or_default()
                        .replace('\n', " ");
                    widths[index] = widths[index].max(cell.len()).min(80);
                    cell
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    lines.push(
        columns
            .iter()
            .enumerate()
            .map(|(index, column)| pad_cell(column, widths[index]))
            .collect::<Vec<_>>()
            .join("  "),
    );
    lines.push(
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  "),
    );
    for row in rendered_rows {
        lines.push(
            row.iter()
                .enumerate()
                .map(|(index, cell)| pad_cell(cell, widths[index]))
                .collect::<Vec<_>>()
                .join("  "),
        );
    }
    Some(lines.join("\n"))
}

fn table_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(items) => format!("[{}]", items.len()),
        Value::Object(_) => "{...}".into(),
    }
}

fn pad_cell(value: &str, width: usize) -> String {
    let truncated = if value.len() > width {
        value
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            + "~"
    } else {
        value.to_string()
    };
    format!("{truncated:width$}")
}

pub(crate) fn exit_error(
    error: NotionliError,
    command: &str,
    started_at: Instant,
    output: OutputOptions,
) -> ! {
    let mut detail = Map::new();
    detail.insert("code".into(), Value::String(error.code().into()));
    detail.insert("message".into(), Value::String(error.to_string()));
    if let Some(fix) = error.suggested_fix() {
        detail.insert("suggested_fix".into(), Value::String(fix.into()));
    }
    detail.insert(
        "correlation_id".into(),
        Value::String(format!(
            "nli_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        )),
    );
    for (key, value) in error.extra() {
        detail.insert(key, value);
    }
    let envelope = json!({
        "ok": false,
        "command": command,
        "error": detail,
        "_meta": {
            "elapsed_ms": started_at.elapsed().as_millis() as u64,
        }
    });
    print_error(&envelope, &output);
    std::process::exit(error.exit_code());
}

fn print_error(value: &Value, output: &OutputOptions) {
    let format = output.format_name();
    if output.jsonl || matches!(format.as_deref(), Some("jsonl") | Some("ndjson")) {
        eprintln!("{}", serde_json::to_string(value).unwrap());
        return;
    }
    if output.json
        || output.quiet
        || matches!(
            format.as_deref(),
            Some("json") | Some("compact") | Some("agent") | Some("agent-safe")
        )
    {
        eprintln!("{}", serde_json::to_string(value).unwrap());
        return;
    }
    eprintln!("{}", serde_json::to_string_pretty(value).unwrap());
}
