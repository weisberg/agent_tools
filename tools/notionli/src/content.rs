use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use serde_json::{json, Map, Value};

use crate::context::Context;
use crate::error::NotionliError;
use crate::notion::run_block_children;
use crate::resolve::ResolvedTarget;
use crate::util::{looks_like_date, normalize_uuidish, split_assignment};

pub(crate) fn properties_from_sets_with_schema(
    sets: Vec<String>,
    schema: Option<&Value>,
) -> Result<Value, NotionliError> {
    let mut map = Map::new();
    for assignment in sets {
        let (name, value) = split_assignment(&assignment)?;
        let property_schema = schema
            .and_then(Value::as_object)
            .and_then(|properties| properties.get(&name));
        map.insert(
            name.clone(),
            typed_property_value(&name, &value, property_schema)?,
        );
    }
    Ok(Value::Object(map))
}

pub(crate) fn title_properties(title: &str, properties: Value) -> Value {
    title_properties_with_schema(title, properties, None)
}

pub(crate) fn title_properties_with_schema(
    title: &str,
    mut properties: Value,
    schema: Option<&Value>,
) -> Value {
    let map = properties
        .as_object_mut()
        .expect("properties_from_sets returns object");
    if !map.values().any(|property| property.get("title").is_some()) {
        if let Some(title_name) = title_property_name(schema) {
            map.insert(
                title_name,
                json!({ "title": [{ "type": "text", "text": { "content": title } }] }),
            );
        } else if !map.contains_key("Name") && !map.contains_key("Title") {
            map.insert(
                "Name".into(),
                json!({ "title": [{ "type": "text", "text": { "content": title } }] }),
            );
        }
    }
    properties
}

pub(crate) fn page_create_properties(
    title: &str,
    properties: Value,
    parent: &ResolvedTarget,
    schema: Option<&Value>,
) -> Result<Value, NotionliError> {
    if parent.object_type == "page" {
        if !properties.as_object().map(Map::is_empty).unwrap_or(true) {
            return Err(NotionliError::Validation {
                message: "--set properties are only supported when creating pages inside databases or data sources.".into(),
            });
        }
        return Ok(page_title_property(title));
    }
    Ok(title_properties_with_schema(title, properties, schema))
}

pub(crate) fn page_update_title_properties(
    title: &str,
    page: Option<&Value>,
    properties: Value,
    schema: Option<&Value>,
) -> Value {
    let is_plain_page = page
        .and_then(|page| page.get("parent"))
        .and_then(|parent| parent.get("type"))
        .and_then(Value::as_str)
        == Some("page_id");
    if is_plain_page {
        let mut map = properties.as_object().cloned().unwrap_or_default();
        map.insert("title".into(), page_title_value(title));
        return Value::Object(map);
    }
    title_properties_with_schema(title, properties, schema)
}

fn page_title_property(title: &str) -> Value {
    json!({ "title": page_title_value(title) })
}

fn page_title_value(title: &str) -> Value {
    json!([{ "type": "text", "text": { "content": title } }])
}

pub(crate) fn property_value(value: &str) -> Value {
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        return json!({ "checkbox": value.eq_ignore_ascii_case("true") });
    }
    if let Ok(number) = value.parse::<f64>() {
        return json!({ "number": number });
    }
    if looks_like_date(value) || value == "today" {
        let date = if value == "today" {
            Utc::now().date_naive().to_string()
        } else {
            value.to_string()
        };
        return json!({ "date": { "start": date } });
    }
    json!({ "rich_text": [{ "type": "text", "text": { "content": value } }] })
}

pub(crate) fn title_property_name(schema: Option<&Value>) -> Option<String> {
    schema.and_then(Value::as_object).and_then(|properties| {
        properties.iter().find_map(|(name, property)| {
            (property.get("type").and_then(Value::as_str) == Some("title")).then(|| name.clone())
        })
    })
}

