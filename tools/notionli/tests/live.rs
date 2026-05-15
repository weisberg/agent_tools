use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use chrono::Utc;
use serde_json::{json, Value};
use tempfile::TempDir;

struct LiveConfig {
    home: TempDir,
    data_source: String,
}

#[test]
#[ignore = "set NOTIONLI_RUN_LIVE_TESTS=1 and NOTIONLI_LIVE_DATA_SOURCE=<shared-data-source-id>"]
fn live_data_source_row_typed_setters_and_section_patch() {
    let Some(config) = live_data_source_config() else {
        return;
    };
    let home = config.home.path();
    let data_source_ref = format!("data_source:{}", config.data_source);
    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let title = format!("NOTIONLI-LIVE-TEST {stamp}");
    let body =
        format!("## Live Section\nOriginal live test body {stamp}.\n## Tail\nTail body remains.");
    let mut created_page_id = None;

    let result = (|| -> Result<(), String> {
        run_json(home, vec!["auth", "whoami"])?;

        let schema_result = run_json(home, vec!["ds", "schema", &data_source_ref])?;
        schema_result
            .get("schema")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("ds schema did not return an object: {schema_result}"))?;
        let title_property = property_by_type(&schema_result["schema"], "title")
            .map(|(name, _)| name)
            .ok_or_else(|| "live data source has no title property".to_string())?;

        let typed_sets = typed_create_sets(&schema_result["schema"], &stamp);
        let mut create_args = strings([
            "--apply",
            "page",
            "create",
            "--parent",
            &data_source_ref,
            "--title",
            &title,
            "--body",
            &body,
        ]);
        for set in &typed_sets {
            create_args.push("--set".to_string());
            create_args.push(set.clone());
        }
        let created = run_json(home, create_args)?;
        let page_id = receipt_target_id(&created)
            .ok_or_else(|| format!("page create did not return a target id: {created}"))?;
        created_page_id = Some(page_id.clone());

        let filter = json!({
            "property": title_property,
            "title": { "contains": title },
        })
        .to_string();
        let queried = run_json(
            home,
            strings([
                "ds",
                "query",
                &data_source_ref,
                "--filter",
                &filter,
                "--limit",
                "5",
            ]),
        )?;
        assert_query_contains_page(&queried, &page_id)?;

        let patch_text = format!("Inserted by live section patch {stamp}.");
        run_json(
            home,
            strings([
                "--apply",
                "page",
                "patch",
                &format!("page:{page_id}"),
                "--section",
                "Live Section",
                "--append-text",
                &patch_text,
            ]),
        )?;
        let fetched = run_json(
            home,
            strings([
                "page",
                "fetch",
                &format!("page:{page_id}"),
                "--format",
                "md",
            ]),
        )?;
        let markdown = fetched
            .get("markdown")
            .and_then(Value::as_str)
            .or_else(|| fetched.pointer("/content/markdown").and_then(Value::as_str))
            .unwrap_or_default();
        if !markdown.contains(&patch_text) {
            return Err(format!(
                "patched text was not visible in fetched markdown. markdown={markdown:?}"
            ));
        }

        let update_sets = typed_update_sets(&schema_result["schema"], &stamp);
        if !update_sets.is_empty() {
            let mut update_args = strings(["--apply", "row", "update", &format!("page:{page_id}")]);
            for set in update_sets {
                update_args.push("--set".to_string());
                update_args.push(set);
            }
            run_json(home, update_args)?;
        }

        if let Some((name, property)) = first_option_property(&schema_result["schema"]) {
            let invalid = format!("{name}=__notionli_invalid_live_option_{stamp}__");
            let output = run_raw(
                home,
                strings([
                    "--apply",
                    "row",
                    "update",
                    &format!("page:{page_id}"),
                    "--set",
                    &invalid,
                ]),
            );
            if output.status.success() {
                return Err(format!(
                    "invalid option update unexpectedly succeeded for property `{name}`"
                ));
            }
            let combined = output_text(&output);
            if !combined.contains("Available options") {
                return Err(format!(
                    "invalid option error did not include available options for `{name}` ({property}). output={combined}"
                ));
            }
        }

        Ok(())
    })();

    if let Some(page_id) = created_page_id {
        let cleanup = run_raw(
            home,
            strings(["--apply", "page", "trash", &format!("page:{page_id}")]),
        );
        if !cleanup.status.success() {
            eprintln!(
                "live cleanup failed for page {page_id}: {}",
                output_text(&cleanup)
            );
        }
    }

    if let Err(message) = result {
        panic!("{message}");
    }
}

fn live_data_source_config() -> Option<LiveConfig> {
    if env::var("NOTIONLI_RUN_LIVE_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping live Notion tests: set NOTIONLI_RUN_LIVE_TESTS=1");
        return None;
    }
    if !token_available() {
        eprintln!("skipping live Notion tests: set NOTION_API_KEY or ~/.config/NOTION_API_KEY");
        return None;
    }
    let Ok(data_source) = env::var("NOTIONLI_LIVE_DATA_SOURCE") else {
        eprintln!("skipping live Notion tests: set NOTIONLI_LIVE_DATA_SOURCE");
        return None;
    };
    Some(LiveConfig {
        home: tempfile::tempdir().expect("create temp notionli home"),
        data_source,
    })
}

