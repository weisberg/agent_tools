use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cli::{PageFetchArgs, PagePatchArgs};
use crate::content::{
    apply_markdown_budget, block_update_payload, blocks_to_markdown, markdown_to_blocks,
    page_update_title_properties, properties_from_sets_with_schema, rich_text_plain,
};
use crate::context::{AuthSource, Context, OauthCredentials};
use crate::error::NotionliError;
use crate::resolve::{resolve_target, ResolvedTarget};
use crate::storage::{cache_object, log_operation, sqlite_query_json};
use crate::util::{api_message, approx_tokens, object_id, operation_id, sql_escape};

const API_BASE: &str = "https://api.notion.com/v1";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub(crate) ok: bool,
    pub(crate) operation_id: String,
    pub(crate) command: String,
    pub(crate) changed: bool,
    pub(crate) dry_run: bool,
    pub(crate) target: Value,
    pub(crate) changes: Vec<Value>,
    pub(crate) undo: Value,
    pub(crate) retried: bool,
    pub(crate) partial: bool,
    #[serde(rename = "_meta")]
    pub(crate) meta: Meta,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Meta {
    pub(crate) approx_tokens: usize,
}
pub(crate) fn update_page(
    ctx: &Context,
    target: &str,
    title: Option<String>,
    sets: Vec<String>,
    if_unmodified_since: Option<String>,
) -> Result<Value, NotionliError> {
    let resolved = resolve_target(ctx, target)?;
    let page = if title.is_some() || !sets.is_empty() {
        page_for_property_schema(ctx, &resolved.id, !ctx.dry_run)?
    } else {
        None
    };
    let schema = page
        .as_ref()
        .and_then(page_data_source_id)
        .map(|data_source_id| data_source_schema(ctx, data_source_id, !ctx.dry_run))
        .transpose()?
        .flatten();
    let mut properties = properties_from_sets_with_schema(sets, schema.as_ref())?;
    if let Some(title) = title {
        properties =
            page_update_title_properties(&title, page.as_ref(), properties, schema.as_ref());
    }
    let mut payload = json!({ "properties": properties });
    if let Some(ts) = if_unmodified_since {
        payload["if_unmodified_since"] = json!(ts);
    }
    write_patch(
        ctx,
        "page.update",
        &format!("/pages/{}", resolved.id),
        payload,
        json!(resolved),
        vec![json!({ "type": "page.update" })],
    )
}

pub(crate) fn data_source_schema_for_parent(
    ctx: &Context,
    parent: &ResolvedTarget,
    allow_live: bool,
) -> Result<Option<Value>, NotionliError> {
    match parent.object_type.as_str() {
        "data_source" => data_source_schema(ctx, &parent.id, allow_live),
        "database" => {
            let Some(database) = cached_object(ctx, &parent.id)?.or_else(|| {
                if allow_live {
                    notion_request(ctx, "GET", &format!("/databases/{}", parent.id), None).ok()
                } else {
                    None
                }
            }) else {
                return Ok(None);
            };
            let Some(source_id) = database
                .get("data_sources")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str)
            else {
                return Ok(None);
            };
            data_source_schema(ctx, source_id, allow_live)
        }
        _ => Ok(None),
    }
}

pub(crate) fn data_source_schema(
    ctx: &Context,
    data_source_id: &str,
    allow_live: bool,
) -> Result<Option<Value>, NotionliError> {
    if let Some(schema) =
        cached_object(ctx, data_source_id)?.and_then(|value| value.get("properties").cloned())
    {
        return Ok(Some(schema));
    }
    if !allow_live {
        return Ok(None);
    }
    let data_source = notion_request(ctx, "GET", &format!("/data_sources/{data_source_id}"), None)?;
    cache_object(ctx, &data_source)?;
    Ok(data_source.get("properties").cloned())
}