fn typed_property_value(
    name: &str,
    value: &str,
    property_schema: Option<&Value>,
) -> Result<Value, NotionliError> {
    let Some(property_schema) = property_schema else {
        return Ok(property_value(value));
    };
    match property_schema
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "title" => Ok(json!({ "title": rich_text(value) })),
        "rich_text" => Ok(json!({ "rich_text": rich_text(value) })),
        "number" => Ok(json!({ "number": parse_optional_number(name, value)? })),
        "checkbox" => Ok(json!({ "checkbox": parse_bool(name, value)? })),
        "date" => Ok(json!({ "date": parse_date_value(value) })),
        "select" => {
            if value.is_empty() {
                Ok(json!({ "select": null }))
            } else {
                validate_option_name(name, value, property_schema, "select")?;
                Ok(json!({ "select": { "name": value } }))
            }
        }
        "status" => {
            if value.is_empty() {
                Ok(json!({ "status": null }))
            } else {
                validate_option_name(name, value, property_schema, "status")?;
                Ok(json!({ "status": { "name": value } }))
            }
        }
        "multi_select" => {
            let values = split_list(value);
            validate_option_names(name, &values, property_schema, "multi_select")?;
            Ok(json!({
                "multi_select": values
                    .into_iter()
                    .map(|item| json!({ "name": item }))
                    .collect::<Vec<_>>()
            }))
        }
        "relation" => Ok(json!({
            "relation": split_list(value)
                .into_iter()
                .map(|item| json!({ "id": normalize_relation_id(&item) }))
                .collect::<Vec<_>>()
        })),
        "people" => Ok(json!({
            "people": split_list(value)
                .into_iter()
                .map(|item| json!({ "id": normalize_relation_id(&item) }))
                .collect::<Vec<_>>()
        })),
        "url" => Ok(json!({ "url": optional_string(value) })),
        "email" => Ok(json!({ "email": optional_string(value) })),
        "phone_number" => Ok(json!({ "phone_number": optional_string(value) })),
        "files" => Err(NotionliError::Validation {
            message: format!(
                "Property `{name}` has type files; use `notionli file attach` instead of --set."
            ),
        }),
        "formula" | "rollup" | "created_time" | "created_by" | "last_edited_time"
        | "last_edited_by" | "unique_id" => Err(NotionliError::Validation {
            message: format!("Property `{name}` is read-only and cannot be set."),
        }),
        _ => Ok(property_value(value)),
    }
}

fn parse_optional_number(name: &str, value: &str) -> Result<Value, NotionliError> {
    if value.is_empty() {
        return Ok(Value::Null);
    }
    let number = value
        .parse::<f64>()
        .map_err(|_| NotionliError::Validation {
            message: format!("Property `{name}` expects a number, got `{value}`."),
        })?;
    Ok(json!(number))
}

fn parse_bool(name: &str, value: &str) -> Result<bool, NotionliError> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" => Ok(true),
        "false" | "no" | "n" | "0" => Ok(false),
        _ => Err(NotionliError::Validation {
            message: format!("Property `{name}` expects a boolean, got `{value}`."),
        }),
    }
}

fn parse_date_value(value: &str) -> Value {
    if value.is_empty() {
        return Value::Null;
    }
    let start = if value == "today" {
        Utc::now().date_naive().to_string()
    } else {
        value.to_string()
    };
    json!({ "start": start })
}

fn optional_string(value: &str) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value.to_string())
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn validate_option_names(
    property_name: &str,
    values: &[String],
    property_schema: &Value,
    schema_key: &str,
) -> Result<(), NotionliError> {
    for value in values {
        validate_option_name(property_name, value, property_schema, schema_key)?;
    }
    Ok(())
}

fn validate_option_name(
    property_name: &str,
    value: &str,
    property_schema: &Value,
    schema_key: &str,
) -> Result<(), NotionliError> {
    let options = option_names(property_schema, schema_key);
    if options.is_empty() || options.iter().any(|option| option == value) {
        return Ok(());
    }
    Err(NotionliError::Validation {
        message: format!(
            "Property `{property_name}` option `{value}` is not available. Available options: {}.",
            options.join(", ")
        ),
    })
}

