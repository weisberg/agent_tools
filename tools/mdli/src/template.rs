use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::*;

pub(crate) fn run_template(cmd: TemplateCommand) -> Result<Outcome, MdliError> {
    match cmd {
        TemplateCommand::Render(args) => {
            let template = read_text_path(&args.template)?;
            let datasets = parse_data_args(&args.data)?;
            let rendered = render_template(&template, &datasets)?;
            Ok(Outcome::Text(rendered))
        }
    }
}

pub(crate) fn parse_data_args(entries: &[String]) -> Result<BTreeMap<String, Value>, MdliError> {
    let mut datasets = BTreeMap::new();
    for entry in entries {
        let (name, path) = entry.split_once('=').ok_or_else(|| {
            MdliError::user(
                "E_TEMPLATE_PARSE",
                format!("--data entry must be NAME=PATH, got {entry}"),
            )
        })?;
        let value = load_dataset(Path::new(path))?;
        datasets.insert(name.to_string(), value);
    }
    Ok(datasets)
}

pub(crate) fn load_dataset(path: &Path) -> Result<Value, MdliError> {
    let text = fs::read_to_string(path).map_err(|e| {
        MdliError::io(
            "E_READ_FAILED",
            format!("failed to read dataset {}", path.display()),
            e,
        )
    })?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    if trimmed.starts_with('[') || (trimmed.starts_with('{') && !looks_like_ndjson(trimmed)) {
        return serde_json::from_str(trimmed).map_err(|e| {
            MdliError::user(
                "E_ROW_INPUT_INVALID",
                format!("invalid JSON in dataset {}: {e}", path.display()),
            )
        });
    }
    if trimmed.starts_with('{') {
        let mut rows = Vec::new();
        for (idx, line) in trimmed.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line).map_err(|e| {
                MdliError::user(
                    "E_ROW_INPUT_INVALID",
                    format!(
                        "invalid NDJSON at line {} of {}: {e}",
                        idx + 1,
                        path.display()
                    ),
                )
            })?;
            rows.push(value);
        }
        return Ok(Value::Array(rows));
    }
    // Try parsing the entire file as a JSON scalar (string, number, boolean, null).
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if !matches!(value, Value::Object(_) | Value::Array(_)) {
            return Ok(value);
        }
    }
    Ok(Value::String(trimmed.to_string()))
}

fn looks_like_ndjson(s: &str) -> bool {
    let mut lines = s.lines().filter(|l| !l.trim().is_empty());
    let first = match lines.next() {
        Some(l) => l.trim(),
        None => return false,
    };
    if !first.starts_with('{') || !first.ends_with('}') {
        return false;
    }
    lines.next().is_some()
}