fn page_for_property_schema(
    ctx: &Context,
    page_id: &str,
    allow_live: bool,
) -> Result<Option<Value>, NotionliError> {
    if let Some(page) = cached_object(ctx, page_id)? {
        return Ok(Some(page));
    }
    if !allow_live {
        return Ok(None);
    }
    let page = notion_request(ctx, "GET", &format!("/pages/{page_id}"), None)?;
    cache_object(ctx, &page)?;
    Ok(Some(page))
}

fn cached_object(ctx: &Context, object_id: &str) -> Result<Option<Value>, NotionliError> {
    let rows = sqlite_query_json(
        &ctx.db_path,
        &format!(
            "SELECT raw_json FROM objects WHERE object_id = '{}' LIMIT 1",
            sql_escape(object_id)
        ),
    )?;
    let Some(raw) = rows.into_iter().next().and_then(|row| {
        row.get("raw_json")
            .and_then(Value::as_str)
            .map(str::to_string)
    }) else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_str(&raw)?))
}

fn page_data_source_id(page: &Value) -> Option<&str> {
    page.get("parent")
        .and_then(|parent| parent.get("data_source_id"))
        .and_then(Value::as_str)
}

pub(crate) fn patch_page(ctx: &Context, args: PagePatchArgs) -> Result<Value, NotionliError> {
    let resolved = resolve_target(ctx, &args.target)?;
    let markdown = if let Some(path) = args.append_md.as_ref() {
        fs::read_to_string(path)?
    } else if let Some(path) = args.replace_md.as_ref() {
        fs::read_to_string(path)?
    } else if let Some(path) = args.prepend_md.as_ref() {
        fs::read_to_string(path)?
    } else {
        args.append_text
            .clone()
            .or(args.text.clone())
            .unwrap_or_default()
    };
    let mode = if args.append_md.is_some() || args.append_text.is_some() {
        "append"
    } else if args.replace_md.is_some() {
        "replace"
    } else if args.prepend_md.is_some() {
        "prepend"
    } else {
        args.op.as_deref().unwrap_or("patch")
    };
    let changes = vec![json!({
        "type": "page.patch",
        "section": args.section,
        "mode": mode,
        "heading": args.heading,
        "block": args.block,
        "text": markdown,
    })];
    if args.diff || ctx.dry_run {
        return make_receipt(ctx, "page.patch", json!(resolved), changes, false, None);
    }
    ensure_page_unmodified(ctx, &resolved, args.if_unmodified_since.as_deref())?;
    let result = apply_page_patch(ctx, &resolved, &args, mode, &markdown)?;
    make_receipt(ctx, "page.patch", result, changes, true, None)
}

fn ensure_page_unmodified(
    ctx: &Context,
    resolved: &ResolvedTarget,
    expected: Option<&str>,
) -> Result<(), NotionliError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let page = notion_request(ctx, "GET", &format!("/pages/{}", resolved.id), None)?;
    let current = page.get("last_edited_time").and_then(Value::as_str);
    if current == Some(expected) {
        Ok(())
    } else {
        Err(NotionliError::Conflict {
            message: format!(
                "Page changed since {expected}; fetch the current page and retry with the latest last_edited_time."
            ),
            current_last_edited_time: current.map(str::to_string),
        })
    }
}