fn option_names(property_schema: &Value, schema_key: &str) -> Vec<String> {
    property_schema
        .get(schema_key)
        .and_then(|typed| typed.get("options"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_relation_id(value: &str) -> String {
    normalize_uuidish(value.split_once(':').map(|(_, id)| id).unwrap_or(value))
}

pub(crate) fn parent_payload(parent: &ResolvedTarget) -> Value {
    match parent.object_type.as_str() {
        "database" => json!({ "database_id": parent.id }),
        "data_source" => json!({ "data_source_id": parent.id }),
        _ => json!({ "page_id": parent.id }),
    }
}

pub(crate) fn markdown_to_blocks(markdown: &str) -> Vec<Value> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if let Some((table, consumed)) = markdown_table_block(&lines[index..]) {
            blocks.push(table);
            index += consumed;
            continue;
        }
        blocks.push(markdown_line_to_block(trimmed));
        index += 1;
    }
    blocks
}

fn markdown_line_to_block(trimmed: &str) -> Value {
    if let Some(text) = trimmed.strip_prefix("### ") {
        block("heading_3", text)
    } else if let Some(text) = trimmed.strip_prefix("## ") {
        block("heading_2", text)
    } else if let Some(text) = trimmed.strip_prefix("# ") {
        block("heading_1", text)
    } else if let Some(text) = trimmed.strip_prefix("- [ ] ") {
        json!({ "object": "block", "type": "to_do", "to_do": { "rich_text": rich_text(text), "checked": false } })
    } else if let Some(text) = trimmed.strip_prefix("- [x] ") {
        json!({ "object": "block", "type": "to_do", "to_do": { "rich_text": rich_text(text), "checked": true } })
    } else if let Some(text) = trimmed.strip_prefix("- ") {
        block("bulleted_list_item", text)
    } else {
        block("paragraph", trimmed)
    }
}

fn markdown_table_block(lines: &[&str]) -> Option<(Value, usize)> {
    if lines.len() < 2 || !is_table_separator(lines[1]) {
        return None;
    }
    let header = parse_table_row(lines[0])?;
    if header.is_empty() {
        return None;
    }
    let mut rows = vec![header];
    let mut consumed = 2;
    while consumed < lines.len() {
        let line = lines[consumed].trim();
        if line.is_empty() || !line.contains('|') {
            break;
        }
        let row = parse_table_row(line)?;
        if row.is_empty() {
            break;
        }
        rows.push(row);
        consumed += 1;
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return None;
    }
    let children = rows
        .into_iter()
        .map(|row| {
            let cells = (0..width)
                .map(|index| rich_text(row.get(index).map(String::as_str).unwrap_or("")))
                .collect::<Vec<_>>();
            json!({ "object": "block", "type": "table_row", "table_row": { "cells": cells } })
        })
        .collect::<Vec<_>>();
    Some((
        json!({
            "object": "block",
            "type": "table",
            "table": {
                "table_width": width,
                "has_column_header": true,
                "has_row_header": false,
                "children": children,
            }
        }),
        consumed,
    ))
}

fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim().trim_matches('|');
    let cells = trimmed
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect::<Vec<_>>();
    if cells.is_empty() {
        None
    } else {
        Some(cells)
    }
}

fn is_table_separator(line: &str) -> bool {
    let Some(cells) = parse_table_row(line) else {
        return false;
    };
    cells.iter().all(|cell| {
        let trimmed = cell.trim();
        trimmed.len() >= 3
            && trimmed
                .chars()
                .all(|character| matches!(character, '-' | ':' | ' '))
            && trimmed.chars().any(|character| character == '-')
    })
}

pub(crate) fn block(kind: &str, text: &str) -> Value {
    json!({ "object": "block", "type": kind, kind: { "rich_text": rich_text(text) } })
}

pub(crate) fn rich_text(text: &str) -> Value {
    json!([{ "type": "text", "text": { "content": text } }])
}

pub(crate) fn block_update_payload(markdown: &str) -> Value {
    let blocks = markdown_to_blocks(markdown);
    blocks
        .into_iter()
        .next()
        .unwrap_or_else(|| block("paragraph", ""))
}

pub(crate) fn blocks_to_markdown(value: &Value) -> String {
    let mut out = String::new();
    if let Some(results) = value.get("results").and_then(Value::as_array) {
        for block in results {
            out.push_str(&block_to_markdown(block));
            out.push('\n');
        }
    }
    out
}