pub(crate) fn render_template(
    template: &str,
    datasets: &BTreeMap<String, Value>,
) -> Result<String, MdliError> {
    let nodes = parse_template(template)?;
    let mut out = String::new();
    render_nodes(&nodes, datasets, &mut out)?;
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

#[derive(Debug)]
pub(crate) enum TemplateNode {
    Text(String),
    Value(String),
    Table(TableHelper),
    IfPresent {
        key: String,
        body: Vec<TemplateNode>,
    },
}

#[derive(Debug)]
pub(crate) struct TableHelper {
    pub(crate) dataset: String,
    pub(crate) columns: Vec<String>,
    pub(crate) key: Option<String>,
    pub(crate) sort: Option<String>,
    pub(crate) truncates: BTreeMap<String, usize>,
    pub(crate) links: BTreeMap<String, String>,
    pub(crate) empty: Option<String>,
    pub(crate) name: Option<String>,
}

pub(crate) fn parse_template(src: &str) -> Result<Vec<TemplateNode>, MdliError> {
    let (nodes, rest) = parse_nodes(src, false)?;
    if !rest.is_empty() {
        return Err(MdliError::user(
            "E_TEMPLATE_PARSE",
            "unexpected trailing template content",
        ));
    }
    Ok(nodes)
}

fn parse_nodes<'a>(
    src: &'a str,
    inside_block: bool,
) -> Result<(Vec<TemplateNode>, &'a str), MdliError> {
    let mut nodes = Vec::new();
    let mut rest = src;
    loop {
        if rest.is_empty() {
            if inside_block {
                return Err(MdliError::user(
                    "E_TEMPLATE_PARSE",
                    "unterminated {{ if_present }} block",
                ));
            }
            break;
        }
        if let Some(idx) = rest.find("{{") {
            if idx > 0 {
                nodes.push(TemplateNode::Text(rest[..idx].to_string()));
                rest = &rest[idx..];
            }
            // Locate end of helper.
            let close = rest
                .find("}}")
                .ok_or_else(|| MdliError::user("E_TEMPLATE_PARSE", "missing closing }}"))?;
            let helper_text = rest[2..close].trim();
            rest = &rest[close + 2..];
            if helper_text == "end" {
                if inside_block {
                    return Ok((nodes, rest));
                }
                return Err(MdliError::user(
                    "E_TEMPLATE_PARSE",
                    "unexpected {{ end }} outside an {{ if_present }} block",
                ));
            }
            if let Some(stripped) = helper_text.strip_prefix("if_present") {
                let key = stripped.trim();
                if key.is_empty() {
                    return Err(MdliError::user(
                        "E_TEMPLATE_PARSE",
                        "{{ if_present }} requires a dataset name",
                    ));
                }
                let (body, new_rest) = parse_nodes(rest, true)?;
                rest = new_rest;
                nodes.push(TemplateNode::IfPresent {
                    key: key.to_string(),
                    body,
                });
                continue;
            }
            if let Some(stripped) = helper_text.strip_prefix("value") {
                let key = stripped.trim();
                if key.is_empty() {
                    return Err(MdliError::user(
                        "E_TEMPLATE_PARSE",
                        "{{ value }} requires a key",
                    ));
                }
                nodes.push(TemplateNode::Value(key.to_string()));
                continue;
            }
            if let Some(stripped) = helper_text.strip_prefix("table") {
                let helper = parse_table_helper(stripped.trim())?;
                nodes.push(TemplateNode::Table(helper));
                continue;
            }
            return Err(MdliError::user(
                "E_TEMPLATE_UNKNOWN_HELPER",
                format!("unknown helper {helper_text}"),
            ));
        } else {
            nodes.push(TemplateNode::Text(rest.to_string()));
            rest = "";
        }
    }
    Ok((nodes, rest))
}

fn parse_table_helper(src: &str) -> Result<TableHelper, MdliError> {
    let mut chars = src.chars().peekable();
    let mut dataset = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            break;
        }
        dataset.push(c);
        chars.next();
    }
    if dataset.is_empty() {
        return Err(MdliError::user(
            "E_TEMPLATE_PARSE",
            "{{ table }} requires a dataset name",
        ));
    }
    let mut helper = TableHelper {
        dataset,
        columns: Vec::new(),
        key: None,
        sort: None,
        truncates: BTreeMap::new(),
        links: BTreeMap::new(),
        empty: None,
        name: None,
    };
    let rest = chars.collect::<String>();
    let kv_pairs = split_helper_kv(&rest)?;
    for (key, value) in kv_pairs {
        match key.as_str() {
            "columns" => {
                helper.columns = parse_string_list(&value)?;
            }
            "key" => helper.key = Some(unquote_scalar(&value)),
            "sort" => {
                let parts = parse_string_list(&value)?;
                if !parts.is_empty() {
                    helper.sort = Some(parts.join(","));
                }
            }
            "truncate" => {
                helper.truncates = parse_object_usize(&value)?;
            }
            "link" => {
                helper.links = parse_object_string(&value)?;
            }
            "empty" => helper.empty = Some(unquote_scalar(&value)),
            "name" => helper.name = Some(unquote_scalar(&value)),
            other => {
                return Err(MdliError::user(
                    "E_TEMPLATE_PARSE",
                    format!("{{ table }} does not support attribute {other}"),
                ));
            }
        }
    }
    Ok(helper)
}