fn apply_page_patch(
    ctx: &Context,
    resolved: &ResolvedTarget,
    args: &PagePatchArgs,
    mode: &str,
    markdown: &str,
) -> Result<Value, NotionliError> {
    match args.op.as_deref() {
        Some("append_after_heading") => {
            let heading = args
                .heading
                .as_deref()
                .or(args.section.as_deref())
                .ok_or_else(|| NotionliError::Validation {
                    message: "Provide --heading or --section for append_after_heading.".into(),
                })?;
            append_to_heading(ctx, &resolved.id, heading, markdown)
        }
        Some("replace_block") => {
            let block_id = required_patch_block(args)?;
            let result = notion_request(
                ctx,
                "PATCH",
                &format!("/blocks/{block_id}"),
                Some(block_update_payload(markdown)),
            )?;
            Ok(json!({ "mode": "replace_block", "block_id": block_id, "result": result }))
        }
        Some("remove_block") => {
            let block_id = required_patch_block(args)?;
            let result = notion_request(
                ctx,
                "PATCH",
                &format!("/blocks/{block_id}"),
                Some(json!({ "in_trash": true })),
            )?;
            Ok(json!({ "mode": "remove_block", "block_id": block_id, "result": result }))
        }
        Some("insert_at") => {
            let after = required_patch_block(args)?;
            append_blocks(ctx, &resolved.id, Some(after), markdown)
        }
        Some(other) if other != "patch" => Err(NotionliError::Validation {
            message: format!(
                "Unsupported page patch op `{other}`. Use append_after_heading, replace_block, insert_at, or remove_block."
            ),
        }),
        _ if mode == "append" => {
            if let Some(section) = args.section.as_deref() {
                append_to_section(ctx, &resolved.id, section, markdown)
            } else {
                append_blocks(ctx, &resolved.id, None, markdown)
            }
        }
        _ if mode == "replace" => {
            if let Some(section) = args.section.as_deref() {
                replace_section(ctx, &resolved.id, section, markdown)
            } else {
                replace_page_children(ctx, &resolved.id, markdown)
            }
        }
        _ if mode == "prepend" => Err(NotionliError::Validation {
            message: "Notion's block API cannot insert before the first child block; use --op insert_at with a known block or --replace-md.".into(),
        }),
        _ => Err(NotionliError::Validation {
            message: "Provide --append-md, --append-text, --replace-md, or a supported --op.".into(),
        }),
    }
}

fn required_patch_block(args: &PagePatchArgs) -> Result<&str, NotionliError> {
    args.block
        .as_deref()
        .ok_or_else(|| NotionliError::Validation {
            message: "Provide --block for this page patch operation.".into(),
        })
}

fn append_to_heading(
    ctx: &Context,
    page_id: &str,
    heading: &str,
    markdown: &str,
) -> Result<Value, NotionliError> {
    let children = fetch_children_recursive(ctx, page_id, 1)?;
    let heading_id = find_heading_block(&children, heading)?;
    append_blocks(ctx, page_id, Some(&heading_id), markdown)
}

fn append_to_section(
    ctx: &Context,
    page_id: &str,
    section: &str,
    markdown: &str,
) -> Result<Value, NotionliError> {
    let children = fetch_children_recursive(ctx, page_id, 1)?;
    let bounds = section_bounds(&children, section)?;
    append_blocks(ctx, page_id, Some(&bounds.after_block_id), markdown)
}

fn replace_section(
    ctx: &Context,
    page_id: &str,
    section: &str,
    markdown: &str,
) -> Result<Value, NotionliError> {
    let children = fetch_children_recursive(ctx, page_id, 1)?;
    let bounds = section_bounds(&children, section)?;
    let archived = archive_blocks(ctx, &bounds.replace_block_ids)?;
    let appended = append_blocks(ctx, page_id, Some(&bounds.heading_block_id), markdown)?;
    Ok(json!({
        "mode": "replace_section",
        "section": section,
        "archived": archived,
        "append": appended,
    }))
}

fn replace_page_children(
    ctx: &Context,
    page_id: &str,
    markdown: &str,
) -> Result<Value, NotionliError> {
    let children = fetch_children_recursive(ctx, page_id, 1)?;
    let ids = children
        .get("results")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(object_id).collect::<Vec<_>>())
        .unwrap_or_default();
    let archived = archive_blocks(ctx, &ids)?;
    let appended = append_blocks(ctx, page_id, None, markdown)?;
    Ok(json!({ "mode": "replace_page", "archived": archived, "append": appended }))
}

pub(crate) fn replace_page_markdown_blocks(
    ctx: &Context,
    page_id: &str,
    markdown: &str,
) -> Result<Value, NotionliError> {
    replace_page_children(ctx, page_id, markdown)
}

