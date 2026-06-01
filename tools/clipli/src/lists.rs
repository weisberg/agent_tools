// Structured list builder/editor for clipboard-ready HTML and Markdown.
//
// The internal representation stays format-neutral so agents can build or edit
// a list once, then choose the clipboard artifact at the end.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub kind: ListKind,
    #[serde(default)]
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ListKind {
    #[default]
    Unordered,
    Ordered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ListItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub font: String,
    pub font_size: String,
    pub class_name: Option<String>,
    pub tight: bool,
    pub include_metadata: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            font: "Calibri".to_string(),
            font_size: "11".to_string(),
            class_name: None,
            tight: false,
            include_metadata: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Auto,
    Json,
    Markdown,
    Lines,
    Html,
}

impl InputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "json" => Ok(Self::Json),
            "markdown" | "md" => Ok(Self::Markdown),
            "lines" | "text" | "plain" => Ok(Self::Lines),
            "html" => Ok(Self::Html),
            other => Err(format!(
                "unsupported list input format '{other}': expected auto, json, markdown, lines, or html"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Html,
    Markdown,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "html" => Ok(Self::Html),
            "markdown" | "md" => Ok(Self::Markdown),
            other => Err(format!(
                "unsupported list output format '{other}': expected html or markdown"
            )),
        }
    }
}

impl ListDocument {
    pub fn new(kind: ListKind) -> Self {
        Self {
            title: None,
            kind,
            items: Vec::new(),
        }
    }

    pub fn item_count(&self) -> usize {
        count_items(&self.items)
    }

    pub fn max_depth(&self) -> usize {
        max_depth(&self.items, 0)
    }

    pub fn add_path_item(&mut self, spec: &str) -> Result<(), String> {
        let parts = spec
            .split('>')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return Err("list --item requires non-empty text".to_string());
        }

        let mut siblings = &mut self.items;
        for part in parts {
            let (text, checked) = parse_checked_text(part);
            let index = siblings
                .iter()
                .position(|item| item.text == text)
                .unwrap_or_else(|| {
                    siblings.push(ListItem {
                        text: text.clone(),
                        items: Vec::new(),
                        checked,
                    });
                    siblings.len() - 1
                });
            if checked.is_some() {
                siblings[index].checked = checked;
            }
            siblings = &mut siblings[index].items;
        }
        Ok(())
    }

    pub fn sort_recursive(&mut self) {
        sort_items(&mut self.items);
    }

    pub fn dedupe_recursive(&mut self) {
        dedupe_items(&mut self.items);
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsonInput {
    Document(JsonDocument),
    Items(Vec<JsonItem>),
}

#[derive(Debug, Deserialize)]
struct JsonDocument {
    title: Option<String>,
    kind: Option<ListKind>,
    ordered: Option<bool>,
    #[serde(default, alias = "children", alias = "subitems", alias = "sub_items")]
    items: Vec<JsonItem>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsonItem {
    Text(String),
    Object(JsonItemObject),
}

#[derive(Debug, Deserialize)]
struct JsonItemObject {
    text: String,
    #[serde(default, alias = "children", alias = "subitems", alias = "sub_items")]
    items: Vec<JsonItem>,
    checked: Option<bool>,
}

impl From<JsonDocument> for ListDocument {
    fn from(value: JsonDocument) -> Self {
        Self {
            title: value.title,
            kind: value.kind.unwrap_or(if value.ordered.unwrap_or(false) {
                ListKind::Ordered
            } else {
                ListKind::Unordered
            }),
            items: value.items.into_iter().map(ListItem::from).collect(),
        }
    }
}

impl From<JsonItem> for ListItem {
    fn from(value: JsonItem) -> Self {
        match value {
            JsonItem::Text(text) => {
                let (text, checked) = parse_checked_text(&text);
                Self {
                    text,
                    checked,
                    items: Vec::new(),
                }
            }
            JsonItem::Object(value) => Self {
                text: value.text,
                checked: value.checked,
                items: value.items.into_iter().map(ListItem::from).collect(),
            },
        }
    }
}

pub fn parse_document(data: &str, format: InputFormat) -> Result<ListDocument, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("list input is empty".to_string());
    }

    let format = match format {
        InputFormat::Auto if trimmed.starts_with('{') || trimmed.starts_with('[') => {
            InputFormat::Json
        }
        InputFormat::Auto if looks_like_html(trimmed) => InputFormat::Html,
        InputFormat::Auto => InputFormat::Markdown,
        other => other,
    };

    match format {
        InputFormat::Json => parse_json(trimmed),
        InputFormat::Markdown | InputFormat::Lines => parse_markdown_or_lines(trimmed),
        InputFormat::Html => parse_html(trimmed),
        InputFormat::Auto => unreachable!("auto list input format should be resolved"),
    }
}

