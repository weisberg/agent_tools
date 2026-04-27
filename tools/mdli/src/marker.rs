use std::collections::BTreeMap;

use crate::*;

#[derive(Debug, Clone)]
pub(crate) struct Marker {
    pub(crate) kind: String,
    pub(crate) fields: BTreeMap<String, String>,
}

pub(crate) fn parse_marker(line: &str) -> Option<Marker> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let rest = inner.strip_prefix("mdli:")?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let kind = parts.next()?.to_string();
    let fields_src = parts.next().unwrap_or("").trim();
    let fields = parse_fields(fields_src).ok()?;
    Some(Marker { kind, fields })
}

pub(crate) fn parse_fields(src: &str) -> Result<BTreeMap<String, String>, ()> {
    let mut fields = BTreeMap::new();
    let mut i = 0;
    let chars = src.chars().collect::<Vec<_>>();
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let key_start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '=' {
            return Err(());
        }
        let key = chars[key_start..i].iter().collect::<String>();
        i += 1;
        let value = if i < chars.len() && chars[i] == '"' {
            i += 1;
            let mut value = String::new();
            while i < chars.len() {
                match chars[i] {
                    '\\' if i + 1 < chars.len() => {
                        i += 1;
                        value.push(chars[i]);
                    }
                    '"' => {
                        i += 1;
                        break;
                    }
                    c => value.push(c),
                }
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            chars[value_start..i].iter().collect::<String>()
        };
        fields.insert(key, value);
    }
    Ok(fields)
}

pub(crate) fn render_marker(kind: &str, fields: &BTreeMap<String, String>) -> String {
    let mut parts = Vec::new();
    if let Some(v) = fields.get("v") {
        parts.push(format!("v={}", quote_field(v)));
    } else {
        parts.push(format!("v={MARKER_VERSION}"));
    }
    let primary = match kind {
        "id" | "begin" | "end" => "id",
        "table" => "name",
        _ => "",
    };
    if !primary.is_empty() {
        if let Some(value) = fields.get(primary) {
            parts.push(format!("{primary}={}", quote_field(value)));
        }
    }
    for (key, value) in fields {
        if key == "v" || key == primary {
            continue;
        }
        parts.push(format!("{key}={}", quote_field(value)));
    }
    format!("<!-- mdli:{kind} {} -->", parts.join(" "))
}

pub(crate) fn quote_field(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | ':' | '/' | '+' | '-')
        })
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

pub(crate) fn id_marker(id: &str) -> String {
    let mut fields = BTreeMap::new();
    fields.insert("v".to_string(), MARKER_VERSION.to_string());
    fields.insert("id".to_string(), id.to_string());
    render_marker("id", &fields)
}

pub(crate) fn table_marker(name: &str, key: Option<&str>) -> String {
    let mut fields = BTreeMap::new();
    fields.insert("v".to_string(), MARKER_VERSION.to_string());
    fields.insert("name".to_string(), name.to_string());
    if let Some(key) = key {
        fields.insert("key".to_string(), key.to_string());
    }
    render_marker("table", &fields)
}

pub(crate) fn begin_marker(id: &str, checksum: &str, locked: bool) -> String {
    let mut fields = BTreeMap::new();
    fields.insert("v".to_string(), MARKER_VERSION.to_string());
    fields.insert("id".to_string(), id.to_string());
    fields.insert("checksum".to_string(), checksum.to_string());
    if locked {
        fields.insert("locked".to_string(), "true".to_string());
    }
    render_marker("begin", &fields)
}

pub(crate) fn end_marker(id: &str) -> String {
    let mut fields = BTreeMap::new();
    fields.insert("v".to_string(), MARKER_VERSION.to_string());
    fields.insert("id".to_string(), id.to_string());
    render_marker("end", &fields)
}