fn archive_blocks(ctx: &Context, block_ids: &[String]) -> Result<Vec<Value>, NotionliError> {
    let mut archived = Vec::new();
    for block_id in block_ids {
        let result = notion_request(
            ctx,
            "PATCH",
            &format!("/blocks/{block_id}"),
            Some(json!({ "in_trash": true })),
        )?;
        archived.push(json!({ "block_id": block_id, "result": result }));
    }
    Ok(archived)
}

fn append_blocks(
    ctx: &Context,
    parent_id: &str,
    after: Option<&str>,
    markdown: &str,
) -> Result<Value, NotionliError> {
    let blocks = markdown_to_blocks(markdown);
    if blocks.is_empty() {
        return Err(NotionliError::Validation {
            message: "Patch markdown/text produced no Notion blocks.".into(),
        });
    }
    let mut payload = json!({ "children": blocks });
    if let Some(after) = after {
        payload["position"] = json!({
            "type": "after_block",
            "after_block": { "id": after },
        });
    }
    let result = notion_request(
        ctx,
        "PATCH",
        &format!("/blocks/{parent_id}/children"),
        Some(payload),
    )?;
    Ok(json!({ "mode": "append_blocks", "parent_id": parent_id, "after": after, "result": result }))
}

#[derive(Debug)]
struct SectionBounds {
    heading_block_id: String,
    after_block_id: String,
    replace_block_ids: Vec<String>,
}

fn section_bounds(children: &Value, heading: &str) -> Result<SectionBounds, NotionliError> {
    let items = children
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| NotionliError::NotFound {
            message: "Page children were not available for section lookup.".into(),
        })?;
    let mut start_index = None;
    let mut start_level = 0u8;
    for (index, item) in items.iter().enumerate() {
        if let Some((level, text)) = heading_block(item) {
            if text.eq_ignore_ascii_case(heading) {
                start_index = Some(index);
                start_level = level;
                break;
            }
        }
    }
    let start = start_index.ok_or_else(|| NotionliError::NotFound {
        message: format!("Heading not found: {heading}"),
    })?;
    let heading_block_id = object_id(&items[start]).ok_or_else(|| NotionliError::Validation {
        message: format!("Heading block for `{heading}` had no block id."),
    })?;
    let mut after_block_id = heading_block_id.clone();
    let mut replace_block_ids = Vec::new();
    for item in items.iter().skip(start + 1) {
        if let Some((level, _)) = heading_block(item) {
            if level <= start_level {
                break;
            }
        }
        if let Some(id) = object_id(item) {
            after_block_id = id.clone();
            replace_block_ids.push(id);
        }
    }
    Ok(SectionBounds {
        heading_block_id,
        after_block_id,
        replace_block_ids,
    })
}

fn find_heading_block(children: &Value, heading: &str) -> Result<String, NotionliError> {
    children
        .get("results")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                heading_block(item).and_then(|(_, text)| {
                    if text.eq_ignore_ascii_case(heading) {
                        object_id(item)
                    } else {
                        None
                    }
                })
            })
        })
        .ok_or_else(|| NotionliError::NotFound {
            message: format!("Heading not found: {heading}"),
        })
}

fn heading_block(block: &Value) -> Option<(u8, String)> {
    let kind = block.get("type").and_then(Value::as_str)?;
    let level = match kind {
        "heading_1" => 1,
        "heading_2" => 2,
        "heading_3" => 3,
        _ => return None,
    };
    let text = block
        .get(kind)
        .and_then(|value| value.get("rich_text"))
        .map(rich_text_plain)
        .unwrap_or_default();
    Some((level, text))
}

pub(crate) fn trash_object(
    ctx: &Context,
    command: &str,
    target: &str,
    confirm_title: Option<String>,
) -> Result<Value, NotionliError> {
    let resolved = resolve_target(ctx, target)?;
    if confirm_title.is_some() && resolved.title.as_deref() != confirm_title.as_deref() {
        return Err(NotionliError::Validation {
            message: "confirm-title does not match the resolved target title.".into(),
        });
    }
    write_patch(
        ctx,
        command,
        &format!("/pages/{}", resolved.id),
        json!({ "in_trash": true }),
        json!(resolved),
        vec![json!({ "type": command })],
    )
}