fn parse_json(data: &str) -> Result<ListDocument, String> {
    let parsed: JsonInput =
        serde_json::from_str(data).map_err(|e| format!("could not parse list JSON: {e}"))?;
    let doc = match parsed {
        JsonInput::Document(doc) => ListDocument::from(doc),
        JsonInput::Items(items) => ListDocument {
            title: None,
            kind: ListKind::Unordered,
            items: items.into_iter().map(ListItem::from).collect(),
        },
    };
    validate_non_empty(&doc)?;
    Ok(doc)
}

fn parse_markdown_or_lines(data: &str) -> Result<ListDocument, String> {
    let mut doc = ListDocument::new(ListKind::Unordered);
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut saw_ordered_root = false;
    let mut saw_unordered_root = false;

    for line in data.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if doc.items.is_empty() && doc.title.is_none() {
            if let Some(title) = line.trim().strip_prefix("# ") {
                doc.title = Some(title.trim().to_string());
                continue;
            }
        }

        let parsed = parse_list_line(line);
        if parsed.level == 0 {
            if parsed.ordered {
                saw_ordered_root = true;
            } else if parsed.had_marker {
                saw_unordered_root = true;
            }
        }

        while stack
            .last()
            .map(|(indent, _)| *indent >= parsed.indent)
            .unwrap_or(false)
        {
            stack.pop();
        }
        let parent_path = stack
            .last()
            .map(|(_, path)| path.clone())
            .unwrap_or_default();
        let index = append_child(
            &mut doc.items,
            &parent_path,
            ListItem {
                text: parsed.text,
                items: Vec::new(),
                checked: parsed.checked,
            },
        )?;
        let mut new_path = parent_path;
        new_path.push(index);
        stack.push((parsed.indent, new_path));
    }

    if saw_ordered_root && !saw_unordered_root {
        doc.kind = ListKind::Ordered;
    }
    validate_non_empty(&doc)?;
    Ok(doc)
}

fn parse_html(data: &str) -> Result<ListDocument, String> {
    if let Some(doc) = parse_html_metadata(data)? {
        return Ok(doc);
    }

    let markdownish = html_to_markdownish(data);
    parse_markdown_or_lines(&markdownish)
}

fn parse_html_metadata(data: &str) -> Result<Option<ListDocument>, String> {
    let Some(start) = data.find("<!-- clipli-list-json-hex:") else {
        return Ok(None);
    };
    let hex_start = start + "<!-- clipli-list-json-hex:".len();
    let Some(end) = data[hex_start..].find("-->") else {
        return Err("clipli list metadata comment is not closed".to_string());
    };
    let hex = data[hex_start..hex_start + end].trim();
    let json = hex_decode(hex)?;
    let doc: ListDocument =
        serde_json::from_str(&json).map_err(|e| format!("could not parse list metadata: {e}"))?;
    validate_non_empty(&doc)?;
    Ok(Some(doc))
}

