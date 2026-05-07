use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::error::NotionliError;
use crate::storage::{alias_get, object_by_slug_or_title, state_get};
use crate::util::{extract_notion_id, looks_like_uuid, normalize_uuidish};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ResolvedTarget {
    #[serde(rename = "type")]
    pub(crate) object_type: String,
    pub(crate) id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    pub(crate) confidence: f64,
}

pub(crate) fn resolve_target(ctx: &Context, input: &str) -> Result<ResolvedTarget, NotionliError> {
    if input == "." {
        let selected = state_get(ctx, "selected")?.ok_or_else(|| NotionliError::NotFound {
            message: "No selected target is set.".into(),
        })?;
        return Ok(serde_json::from_str(&selected)?);
    }
    if let Some(alias) = alias_get(ctx, input)? {
        return Ok(alias);
    }
    let parsed = parse_reference(input);
    if parsed.id != input || looks_like_uuid(input) || input.starts_with("http") {
        return Ok(ResolvedTarget {
            object_type: parsed.object_type,
            id: parsed.id,
            alias: None,
            slug: None,
            title: None,
            url: parsed.url,
            confidence: 1.0,
        });
    }
    if let Some(row) = object_by_slug_or_title(ctx, input)? {
        return Ok(row);
    }
    Err(NotionliError::NotFound {
        message: format!("Could not resolve target '{input}'."),
    })
}

#[derive(Debug)]
pub(crate) struct ParsedReference {
    pub(crate) object_type: String,
    pub(crate) id: String,
    pub(crate) url: Option<String>,
}

pub(crate) fn parse_reference(input: &str) -> ParsedReference {
    let mut value = input.to_string();
    let mut object_type = "page".to_string();
    if let Some((prefix, rest)) = input.split_once(':') {
        match prefix {
            "page" | "block" | "database" | "data_source" | "ds" | "row" => {
                object_type = if prefix == "ds" {
                    "data_source"
                } else {
                    prefix
                }
                .to_string();
                value = rest.to_string();
            }
            "url" => value = rest.to_string(),
            _ => {}
        }
    }
    if value.starts_with("http") {
        let id = extract_notion_id(&value).unwrap_or(value.clone());
        return ParsedReference {
            object_type,
            id,
            url: Some(value),
        };
    }
    ParsedReference {
        object_type,
        id: normalize_uuidish(&value),
        url: None,
    }
}