pub(crate) fn block_to_markdown(block: &Value) -> String {
    let kind = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("paragraph");
    let text = block
        .get(kind)
        .and_then(|v| v.get("rich_text"))
        .map(rich_text_plain)
        .unwrap_or_default();
    match kind {
        "heading_1" => format!("# {text}"),
        "heading_2" => format!("## {text}"),
        "heading_3" => format!("### {text}"),
        "bulleted_list_item" => format!("- {text}"),
        "numbered_list_item" => format!("1. {text}"),
        "to_do" => {
            let checked = block
                .get(kind)
                .and_then(|v| v.get("checked"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            format!("- [{}] {text}", if checked { "x" } else { " " })
        }
        _ => text,
    }
}

pub(crate) fn rich_text_plain(value: &Value) -> String {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("plain_text").and_then(Value::as_str).or_else(|| {
                        item.get("text")
                            .and_then(|t| t.get("content"))
                            .and_then(Value::as_str)
                    })
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}
pub(crate) fn extract_section(
    markdown: &str,
    heading: &str,
    include_subsections: bool,
) -> Result<String, NotionliError> {
    let mut capture = false;
    let mut level = 0usize;
    let mut out = Vec::new();
    for line in markdown.lines() {
        if let Some((line_level, text)) = heading_line(line) {
            if capture && line_level <= level && (!include_subsections || line_level == level) {
                break;
            }
            if text.eq_ignore_ascii_case(heading) {
                capture = true;
                level = line_level;
                out.push(line.to_string());
                continue;
            }
        }
        if capture {
            out.push(line.to_string());
        }
    }
    if out.is_empty() {
        return Err(NotionliError::NotFound {
            message: format!("Heading not found: {heading}"),
        });
    }
    Ok(out.join("\n"))
}

pub(crate) fn extract_outline(markdown: &str, _with_block_ids: bool) -> Vec<Value> {
    markdown
        .lines()
        .filter_map(|line| {
            heading_line(line).map(|(level, text)| json!({ "level": level, "text": text }))
        })
        .collect()
}

pub(crate) fn heading_line(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
        Some((hashes, trimmed[hashes + 1..].trim()))
    } else {
        None
    }
}

pub(crate) fn block_extract(
    ctx: &Context,
    target: &str,
    kind: &str,
) -> Result<Value, NotionliError> {
    let value = run_block_children(target, 5, ctx)?;
    let mut hits = Vec::new();
    collect_block_matches(&value, None, Some(kind), None, &mut hits);
    Ok(json!({ "target": target, "matches": hits }))
}

pub(crate) fn collect_block_matches(
    value: &Value,
    text: Option<&str>,
    kind: Option<&str>,
    heading: Option<&str>,
    hits: &mut Vec<Value>,
) {
    if let Some(results) = value.get("results").and_then(Value::as_array) {
        for item in results {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            let plain = block_to_markdown(item);
            let type_ok = kind.map(|k| k == item_type).unwrap_or(true);
            let text_ok = text
                .map(|needle| plain.to_lowercase().contains(&needle.to_lowercase()))
                .unwrap_or(true);
            let heading_ok = heading
                .map(|needle| {
                    plain
                        .trim_start_matches('#')
                        .trim()
                        .eq_ignore_ascii_case(needle)
                })
                .unwrap_or(true);
            if type_ok && text_ok && heading_ok {
                hits.push(item.clone());
            }
            collect_block_matches(
                item.get("children").unwrap_or(&Value::Null),
                text,
                kind,
                heading,
                hits,
            );
        }
    }
}

pub(crate) fn read_body(
    path: Option<&PathBuf>,
    text: Option<&str>,
) -> Result<String, NotionliError> {
    if let Some(path) = path {
        return Ok(fs::read_to_string(path)?);
    }
    Ok(text.unwrap_or_default().to_string())
}

pub(crate) fn h1_title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(|s| s.trim().to_string()))
}

pub(crate) fn apply_markdown_budget(markdown: &str, budget: Option<u32>) -> String {
    let Some(budget) = budget else {
        return markdown.to_string();
    };
    let max_chars = budget as usize * 4;
    if markdown.len() <= max_chars {
        markdown.to_string()
    } else {
        format!(
            "{}\n\n<!-- notionli: truncated by local budget -->",
            &markdown[..max_chars]
        )
    }
}

pub(crate) fn extract_actions_from_text(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line
                .trim()
                .trim_start_matches('-')
                .trim_start_matches('*')
                .trim();
            let action = trimmed
                .strip_prefix("[ ]")
                .or_else(|| trimmed.strip_prefix("TODO:"))
                .or_else(|| trimmed.strip_prefix("Action:"))
                .or_else(|| trimmed.strip_prefix("Action item:"))
                .map(str::trim)?;
            if action.is_empty() {
                return None;
            }
            Some(json!({
                "text": action,
                "done": false,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::markdown_to_blocks;

    #[test]
    fn markdown_to_blocks_parses_pipe_tables() {
        let blocks = markdown_to_blocks(
            r#"# Demo

| Name | Status | Owner |
| --- | --- | --- |
| Import | Done | notionli |
| Verify | Next | user |
"#,
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1]["type"], "table");
        assert_eq!(blocks[1]["table"]["table_width"], 3);
        assert_eq!(blocks[1]["table"]["has_column_header"], true);
        let rows = blocks[1]["table"]["children"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0]["table_row"]["cells"][0][0]["text"]["content"],
            "Name"
        );
        assert_eq!(
            rows[2]["table_row"]["cells"][2][0]["text"]["content"],
            "user"
        );
    }
}