fn html_to_markdownish(data: &str) -> String {
    let mut out = data
        .replace("<li>", "\n- ")
        .replace("<li ", "\n- <li ")
        .replace("</li>", "\n")
        .replace("<ul>", "\n")
        .replace("</ul>", "\n")
        .replace("<ol>", "\n")
        .replace("</ol>", "\n");

    let tag_re = regex::Regex::new(r"(?is)<[^>]+>").unwrap();
    out = tag_re.replace_all(&out, "").to_string();
    decode_basic_entities(&out)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
struct ParsedLine {
    indent: usize,
    level: usize,
    text: String,
    checked: Option<bool>,
    ordered: bool,
    had_marker: bool,
}

fn parse_list_line(line: &str) -> ParsedLine {
    let mut indent = 0usize;
    for ch in line.chars() {
        match ch {
            ' ' => indent += 1,
            '\t' => indent += 4,
            _ => break,
        }
    }

    let mut rest = line.trim_start();
    let mut ordered = false;
    let mut had_marker = false;

    if let Some(stripped) = rest
        .strip_prefix("- ")
        .or_else(|| rest.strip_prefix("* "))
        .or_else(|| rest.strip_prefix("+ "))
    {
        rest = stripped;
        had_marker = true;
    } else if let Some(stripped) = strip_ordered_marker(rest) {
        rest = stripped;
        ordered = true;
        had_marker = true;
    }

    let (text, checked) = parse_checked_text(rest);
    ParsedLine {
        indent,
        level: indent / 2,
        text,
        checked,
        ordered,
        had_marker,
    }
}

fn strip_ordered_marker(value: &str) -> Option<&str> {
    let mut chars = value.char_indices().peekable();
    let mut saw_digit = false;
    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }
    let (_, marker) = chars.next()?;
    if marker != '.' && marker != ')' {
        return None;
    }
    let (space_idx, space) = chars.next()?;
    if !space.is_whitespace() {
        return None;
    }
    Some(value[space_idx + space.len_utf8()..].trim_start())
}

fn parse_checked_text(value: &str) -> (String, Option<bool>) {
    let trimmed = value.trim();
    for (prefix, checked) in [("[x]", true), ("[X]", true), ("[ ]", false)] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return (rest.trim_start().to_string(), Some(checked));
        }
    }
    (trimmed.to_string(), None)
}