pub(crate) fn write_patch(
    ctx: &Context,
    command: &str,
    path: &str,
    payload: Value,
    target: Value,
    changes: Vec<Value>,
) -> Result<Value, NotionliError> {
    if ctx.dry_run {
        return make_receipt(ctx, command, target, changes, false, None);
    }
    let result = notion_request(ctx, "PATCH", path, Some(payload))?;
    make_receipt(ctx, command, result, changes, true, None)
}

pub(crate) fn write_post(
    ctx: &Context,
    command: &str,
    path: &str,
    payload: Value,
    target: Value,
    changes: Vec<Value>,
) -> Result<Value, NotionliError> {
    if ctx.dry_run {
        return make_receipt(ctx, command, target, changes, false, None);
    }
    let result = notion_request(ctx, "POST", path, Some(payload))?;
    make_receipt(ctx, command, result, changes, true, None)
}

pub(crate) fn make_receipt(
    ctx: &Context,
    command: &str,
    target: Value,
    changes: Vec<Value>,
    changed: bool,
    inverse: Option<String>,
) -> Result<Value, NotionliError> {
    let operation_id = operation_id();
    let undo = json!({
        "available": inverse.is_some(),
        "command": inverse.clone().unwrap_or_else(|| format!("notionli op undo {operation_id}")),
    });
    let mut receipt = Receipt {
        ok: true,
        operation_id: operation_id.clone(),
        command: command.to_string(),
        changed,
        dry_run: ctx.dry_run,
        target,
        changes,
        undo,
        retried: false,
        partial: false,
        meta: Meta { approx_tokens: 0 },
    };
    let mut value = serde_json::to_value(&receipt)?;
    let tokens = approx_tokens(&value);
    receipt.meta.approx_tokens = tokens;
    value = serde_json::to_value(&receipt)?;
    if changed && !ctx.dry_run {
        log_operation(ctx, &operation_id, command, &value, inverse)?;
    }
    Ok(value)
}