fn token_available() -> bool {
    if env::var("NOTION_API_KEY").is_ok_and(|value| !value.trim().is_empty()) {
        return true;
    }
    let config_home = env::var("XDG_CONFIG_HOME")
        .map(Into::into)
        .unwrap_or_else(|_| {
            env::var("HOME")
                .map(|home| Path::new(&home).join(".config"))
                .unwrap_or_else(|_| Path::new(".").join(".config"))
        });
    Path::new(&config_home).join("NOTION_API_KEY").is_file()
}

fn run_json<I, S>(home: &Path, args: I) -> Result<Value, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_raw(home, args);
    if !output.status.success() {
        return Err(output_text(&output));
    }
    serde_json::from_slice(&output.stdout).map_err(|err| {
        format!(
            "command returned non-json output: {err}\n{}",
            output_text(&output)
        )
    })
}

fn run_raw<I, S>(home: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(env!("CARGO_BIN_EXE_notionli"));
    command.arg("--home").arg(home).arg("--json").args(args);
    command.env_remove("NOTIONLI_CURL");
    command.env_remove("NOTIONLI_API_BASE");
    command
        .output()
        .unwrap_or_else(|err| panic!("failed to run notionli: {err}"))
}

fn strings<const N: usize>(items: [&str; N]) -> Vec<String> {
    items.into_iter().map(str::to_string).collect()
}

fn output_text(output: &Output) -> String {
    format!(
        "status={}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn receipt_target_id(value: &Value) -> Option<String> {
    value
        .pointer("/target/id")
        .and_then(Value::as_str)
        .or_else(|| value.get("id").and_then(Value::as_str))
        .map(str::to_string)
}

fn assert_query_contains_page(value: &Value, page_id: &str) -> Result<(), String> {
    let contains = value
        .pointer("/query/results")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .any(|row| row.get("id").and_then(Value::as_str) == Some(page_id))
        })
        .unwrap_or(false);
    if contains {
        Ok(())
    } else {
        Err(format!(
            "query response did not include created page {page_id}: {value}"
        ))
    }
}

fn typed_create_sets(schema: &Value, stamp: &str) -> Vec<String> {
    let today = Utc::now().date_naive().to_string();
    let mut sets = Vec::new();
    push_property_set(schema, "rich_text", &mut sets, |name, _| {
        format!("{name}=live rich text {stamp}")
    });
    push_property_set(schema, "number", &mut sets, |name, _| format!("{name}=12"));
    push_property_set(schema, "checkbox", &mut sets, |name, _| {
        format!("{name}=true")
    });
    push_property_set(schema, "date", &mut sets, |name, _| {
        format!("{name}={today}")
    });
    push_property_set(schema, "url", &mut sets, |name, _| {
        format!("{name}=https://example.com/notionli-live-test")
    });
    push_property_set(schema, "email", &mut sets, |name, _| {
        format!("{name}=notionli-live@example.com")
    });
    push_property_set(schema, "phone_number", &mut sets, |name, _| {
        format!("{name}=+15555550100")
    });
    push_option_set(schema, "select", &mut sets, 0);
    push_option_set(schema, "status", &mut sets, 0);
    push_multi_select_set(schema, &mut sets);
    sets
}

fn typed_update_sets(schema: &Value, stamp: &str) -> Vec<String> {
    let today = Utc::now().date_naive().to_string();
    let mut sets = Vec::new();
    push_property_set(schema, "rich_text", &mut sets, |name, _| {
        format!("{name}=updated live rich text {stamp}")
    });
    push_property_set(schema, "number", &mut sets, |name, _| format!("{name}=13"));
    push_property_set(schema, "checkbox", &mut sets, |name, _| {
        format!("{name}=false")
    });
    push_property_set(schema, "date", &mut sets, |name, _| {
        format!("{name}={today}")
    });
    push_option_set(schema, "select", &mut sets, 1);
    push_option_set(schema, "status", &mut sets, 1);
    sets
}

fn push_property_set<F>(schema: &Value, property_type: &str, sets: &mut Vec<String>, build: F)
where
    F: FnOnce(&str, &Value) -> String,
{
    if let Some((name, property)) = property_by_type(schema, property_type) {
        sets.push(build(&name, property));
    }
}

fn push_option_set(schema: &Value, property_type: &str, sets: &mut Vec<String>, index: usize) {
    let Some((name, property)) = property_by_type(schema, property_type) else {
        return;
    };
    let options = option_names(property, property_type);
    if options.is_empty() {
        return;
    }
    let selected = options
        .get(index)
        .or_else(|| options.first())
        .expect("checked");
    sets.push(format!("{name}={selected}"));
}

fn push_multi_select_set(schema: &Value, sets: &mut Vec<String>) {
    let Some((name, property)) = property_by_type(schema, "multi_select") else {
        return;
    };
    let options = option_names(property, "multi_select");
    if options.is_empty() {
        return;
    }
    let selected = options.into_iter().take(2).collect::<Vec<_>>().join(",");
    sets.push(format!("{name}={selected}"));
}

fn first_option_property(schema: &Value) -> Option<(String, String)> {
    for property_type in ["status", "select", "multi_select"] {
        let Some((name, property)) = property_by_type(schema, property_type) else {
            continue;
        };
        if !option_names(property, property_type).is_empty() {
            return Some((name, property_type.to_string()));
        }
    }
    None
}

fn property_by_type<'a>(schema: &'a Value, property_type: &str) -> Option<(String, &'a Value)> {
    schema.as_object()?.iter().find_map(|(name, property)| {
        (property.get("type").and_then(Value::as_str) == Some(property_type))
            .then(|| (name.clone(), property))
    })
}

fn option_names(property: &Value, property_type: &str) -> Vec<String> {
    property
        .get(property_type)
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