pub fn render_html(doc: &ListDocument, options: &RenderOptions) -> Result<String, String> {
    validate_non_empty(doc)?;
    let mut html = String::new();
    let class_attr = options
        .class_name
        .as_deref()
        .map(|class| format!(r#" class="{}""#, escape_attr(class)))
        .unwrap_or_default();
    let margin = if options.tight { "0" } else { "0 0 0.75em 0" };
    html.push_str(&format!(
        r#"<div{} data-clipli-list="1" style="font-family:{};font-size:{}pt;margin:{};">"#,
        class_attr,
        escape_attr(&options.font),
        escape_attr(&options.font_size),
        margin
    ));
    if let Some(title) = &doc.title {
        html.push_str(&format!(
            r#"<div style="font-weight:700;margin:0 0 0.35em 0;">{}</div>"#,
            escape_html(title)
        ));
    }
    render_html_items(&mut html, &doc.items, doc.kind, options.tight);
    html.push_str("</div>");
    if options.include_metadata {
        let json =
            serde_json::to_string(doc).map_err(|e| format!("could not serialize list: {e}"))?;
        html.push_str(&format!(
            "<!-- clipli-list-json-hex:{} -->",
            hex_encode(&json)
        ));
    }
    Ok(html)
}

fn render_html_items(out: &mut String, items: &[ListItem], kind: ListKind, tight: bool) {
    let tag = match kind {
        ListKind::Unordered => "ul",
        ListKind::Ordered => "ol",
    };
    let margin = if tight {
        "0 0 0 1.2em"
    } else {
        "0.15em 0 0.35em 1.4em"
    };
    out.push_str(&format!(
        r#"<{tag} style="margin:{margin};padding-left:1.2em;">"#
    ));
    for item in items {
        let item_margin = if tight { "0" } else { "0.12em 0" };
        out.push_str(&format!(r#"<li style="margin:{item_margin};">"#));
        if let Some(checked) = item.checked {
            let checked_attr = if checked { " checked" } else { "" };
            out.push_str(&format!(
                r#"<input type="checkbox" disabled{} style="margin-right:0.35em;">"#,
                checked_attr
            ));
        }
        out.push_str(&escape_html(&item.text));
        if !item.items.is_empty() {
            render_html_items(out, &item.items, kind, tight);
        }
        out.push_str("</li>");
    }
    out.push_str(&format!("</{tag}>"));
}

pub fn render_markdown(doc: &ListDocument) -> Result<String, String> {
    validate_non_empty(doc)?;
    let mut out = String::new();
    if let Some(title) = &doc.title {
        out.push_str("# ");
        out.push_str(title.trim());
        out.push_str("\n\n");
    }
    render_markdown_items(&mut out, &doc.items, doc.kind, 0);
    Ok(out)
}

fn render_markdown_items(out: &mut String, items: &[ListItem], kind: ListKind, depth: usize) {
    for (index, item) in items.iter().enumerate() {
        out.push_str(&"  ".repeat(depth));
        match kind {
            ListKind::Unordered => out.push_str("- "),
            ListKind::Ordered => out.push_str(&format!("{}. ", index + 1)),
        }
        if let Some(checked) = item.checked {
            out.push_str(if checked { "[x] " } else { "[ ] " });
        }
        out.push_str(&item.text.replace('\n', " "));
        out.push('\n');
        if !item.items.is_empty() {
            render_markdown_items(out, &item.items, kind, depth + 1);
        }
    }
}

pub fn set_text(doc: &mut ListDocument, spec: &str) -> Result<(), String> {
    let (path, text, checked) = parse_required_path_text(spec, "--set")?;
    let item = item_mut(&mut doc.items, &path)?;
    item.text = text;
    if checked.is_some() {
        item.checked = checked;
    }
    Ok(())
}

pub fn append_item(doc: &mut ListDocument, spec: &str) -> Result<(), String> {
    let (parent_path, text, checked) = parse_optional_path_text(spec)?;
    append_child(
        &mut doc.items,
        &parent_path,
        ListItem {
            text,
            checked,
            items: Vec::new(),
        },
    )?;
    Ok(())
}

pub fn insert_before(doc: &mut ListDocument, spec: &str) -> Result<(), String> {
    let (path, text, checked) = parse_required_path_text(spec, "--insert-before")?;
    insert_relative(&mut doc.items, &path, 0, text, checked)
}

pub fn insert_after(doc: &mut ListDocument, spec: &str) -> Result<(), String> {
    let (path, text, checked) = parse_required_path_text(spec, "--insert-after")?;
    insert_relative(&mut doc.items, &path, 1, text, checked)
}

pub fn remove_item(doc: &mut ListDocument, path: &str) -> Result<(), String> {
    let path = parse_path(path)?;
    let (siblings_path, index) = split_parent_index(&path)?;
    let siblings = children_mut(&mut doc.items, siblings_path)?;
    if index >= siblings.len() {
        return Err(format!("list path '{}' does not exist", format_path(&path)));
    }
    siblings.remove(index);
    Ok(())
}

pub fn set_checked(doc: &mut ListDocument, path: &str, checked: bool) -> Result<(), String> {
    item_mut(&mut doc.items, &parse_path(path)?)?.checked = Some(checked);
    Ok(())
}

pub fn toggle_checked(doc: &mut ListDocument, path: &str) -> Result<(), String> {
    let item = item_mut(&mut doc.items, &parse_path(path)?)?;
    item.checked = Some(!item.checked.unwrap_or(false));
    Ok(())
}

pub fn indent_item(doc: &mut ListDocument, path: &str) -> Result<(), String> {
    let path = parse_path(path)?;
    let (siblings_path, index) = split_parent_index(&path)?;
    if index == 0 {
        return Err(format!(
            "cannot indent '{}': it has no previous sibling",
            format_path(&path)
        ));
    }
    let siblings = children_mut(&mut doc.items, siblings_path)?;
    if index >= siblings.len() {
        return Err(format!("list path '{}' does not exist", format_path(&path)));
    }
    let item = siblings.remove(index);
    siblings[index - 1].items.push(item);
    Ok(())
}

pub fn outdent_item(doc: &mut ListDocument, path: &str) -> Result<(), String> {
    let path = parse_path(path)?;
    if path.len() < 2 {
        return Err(format!("cannot outdent root item '{}'", format_path(&path)));
    }

    let item_index = *path.last().unwrap();
    let parent_path = &path[..path.len() - 1];
    let grand_path = &path[..path.len() - 2];
    let parent_index = *parent_path.last().unwrap();

    let item = {
        let siblings = children_mut(&mut doc.items, parent_path)?;
        if item_index >= siblings.len() {
            return Err(format!("list path '{}' does not exist", format_path(&path)));
        }
        siblings.remove(item_index)
    };

    let grand_siblings = children_mut(&mut doc.items, grand_path)?;
    grand_siblings.insert(parent_index + 1, item);
    Ok(())
}

pub fn sort_at(doc: &mut ListDocument, path: &str) -> Result<(), String> {
    let path = parse_path_or_root(path)?;
    sort_items(children_mut(&mut doc.items, &path)?);
    Ok(())
}

fn parse_required_path_text(
    spec: &str,
    flag_name: &str,
) -> Result<(Vec<usize>, String, Option<bool>), String> {
    let (path, text) = spec
        .split_once(':')
        .ok_or_else(|| format!("{flag_name} expects PATH:TEXT, e.g. 1.2:Updated text"))?;
    let (text, checked) = parse_checked_text(text);
    if text.is_empty() {
        return Err(format!("{flag_name} text cannot be empty"));
    }
    Ok((parse_path(path)?, text, checked))
}

fn parse_optional_path_text(spec: &str) -> Result<(Vec<usize>, String, Option<bool>), String> {
    let (path, text) = match spec.split_once(':') {
        Some((path, text)) if path.trim().is_empty() || parse_path(path).is_ok() => (path, text),
        _ => ("", spec),
    };
    let (text, checked) = parse_checked_text(text);
    if text.is_empty() {
        return Err("--append text cannot be empty".to_string());
    }
    Ok((parse_path_or_root(path)?, text, checked))
}

fn insert_relative(
    items: &mut Vec<ListItem>,
    path: &[usize],
    offset: usize,
    text: String,
    checked: Option<bool>,
) -> Result<(), String> {
    let (siblings_path, index) = split_parent_index(path)?;
    let siblings = children_mut(items, siblings_path)?;
    if index >= siblings.len() {
        return Err(format!("list path '{}' does not exist", format_path(path)));
    }
    siblings.insert(
        index + offset,
        ListItem {
            text,
            checked,
            items: Vec::new(),
        },
    );
    Ok(())
}

fn append_child(
    items: &mut Vec<ListItem>,
    parent_path: &[usize],
    item: ListItem,
) -> Result<usize, String> {
    let siblings = children_mut(items, parent_path)?;
    siblings.push(item);
    Ok(siblings.len() - 1)
}

fn item_mut<'a>(items: &'a mut [ListItem], path: &[usize]) -> Result<&'a mut ListItem, String> {
    let (first, rest) = path
        .split_first()
        .ok_or_else(|| "list path cannot be empty".to_string())?;
    let item = items
        .get_mut(*first)
        .ok_or_else(|| format!("list path '{}' does not exist", format_path(path)))?;
    if rest.is_empty() {
        Ok(item)
    } else {
        item_mut(&mut item.items, rest)
    }
}

fn children_mut<'a>(
    items: &'a mut Vec<ListItem>,
    parent_path: &[usize],
) -> Result<&'a mut Vec<ListItem>, String> {
    if parent_path.is_empty() {
        Ok(items)
    } else {
        Ok(&mut item_mut(items, parent_path)?.items)
    }
}