fn split_helper_kv(src: &str) -> Result<Vec<(String, String)>, MdliError> {
    let mut pairs = Vec::new();
    let mut chars = src.chars().peekable();
    loop {
        while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' {
                chars.next();
                break;
            }
            if c.is_whitespace() {
                return Err(MdliError::user(
                    "E_TEMPLATE_PARSE",
                    "expected = after key in {{ table }}",
                ));
            }
            key.push(c);
            chars.next();
        }
        let value = read_helper_value(&mut chars)?;
        pairs.push((key, value));
    }
    Ok(pairs)
}

fn read_helper_value(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<String, MdliError> {
    let mut depth_brackets = 0i32;
    let mut depth_braces = 0i32;
    let mut value = String::new();
    let mut in_string: Option<char> = None;
    while let Some(&c) = chars.peek() {
        if let Some(quote) = in_string {
            value.push(c);
            chars.next();
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    value.push(next);
                    chars.next();
                }
                continue;
            }
            if c == quote {
                in_string = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                in_string = Some(c);
                value.push(c);
                chars.next();
            }
            '[' => {
                depth_brackets += 1;
                value.push(c);
                chars.next();
            }
            ']' => {
                depth_brackets -= 1;
                value.push(c);
                chars.next();
                if depth_brackets == 0 && depth_braces == 0 {
                    break;
                }
            }
            '{' => {
                depth_braces += 1;
                value.push(c);
                chars.next();
            }
            '}' => {
                depth_braces -= 1;
                value.push(c);
                chars.next();
                if depth_brackets == 0 && depth_braces == 0 {
                    break;
                }
            }
            _ if c.is_whitespace() && depth_brackets == 0 && depth_braces == 0 => {
                break;
            }
            _ => {
                value.push(c);
                chars.next();
            }
        }
    }
    if depth_brackets != 0 || depth_braces != 0 || in_string.is_some() {
        return Err(MdliError::user(
            "E_TEMPLATE_PARSE",
            "unbalanced delimiters in {{ table }} attribute",
        ));
    }
    Ok(value.trim().to_string())
}

fn parse_string_list(src: &str) -> Result<Vec<String>, MdliError> {
    let trimmed = src.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(MdliError::user(
            "E_TEMPLATE_PARSE",
            "expected [ ... ] for list",
        ));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_string: Option<char> = None;
    for c in inner.chars() {
        if let Some(q) = in_string {
            if c == q {
                in_string = None;
            } else if c != '\\' {
                current.push(c);
            }
            continue;
        }
        match c {
            '"' | '\'' => in_string = Some(c),
            ',' => {
                let s = current.trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }
    let s = current.trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
    Ok(out)
}

fn parse_object_string(src: &str) -> Result<BTreeMap<String, String>, MdliError> {
    let json = json_loose_to_strict(src)?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|e| MdliError::user("E_TEMPLATE_PARSE", format!("invalid object: {e}")))?;
    let map = value
        .as_object()
        .ok_or_else(|| MdliError::user("E_TEMPLATE_PARSE", "expected an object"))?;
    let mut out = BTreeMap::new();
    for (k, v) in map {
        out.insert(
            k.clone(),
            v.as_str()
                .ok_or_else(|| {
                    MdliError::user(
                        "E_TEMPLATE_PARSE",
                        format!("expected string value for key {k}"),
                    )
                })?
                .to_string(),
        );
    }
    Ok(out)
}

fn parse_object_usize(src: &str) -> Result<BTreeMap<String, usize>, MdliError> {
    let json = json_loose_to_strict(src)?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|e| MdliError::user("E_TEMPLATE_PARSE", format!("invalid object: {e}")))?;
    let map = value
        .as_object()
        .ok_or_else(|| MdliError::user("E_TEMPLATE_PARSE", "expected an object"))?;
    let mut out = BTreeMap::new();
    for (k, v) in map {
        out.insert(
            k.clone(),
            v.as_u64().ok_or_else(|| {
                MdliError::user("E_TEMPLATE_PARSE", format!("expected integer for key {k}"))
            })? as usize,
        );
    }
    Ok(out)
}