pub(crate) fn notion_request(
    ctx: &Context,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, NotionliError> {
    let auth = ctx.auth_token()?;
    let mut token = auth.token;
    let can_refresh = matches!(auth.source, AuthSource::Oauth);
    let api_base = env::var("NOTIONLI_API_BASE").unwrap_or_else(|_| API_BASE.into());
    let url = if path.starts_with("http") {
        path.to_string()
    } else {
        format!("{}{path}", api_base.trim_end_matches('/'))
    };
    let attempts = ctx.retry.max(1);
    let total_attempts = attempts + u32::from(can_refresh);
    let mut oauth_refreshed = false;
    let mut last_rate_limit = None;
    for attempt in 1..=total_attempts {
        let response = notion_request_once(ctx, method, &url, &token, body.as_ref())?;
        match response.status {
            200..=299 => return Ok(response.value),
            429 if attempt < attempts => {
                let retry_after_ms = retry_after_ms(&response.value).unwrap_or(100);
                last_rate_limit = Some((api_message(&response.value), retry_after_ms));
                thread::sleep(Duration::from_millis(retry_after_ms.min(1_000)));
            }
            429 => {
                let retry_after_ms = retry_after_ms(&response.value).or_else(|| {
                    last_rate_limit
                        .as_ref()
                        .map(|(_, retry_after_ms)| *retry_after_ms)
                });
                return Err(NotionliError::RateLimited {
                    message: api_message(&response.value),
                    retry_after_ms,
                });
            }
            401 if can_refresh && !oauth_refreshed => {
                let refreshed = oauth_refresh(ctx)?;
                token = refreshed.access_token;
                oauth_refreshed = true;
            }
            401 => {
                return Err(NotionliError::Auth {
                    message: api_message(&response.value),
                })
            }
            403 => {
                return Err(NotionliError::Permission {
                    message: api_message(&response.value),
                })
            }
            404 => {
                return Err(NotionliError::NotFound {
                    message: api_message(&response.value),
                })
            }
            409 => {
                return Err(NotionliError::Conflict {
                    message: api_message(&response.value),
                    current_last_edited_time: None,
                })
            }
            status => {
                return Err(NotionliError::Network {
                    message: format!(
                        "Notion API returned HTTP {status}: {}",
                        api_message(&response.value)
                    ),
                })
            }
        }
    }
    Err(NotionliError::Network {
        message: "Notion request retry loop exited unexpectedly.".into(),
    })
}

pub(crate) fn oauth_exchange_code(
    ctx: &Context,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OauthCredentials, NotionliError> {
    let response = oauth_token_request(
        ctx,
        client_id,
        client_secret,
        json!({
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
        }),
    )?;
    oauth_credentials_from_response(response, false)
}

pub(crate) fn oauth_refresh(ctx: &Context) -> Result<OauthCredentials, NotionliError> {
    let mut credentials = ctx
        .oauth_credentials()?
        .ok_or_else(|| NotionliError::Auth {
            message: "No OAuth credentials are stored for this profile.".into(),
        })?;
    let client = oauth_client_config(ctx)?;
    let response = oauth_token_request(
        ctx,
        &client.client_id,
        &client.client_secret,
        json!({
            "grant_type": "refresh_token",
            "refresh_token": credentials.refresh_token,
        }),
    )?;
    let refreshed = oauth_credentials_from_response(response, true)?;
    credentials.access_token = refreshed.access_token;
    credentials.refresh_token = refreshed.refresh_token;
    credentials.refreshed_at = refreshed.refreshed_at;
    if refreshed.bot_id.is_some() {
        credentials.bot_id = refreshed.bot_id;
    }
    if refreshed.workspace_id.is_some() {
        credentials.workspace_id = refreshed.workspace_id;
    }
    if refreshed.workspace_name.is_some() {
        credentials.workspace_name = refreshed.workspace_name;
    }
    if refreshed.workspace_icon.is_some() {
        credentials.workspace_icon = refreshed.workspace_icon;
    }
    if refreshed.owner.is_some() {
        credentials.owner = refreshed.owner;
    }
    ctx.store_oauth_credentials(&credentials)?;
    Ok(credentials)
}

pub(crate) struct OauthClientConfig {
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
}

pub(crate) fn oauth_client_config(ctx: &Context) -> Result<OauthClientConfig, NotionliError> {
    let client_id = env::var("NOTION_OAUTH_CLIENT_ID")
        .or_else(|_| env::var("OAUTH_CLIENT_ID"))
        .ok();
    let client_secret = env::var("NOTION_OAUTH_CLIENT_SECRET")
        .or_else(|_| env::var("OAUTH_CLIENT_SECRET"))
        .ok();
    let file = ctx.config_home.join("oauth-client.json");
    let file_value = if file.exists() {
        Some(serde_json::from_str::<Value>(&fs::read_to_string(&file)?)?)
    } else {
        None
    };
    let client_id = client_id
        .or_else(|| {
            file_value
                .as_ref()
                .and_then(|value| value.get("client_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| NotionliError::Auth {
            message: "OAuth client id missing. Pass --client-id, set NOTION_OAUTH_CLIENT_ID, or create ~/.config/notionli/oauth-client.json.".into(),
        })?;
    let client_secret = client_secret
        .or_else(|| {
            file_value
                .as_ref()
                .and_then(|value| value.get("client_secret"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| NotionliError::Auth {
            message: "OAuth client secret missing. Pass --client-secret, set NOTION_OAUTH_CLIENT_SECRET, or create ~/.config/notionli/oauth-client.json.".into(),
        })?;
    Ok(OauthClientConfig {
        client_id,
        client_secret,
    })
}

fn oauth_token_request(
    _ctx: &Context,
    client_id: &str,
    client_secret: &str,
    body: Value,
) -> Result<Value, NotionliError> {
    let api_base = env::var("NOTIONLI_API_BASE").unwrap_or_else(|_| API_BASE.into());
    let url = format!("{}/oauth/token", api_base.trim_end_matches('/'));
    let curl = env::var_os("NOTIONLI_CURL").unwrap_or_else(|| "curl".into());
    let basic = base64_encode(format!("{client_id}:{client_secret}").as_bytes());
    let output = Command::new(curl)
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Accept: application/json")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("Authorization: Basic {basic}"))
        .arg("-w")
        .arg("\n%{http_code}")
        .arg(url)
        .arg("--data")
        .arg(serde_json::to_string(&body)?)
        .output()?;
    if !output.status.success() {
        return Err(NotionliError::Network {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    notion_response_value(parse_notion_response(&output.stdout)?)
}

fn oauth_credentials_from_response(
    value: Value,
    refreshed: bool,
) -> Result<OauthCredentials, NotionliError> {
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| NotionliError::Auth {
            message: "OAuth token response did not include access_token.".into(),
        })?
        .to_string();
    let refresh_token = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| NotionliError::Auth {
            message: "OAuth token response did not include refresh_token.".into(),
        })?
        .to_string();
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    Ok(OauthCredentials {
        access_token,
        refresh_token,
        bot_id: value
            .get("bot_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        workspace_id: value
            .get("workspace_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        workspace_name: value
            .get("workspace_name")
            .and_then(Value::as_str)
            .map(str::to_string),
        workspace_icon: value
            .get("workspace_icon")
            .and_then(Value::as_str)
            .map(str::to_string),
        duplicated_template_id: value
            .get("duplicated_template_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        owner: value.get("owner").cloned(),
        obtained_at: (!refreshed).then(|| timestamp.clone()),
        refreshed_at: refreshed.then_some(timestamp),
    })
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub(crate) fn notion_send_file_upload(
    ctx: &Context,
    file_upload_id: &str,
    file_path: &Path,
    part_number: Option<u32>,
) -> Result<Value, NotionliError> {
    let auth = ctx.auth_token()?;
    let mut token = auth.token;
    let can_refresh = matches!(auth.source, AuthSource::Oauth);
    let api_base = env::var("NOTIONLI_API_BASE").unwrap_or_else(|_| API_BASE.into());
    let url = format!(
        "{}/file_uploads/{file_upload_id}/send",
        api_base.trim_end_matches('/')
    );
    let mut refreshed = false;
    loop {
        let response = notion_send_file_upload_once(ctx, &url, &token, file_path, part_number)?;
        if response.status == 401 && can_refresh && !refreshed {
            let credentials = oauth_refresh(ctx)?;
            token = credentials.access_token;
            refreshed = true;
            continue;
        }
        return notion_response_value(response);
    }
}

fn notion_send_file_upload_once(
    ctx: &Context,
    url: &str,
    token: &str,
    file_path: &Path,
    part_number: Option<u32>,
) -> Result<NotionResponse, NotionliError> {
    let curl = env::var_os("NOTIONLI_CURL").unwrap_or_else(|| "curl".into());
    let mut cmd = Command::new(curl);
    cmd.arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg(format!("Authorization: Bearer {token}"))
        .arg("-H")
        .arg(format!("Notion-Version: {}", ctx.api_version))
        .arg("-w")
        .arg("\n%{http_code}")
        .arg(url)
        .arg("-F")
        .arg(format!("file=@{}", file_path.display()));
    if let Some(part_number) = part_number {
        cmd.arg("-F").arg(format!("part_number={part_number}"));
    }
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(NotionliError::Network {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    parse_notion_response(&output.stdout)
}

struct NotionResponse {
    status: u16,
    value: Value,
}

fn notion_request_once(
    ctx: &Context,
    method: &str,
    url: &str,
    token: &str,
    body: Option<&Value>,
) -> Result<NotionResponse, NotionliError> {
    let curl = env::var_os("NOTIONLI_CURL").unwrap_or_else(|| "curl".into());
    let mut cmd = Command::new(curl);
    cmd.arg("-sS")
        .arg("-X")
        .arg(method)
        .arg("-H")
        .arg(format!("Authorization: Bearer {token}"))
        .arg("-H")
        .arg(format!("Notion-Version: {}", ctx.api_version))
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-w")
        .arg("\n%{http_code}")
        .arg(url);
    if let Some(body) = body {
        cmd.arg("--data").arg(serde_json::to_string(&body)?);
    }
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(NotionliError::Network {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    parse_notion_response(&output.stdout)
}

fn parse_notion_response(stdout: &[u8]) -> Result<NotionResponse, NotionliError> {
    let stdout = String::from_utf8_lossy(stdout);
    let (body_text, code_text) =
        stdout
            .rsplit_once('\n')
            .ok_or_else(|| NotionliError::Network {
                message: "curl response did not include HTTP status".into(),
            })?;
    let status: u16 = code_text.trim().parse().unwrap_or(0);
    let value: Value = if body_text.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(body_text).unwrap_or_else(|_| json!({ "raw": body_text }))
    };
    Ok(NotionResponse { status, value })
}

fn notion_response_value(response: NotionResponse) -> Result<Value, NotionliError> {
    match response.status {
        200..=299 => Ok(response.value),
        429 => Err(NotionliError::RateLimited {
            message: api_message(&response.value),
            retry_after_ms: retry_after_ms(&response.value),
        }),
        401 => Err(NotionliError::Auth {
            message: api_message(&response.value),
        }),
        403 => Err(NotionliError::Permission {
            message: api_message(&response.value),
        }),
        404 => Err(NotionliError::NotFound {
            message: api_message(&response.value),
        }),
        409 => Err(NotionliError::Conflict {
            message: api_message(&response.value),
            current_last_edited_time: None,
        }),
        status => Err(NotionliError::Network {
            message: format!(
                "Notion API returned HTTP {status}: {}",
                api_message(&response.value)
            ),
        }),
    }
}

fn retry_after_ms(value: &Value) -> Option<u64> {
    value
        .get("retry_after_ms")
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .get("retry_after")
                .and_then(Value::as_u64)
                .map(|s| s * 1000)
        })
}

pub(crate) fn fetch_page_markdown(
    ctx: &Context,
    resolved: &ResolvedTarget,
    args: &PageFetchArgs,
) -> Result<String, NotionliError> {
    let md_path = format!("/pages/{}/markdown", resolved.id);
    if let Ok(value) = notion_request(ctx, "GET", &md_path, None) {
        if let Some(md) = value
            .get("markdown")
            .and_then(Value::as_str)
            .or_else(|| value.get("content").and_then(Value::as_str))
        {
            return Ok(apply_markdown_budget(md, args.budget));
        }
        if let Some(raw) = value.as_str() {
            return Ok(apply_markdown_budget(raw, args.budget));
        }
    }
    let blocks = notion_request(
        ctx,
        "GET",
        &format!("/blocks/{}/children?page_size=100", resolved.id),
        None,
    )?;
    let md = blocks_to_markdown(&blocks);
    Ok(apply_markdown_budget(&md, args.budget))
}

pub(crate) fn run_block_children(
    target: &str,
    depth: u32,
    ctx: &Context,
) -> Result<Value, NotionliError> {
    let resolved = resolve_target(ctx, target)?;
    let children = fetch_children_recursive(ctx, &resolved.id, depth)?;
    Ok(json!({ "target": resolved, "children": children }))
}

pub(crate) fn fetch_children_recursive(
    ctx: &Context,
    id: &str,
    depth: u32,
) -> Result<Value, NotionliError> {
    let mut result = notion_request(
        ctx,
        "GET",
        &format!("/blocks/{id}/children?page_size=100"),
        None,
    )?;
    if depth > 1 {
        if let Some(items) = result.get_mut("results").and_then(Value::as_array_mut) {
            for item in items {
                if item
                    .get("has_children")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    if let Some(child_id) = object_id(item) {
                        item["children"] = fetch_children_recursive(ctx, &child_id, depth - 1)?;
                    }
                }
            }
        }
    }
    Ok(result)
}