fn split_parent_index(path: &[usize]) -> Result<(&[usize], usize), String> {
    let (index, parent) = path
        .split_last()
        .ok_or_else(|| "list path cannot be empty".to_string())?;
    Ok((parent, *index))
}

fn parse_path(value: &str) -> Result<Vec<usize>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("list path cannot be empty".to_string());
    }
    trimmed
        .split('.')
        .map(|part| {
            let parsed = part.parse::<usize>().map_err(|_| {
                format!("invalid list path '{trimmed}': use 1-based paths like 1.2")
            })?;
            if parsed == 0 {
                return Err(format!("invalid list path '{trimmed}': indexes start at 1"));
            }
            Ok(parsed - 1)
        })
        .collect()
}

fn parse_path_or_root(value: &str) -> Result<Vec<usize>, String> {
    match value.trim() {
        "" | "." | "root" => Ok(Vec::new()),
        other => parse_path(other),
    }
}

fn format_path(path: &[usize]) -> String {
    path.iter()
        .map(|index| (index + 1).to_string())
        .collect::<Vec<_>>()
        .join(".")
}

fn sort_items(items: &mut Vec<ListItem>) {
    items.sort_by_key(|item| item.text.to_ascii_lowercase());
    for item in items {
        sort_items(&mut item.items);
    }
}