fn json_loose_to_strict(src: &str) -> Result<String, MdliError> {
    // Accept single-quoted strings and bare keys by passing-through; serde_json is strict.
    // We support the standard double-quoted JSON object form. If callers used single quotes,
    // convert them to double quotes outside of escaped strings.
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                out.push('"');
                while let Some(&n) = chars.peek() {
                    if n == '\\' {
                        out.push(n);
                        chars.next();
                        if let Some(&esc) = chars.peek() {
                            out.push(esc);
                            chars.next();
                        }
                        continue;
                    }
                    if n == '\'' {
                        chars.next();
                        out.push('"');
                        break;
                    }
                    if n == '"' {
                        out.push('\\');
                    }
                    out.push(n);
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    Ok(out)
}

fn unquote_scalar(src: &str) -> String {
    let trimmed = src.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        let inner = &trimmed[1..trimmed.len() - 1];
        inner.replace("\\\"", "\"").replace("\\'", "'")
    } else {
        trimmed.to_string()
    }
}

fn render_nodes(
    nodes: &[TemplateNode],
    datasets: &BTreeMap<String, Value>,
    out: &mut String,
) -> Result<(), MdliError> {
    for node in nodes {
        match node {
            TemplateNode::Text(t) => out.push_str(t),
            TemplateNode::Value(key) => {
                let value = lookup_value(key, datasets).ok_or_else(|| {
                    MdliError::user(
                        "E_TEMPLATE_MISSING_VALUE",
                        format!("template references missing value {key}"),
                    )
                })?;
                out.push_str(&value_to_text(value));
            }
            TemplateNode::Table(helper) => {
                let dataset = datasets.get(&helper.dataset).ok_or_else(|| {
                    MdliError::user(
                        "E_TEMPLATE_MISSING_DATASET",
                        format!("template references missing dataset {}", helper.dataset),
                    )
                })?;
                let rendered = render_table_helper(helper, dataset)?;
                out.push_str(&rendered);
            }
            TemplateNode::IfPresent { key, body } => {
                if lookup_value(key, datasets).is_some() {
                    render_nodes(body, datasets, out)?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn lookup_value<'a>(
    key: &str,
    datasets: &'a BTreeMap<String, Value>,
) -> Option<&'a Value> {
    let mut parts = key.split('.');
    let head = parts.next()?;
    let mut current = datasets.get(head)?;
    for part in parts {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub(crate) fn render_table_helper(
    helper: &TableHelper,
    dataset: &Value,
) -> Result<String, MdliError> {
    let rows: Vec<Map<String, Value>> = match dataset {
        Value::Array(arr) => arr
            .iter()
            .map(|v| value_to_row(v.clone()))
            .collect::<Result<_, _>>()?,
        Value::Object(map) => vec![map.clone()],
        _ => {
            return Err(MdliError::user(
                "E_TEMPLATE_PARSE",
                format!("dataset {} must be an array of objects", helper.dataset),
            ))
        }
    };
    let columns = helper
        .columns
        .iter()
        .map(|spec| parse_columns(spec))
        .collect::<Result<Vec<_>, MdliError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(MdliError::user(
            "E_TEMPLATE_PARSE",
            "{{ table }} requires non-empty columns=",
        ));
    }
    let render_options = RenderTableOptions {
        columns,
        key: helper.key.clone(),
        sort: helper.sort.clone(),
        missing: MissingMode::Empty,
        rich_cell: RichCellMode::Error,
        duplicate_key: DuplicateKeyMode::Error,
        empty: helper.empty.clone(),
        links: helper.links.clone(),
        truncates: helper.truncates.clone(),
        escape_markdown: false,
    };
    let rendered = render_table_from_rows(&rows, &render_options)?;
    let mut lines = Vec::new();
    if let Some(name) = &helper.name {
        lines.push(table_marker(name, helper.key.as_deref()));
    }
    lines.extend(rendered.lines);
    let mut out = lines.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}