fn dedupe_items(items: &mut Vec<ListItem>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.text.to_ascii_lowercase()));
    for item in items {
        dedupe_items(&mut item.items);
    }
}

fn count_items(items: &[ListItem]) -> usize {
    items.iter().map(|item| 1 + count_items(&item.items)).sum()
}

fn max_depth(items: &[ListItem], depth: usize) -> usize {
    items
        .iter()
        .map(|item| max_depth(&item.items, depth + 1))
        .max()
        .unwrap_or(depth)
}

fn validate_non_empty(doc: &ListDocument) -> Result<(), String> {
    if doc.items.is_empty() {
        Err("list needs at least one item".to_string())
    } else {
        Ok(())
    }
}

fn looks_like_html(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("<ul")
        || lower.contains("<ol")
        || lower.contains("<li")
        || lower.contains("clipli-list-json-hex")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

fn decode_basic_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn hex_encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex_decode(value: &str) -> Result<String, String> {
    if value.len() % 2 != 0 {
        return Err("invalid hex list metadata length".to_string());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let hex = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
        let byte = u8::from_str_radix(hex, 16).map_err(|e| format!("invalid hex metadata: {e}"))?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|e| format!("list metadata is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markdown_nested_tasks() {
        let doc = parse_document(
            "- Launch\n  - [x] QA\n  - [ ] Docs\n- Measure",
            InputFormat::Markdown,
        )
        .unwrap();
        assert_eq!(doc.item_count(), 4);
        assert_eq!(doc.items[0].items[0].checked, Some(true));
    }

    #[test]
    fn renders_html_and_round_trips_metadata() {
        let doc = parse_document(
            r#"{"title":"Plan","items":[{"text":"Launch","items":["QA"]}]}"#,
            InputFormat::Json,
        )
        .unwrap();
        let html = render_html(&doc, &RenderOptions::default()).unwrap();
        let parsed = parse_document(&html, InputFormat::Html).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Plan"));
        assert_eq!(parsed.items[0].items[0].text, "QA");
    }

    #[test]
    fn applies_path_edits() {
        let mut doc = parse_document("- Launch\n  - QA\n- Measure", InputFormat::Markdown).unwrap();
        set_text(&mut doc, "1.1:Regression").unwrap();
        append_item(&mut doc, "1:[ ] Docs").unwrap();
        indent_item(&mut doc, "2").unwrap();
        assert_eq!(doc.items.len(), 1);
        assert_eq!(doc.items[0].items.len(), 3);
        assert_eq!(doc.items[0].items[0].text, "Regression");
        assert_eq!(doc.items[0].items[1].checked, Some(false));
    }
}
