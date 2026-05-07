use std::fs;
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::net::{Shutdown, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::Duration;

#[test]
fn schema_errors_returns_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "schema", "errors"])
        .output()
        .expect("run notionli");
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true);
    assert!(value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == "auth_error"));
}

#[test]
fn global_json_outputs_compact_envelope() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "--json", "schema", "errors"])
        .output()
        .expect("schema errors json");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.trim_end().contains('\n'));
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["ok"], true);
}

#[test]
fn global_json_outputs_compact_errors() {
    let home = temp_dir();
    let policy = format!("{home}/policy.json");
    fs::write(
        &policy,
        serde_json::json!({
            "allow": ["alias.list"],
            "deny": ["alias.set"]
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--json",
            "--policy",
            &policy,
            "alias",
            "set",
            "roadmap",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("compact json error");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.trim_end().contains('\n'));
    let value: serde_json::Value = serde_json::from_str(&stderr).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "permission_denied");
}

#[test]
fn global_jsonl_streams_array_payloads() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "--jsonl", "schema", "errors"])
        .output()
        .expect("schema errors jsonl");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();
    assert!(lines.len() >= 3);
    assert!(lines.iter().any(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .map(|value| value["code"] == "auth_error")
            .unwrap_or(false)
    }));
}

#[test]
fn global_jsonl_formats_init_errors_as_single_records() {
    let home = temp_dir();
    let home_file = format!("{home}/not-a-directory");
    fs::write(&home_file, "not a directory").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", &home_file, "--jsonl", "schema", "errors"])
        .output()
        .expect("jsonl init error");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let lines = stderr.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let value: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["command"], "init");
}

#[test]
fn global_quiet_prints_primary_identifier() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            temp_dir(),
            "--quiet",
            "resolve",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("quiet resolve");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "16d8004e-5f6a-42a6-9811-51c22ddada12\n"
    );
}

#[test]
fn global_table_renders_array_payloads() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            temp_dir(),
            "--format",
            "table",
            "schema",
            "errors",
        ])
        .output()
        .expect("schema errors table");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.lines().next().unwrap_or_default().contains("code"));
    assert!(stdout.contains("auth_error"));
}

#[test]
fn auth_login_reports_token_bootstrap_methods() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "auth", "login"])
        .env_remove("NOTION_API_KEY")
        .output()
        .expect("auth login");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(
        value["login"].as_str(),
        Some("ready" | "manual_token_required")
    ));
    assert!(value["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["type"] == "env"));
    assert!(value["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["type"] == "file"));
}

#[cfg(unix)]
#[test]
fn auth_whoami_reads_notion_api_key_config_file() {
    let home = temp_dir();
    let config_home = format!("{home}/config");
    fs::create_dir_all(&config_home).unwrap();
    fs::write(
        format!("{config_home}/NOTION_API_KEY"),
        "secret_from_file\n",
    )
    .unwrap();

    let fake_curl = format!("{home}/fake-auth-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
auth=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -H)
      shift
      case "$1" in
        Authorization:*) auth="$1" ;;
      esac
      ;;
  esac
  shift
done
printf '%s\n' "$auth" > "$(dirname "$0")/auth-header"
printf '{"object":"user","id":"fake-bot-user","type":"bot","bot":{"owner":{"type":"workspace"}}}\n200'
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "auth", "whoami"])
        .env_remove("NOTION_API_KEY")
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", home)
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("auth whoami with config token");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["bot"]["id"], "fake-bot-user");
    let auth_header = fs::read_to_string(format!("{home}/auth-header")).unwrap();
    assert_eq!(auth_header.trim(), "Authorization: Bearer secret_from_file");
}

#[cfg(unix)]
#[test]
fn auth_login_exchanges_oauth_code_and_stores_config_credentials() {
    let home = temp_dir();
    let config_home = format!("{home}/config");
    fs::create_dir_all(&config_home).unwrap();

    let fake_curl = format!("{home}/fake-oauth-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
auth=""
body=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -H)
      shift
      case "$1" in
        Authorization:*) auth="$1" ;;
      esac
      ;;
    --data)
      shift
      body="$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s\n%s\n%s\n' "$auth" "$body" "$url" > "$(dirname "$0")/oauth-request"
printf '{"access_token":"oauth_access","refresh_token":"oauth_refresh","bot_id":"bot_123","workspace_id":"ws_123","workspace_name":"Test Workspace","owner":{"type":"user"}}\n200'
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "auth",
            "login",
            "--client-id",
            "client",
            "--client-secret",
            "secret",
            "--redirect-uri",
            "http://127.0.0.1:53682/oauth/callback",
            "--code",
            "auth_code",
        ])
        .env_remove("NOTION_API_KEY")
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", home)
        .env("NOTIONLI_CURL", &fake_curl)
        .env("NOTIONLI_API_BASE", "https://api.notion.test/v1")
        .output()
        .expect("auth login oauth code");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["login"], "ready");
    assert_eq!(value["storage"], "oauth");
    assert_eq!(value["workspace_id"], "ws_123");

    let stored = fs::read_to_string(format!(
        "{config_home}/notionli/profiles/default/oauth.json"
    ))
    .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored["access_token"], "oauth_access");
    assert_eq!(stored["refresh_token"], "oauth_refresh");

    let client_config =
        fs::read_to_string(format!("{config_home}/notionli/oauth-client.json")).unwrap();
    let client_config: serde_json::Value = serde_json::from_str(&client_config).unwrap();
    assert_eq!(client_config["client_id"], "client");
    assert_eq!(client_config["client_secret"], "secret");
    assert_eq!(
        client_config["redirect_uri"],
        "http://127.0.0.1:53682/oauth/callback"
    );

    let request = fs::read_to_string(format!("{home}/oauth-request")).unwrap();
    assert!(request.contains("Authorization: Basic Y2xpZW50OnNlY3JldA=="));
    assert!(request.contains("\"grant_type\":\"authorization_code\""));
    assert!(request.contains("\"code\":\"auth_code\""));
}

#[cfg(unix)]
#[test]
fn auth_whoami_uses_stored_oauth_credentials_without_api_key() {
    let home = temp_dir();
    let config_home = format!("{home}/config");
    let profile_config = format!("{config_home}/notionli/profiles/default");
    fs::create_dir_all(&profile_config).unwrap();
    fs::write(
        format!("{profile_config}/oauth.json"),
        r#"{"access_token":"oauth_access","refresh_token":"oauth_refresh","workspace_id":"ws_123"}"#,
    )
    .unwrap();

    let fake_curl = format!("{home}/fake-oauth-whoami-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
auth=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -H)
      shift
      case "$1" in
        Authorization:*) auth="$1" ;;
      esac
      ;;
  esac
  shift
done
printf '%s\n' "$auth" > "$(dirname "$0")/auth-header"
printf '{"object":"user","id":"oauth-bot-user","type":"bot","bot":{"owner":{"type":"workspace"}}}\n200'
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "auth", "whoami"])
        .env_remove("NOTION_API_KEY")
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", home)
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("auth whoami with oauth token");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["bot"]["id"], "oauth-bot-user");
    let auth_header = fs::read_to_string(format!("{home}/auth-header")).unwrap();
    assert_eq!(auth_header.trim(), "Authorization: Bearer oauth_access");
}

#[cfg(unix)]
#[test]
fn oauth_credentials_refresh_after_unauthorized_response() {
    let home = temp_dir();
    let config_home = format!("{home}/config");
    let notionli_config = format!("{config_home}/notionli");
    let profile_config = format!("{notionli_config}/profiles/default");
    fs::create_dir_all(&profile_config).unwrap();
    fs::write(
        format!("{profile_config}/oauth.json"),
        r#"{"access_token":"oauth_old","refresh_token":"oauth_refresh","workspace_id":"ws_123"}"#,
    )
    .unwrap();
    fs::write(
        format!("{notionli_config}/oauth-client.json"),
        r#"{"client_id":"client","client_secret":"secret","redirect_uri":"http://127.0.0.1:53682/oauth/callback"}"#,
    )
    .unwrap();

    let fake_curl = format!("{home}/fake-oauth-refresh-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
auth=""
body=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -H)
      shift
      case "$1" in
        Authorization:*) auth="$1" ;;
      esac
      ;;
    --data)
      shift
      body="$1"
      ;;
  esac
  shift
done
printf '%s\n' "$auth" >> "$(dirname "$0")/auth-headers"
case "$body" in
  *refresh_token*)
    printf '{"access_token":"oauth_new","refresh_token":"oauth_refresh_new","workspace_id":"ws_123"}\n200'
    ;;
  *)
    case "$auth" in
      *oauth_old*)
        printf '{"object":"error","code":"unauthorized","message":"expired"}\n401'
        ;;
      *oauth_new*)
        printf '{"object":"user","id":"refreshed-bot-user","type":"bot","bot":{"owner":{"type":"workspace"}}}\n200'
        ;;
      *)
        printf '{"object":"error","code":"unauthorized","message":"wrong token"}\n401'
        ;;
    esac
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--retry", "1", "auth", "whoami"])
        .env_remove("NOTION_API_KEY")
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", home)
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("auth whoami refreshes oauth token");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["bot"]["id"], "refreshed-bot-user");
    let stored = fs::read_to_string(format!("{profile_config}/oauth.json")).unwrap();
    assert!(stored.contains("oauth_new"));
    assert!(stored.contains("oauth_refresh_new"));
}

#[test]
fn schema_commands_reflects_real_clap_tree() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "schema", "commands"])
        .output()
        .expect("schema commands");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["commands"]["name"], "notionli");
    assert!(has_command_path(&value["commands"], &["page", "fetch"]));
    assert!(has_command_path(
        &value["commands"],
        &["page", "worktree", "checkout"]
    ));
    assert!(has_command_path(
        &value["commands"],
        &["page", "worktree", "push"]
    ));
    assert!(has_command_path(&value["commands"], &["tools", "schema"]));
    assert!(has_command_path(&value["commands"], &["ds", "deduplicate"]));
    assert!(has_command_path(&value["commands"], &["bulk", "rename"]));
    assert!(has_command_path(&value["commands"], &["webhook", "create"]));
    assert!(has_command_path(&value["commands"], &["webhook", "serve"]));
    assert!(has_command_path(&value["commands"], &["watch"]));
    assert!(has_command_path(&value["commands"], &["mock", "serve"]));
    assert!(has_command_path(&value["commands"], &["fixture", "record"]));
}

#[test]
fn doctor_round_trip_returns_applyable_plan_by_default() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            temp_dir(),
            "doctor",
            "round-trip",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("doctor round-trip");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["round_trip"], "planned");
    assert_eq!(value["dry_run"], true);
    assert!(value["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "trash_created_page"));
}

#[test]
fn tui_returns_terminal_dashboard_summary() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "tui"])
        .output()
        .expect("tui summary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["tui"], "summary");
    assert_eq!(value["cache"]["objects"], 1);
    assert!(value["next_actions"].as_array().unwrap().len() >= 2);
}

#[test]
fn webhook_create_list_delete_and_watch_are_local_direct_mode() {
    let home = temp_dir();
    let create = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "webhook",
            "create",
            "--events",
            "page.content_updated,data_source.content_updated",
            "--url",
            "https://example.com/notionli-hook",
        ])
        .output()
        .expect("webhook create");
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    let created: serde_json::Value = serde_json::from_slice(&create.stdout).unwrap();
    let webhook_id = created["webhook"]["webhook_id"].as_str().unwrap();
    assert_eq!(created["stored"], true);

    let list = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "webhook", "list"])
        .output()
        .expect("webhook list");
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let listed: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(listed["webhooks"].as_array().unwrap().len(), 1);

    let watch = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "watch",
            "--events",
            "page.content_updated",
            "--all-shared",
        ])
        .output()
        .expect("watch");
    assert!(
        watch.status.success(),
        "{}",
        String::from_utf8_lossy(&watch.stderr)
    );
    let watched: serde_json::Value = serde_json::from_slice(&watch.stdout).unwrap();
    assert_eq!(watched["watch"], "direct-poll");
    assert_eq!(watched["webhooks"].as_array().unwrap().len(), 1);

    let serve_plan = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "webhook", "serve", "--port", "0", "--once"])
        .output()
        .expect("webhook serve plan");
    assert!(
        serve_plan.status.success(),
        "{}",
        String::from_utf8_lossy(&serve_plan.stderr)
    );
    let served: serde_json::Value = serde_json::from_slice(&serve_plan.stdout).unwrap();
    assert_eq!(served["webhook"], "serve-plan");
    assert_eq!(served["transport"], "http");
    assert_eq!(served["dry_run"], serde_json::Value::Null);

    let delete = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--apply", "webhook", "delete", webhook_id])
        .output()
        .expect("webhook delete");
    assert!(
        delete.status.success(),
        "{}",
        String::from_utf8_lossy(&delete.stderr)
    );
    let deleted: serde_json::Value = serde_json::from_slice(&delete.stdout).unwrap();
    assert_eq!(deleted["deleted"], true);
}

#[cfg(unix)]
#[test]
#[ignore = "binds a localhost TCP port; run explicitly where socket binds are allowed"]
fn webhook_serve_captures_http_events() {
    let home = temp_dir();
    let port_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_listener.local_addr().unwrap().port();
    drop(port_listener);
    let out = format!("{home}/captured-webhooks.jsonl");
    let hook = format!("{home}/webhook-hook.sh");
    fs::write(
        &hook,
        r#"#!/bin/sh
printf '%s\n' "$NOTIONLI_WEBHOOK_EVENT" > "$(dirname "$0")/webhook-env.json"
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).unwrap();

    let child = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "webhook",
            "serve",
            "--port",
            &port.to_string(),
            "--once",
            "--out",
            &out,
            "--secret",
            "test-secret",
            "--on-event",
            &hook,
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn webhook server");

    let mut stream = connect_with_retry(port);
    let body = r#"{"type":"page.content_updated","page_id":"page_123"}"#;
    write!(
        stream,
        "POST /webhook HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nX-Notionli-Secret: test-secret\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let response = read_http_response(stream);
    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
    let response_body = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    let accepted: serde_json::Value = serde_json::from_str(response_body).unwrap();
    assert_eq!(accepted["accepted"], true);
    assert_eq!(accepted["event"]["event"]["type"], "page.content_updated");

    let output = child.wait_with_output().expect("webhook server output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stopped: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stopped["webhook"], "stopped");
    assert_eq!(stopped["handled"], 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("webhook"));

    let captured = fs::read_to_string(&out).unwrap();
    let captured: serde_json::Value =
        serde_json::from_str(captured.lines().next().unwrap()).unwrap();
    assert_eq!(captured["event"]["page_id"], "page_123");
    let env_event: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(format!("{home}/webhook-env.json")).unwrap())
            .unwrap();
    assert_eq!(env_event["event"]["type"], "page.content_updated");
}

#[cfg(unix)]
#[test]
fn watch_checkpoints_cache_changes_and_runs_on_change() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );
    let hook = format!("{home}/watch-hook.sh");
    fs::write(
        &hook,
        r#"#!/bin/sh
printf '%s\n' "$NOTIONLI_WATCH_CHANGES" >> "$(dirname "$0")/watch-events.jsonl"
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).unwrap();

    let first = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "watch",
            "--events",
            "page.content_updated",
            "--all-shared",
            "--on-change",
            &hook,
        ])
        .output()
        .expect("first watch");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(value["watch"], "direct-poll");
    assert_eq!(value["change_count"], 1);
    assert_eq!(value["changes"][0]["event"], "added");
    assert_eq!(value["checkpointed"], true);
    assert_eq!(value["on_change_result"]["success"], true);

    let unchanged = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "watch",
            "--events",
            "page.content_updated",
            "--all-shared",
        ])
        .output()
        .expect("unchanged watch");
    assert!(unchanged.status.success());
    let value: serde_json::Value = serde_json::from_slice(&unchanged.stdout).unwrap();
    assert_eq!(value["change_count"], 0);

    update_cache_object_title(
        home,
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "Roadmap Updated",
    );
    let updated = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "watch",
            "--events",
            "page.content_updated",
            "--all-shared",
            "--on-change",
            &hook,
        ])
        .output()
        .expect("updated watch");
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&updated.stdout).unwrap();
    assert_eq!(value["change_count"], 1);
    assert_eq!(value["changes"][0]["event"], "updated");
    assert_eq!(
        value["changes"][0]["previous_updated_at"],
        "2026-05-04T12:00:00Z"
    );

    let events = fs::read_to_string(format!("{home}/watch-events.jsonl")).unwrap();
    assert!(events.contains("\"event\":\"added\""));
    assert!(events.contains("\"event\":\"updated\""));
}

#[test]
fn mock_manifest_and_fixture_record_replay_support_offline_flows() {
    let home = temp_dir();
    let mock = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "mock", "serve"])
        .output()
        .expect("mock serve");
    assert!(
        mock.status.success(),
        "{}",
        String::from_utf8_lossy(&mock.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&mock.stdout).unwrap();
    assert_eq!(manifest["mock"], "manifest");
    assert_eq!(manifest["transport"], "http");
    assert_eq!(manifest["api_base_env"]["name"], "NOTIONLI_API_BASE");
    assert_eq!(manifest["curl_env"]["name"], "NOTIONLI_CURL");

    let fixture_path = format!("{home}/schema-errors-fixture.json");
    let record = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "fixture",
            "record",
            "--command",
            "schema errors",
            "--out",
            &fixture_path,
        ])
        .output()
        .expect("fixture record");
    assert!(
        record.status.success(),
        "{}",
        String::from_utf8_lossy(&record.stderr)
    );
    let recorded: serde_json::Value = serde_json::from_slice(&record.stdout).unwrap();
    assert_eq!(recorded["record"]["result"]["command"], "schema");

    let replay = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "fixture", "replay", &fixture_path])
        .output()
        .expect("fixture replay");
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let replayed: serde_json::Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replayed["command"], "schema errors");
    assert!(replayed["result"]["errors"].as_array().unwrap().len() >= 10);
}

#[cfg(unix)]
#[test]
#[ignore = "binds a localhost TCP port; run explicitly where socket binds are allowed"]
fn mock_serve_apply_starts_http_notion_fixture() {
    let server_home = temp_dir();
    let client_home = temp_dir();
    let port_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_listener.local_addr().unwrap().port();
    drop(port_listener);

    let mut server = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            server_home,
            "--apply",
            "mock",
            "serve",
            "--port",
            &port.to_string(),
            "--once",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start mock server");

    thread::sleep(Duration::from_millis(150));
    let whoami = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", client_home, "auth", "whoami"])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_API_BASE", format!("http://127.0.0.1:{port}/v1"))
        .output()
        .expect("auth whoami through mock server");
    if !whoami.status.success() {
        let _ = server.kill();
        let server_output = server.wait_with_output().ok();
        panic!(
            "whoami failed\nstdout:\n{}\nstderr:\n{}\nserver:\n{:?}",
            String::from_utf8_lossy(&whoami.stdout),
            String::from_utf8_lossy(&whoami.stderr),
            server_output
        );
    }
    let value: serde_json::Value = serde_json::from_slice(&whoami.stdout).unwrap();
    assert_eq!(value["bot"]["id"], "fake-bot-user");

    let server_output = server.wait_with_output().expect("mock server output");
    assert!(
        server_output.status.success(),
        "{}",
        String::from_utf8_lossy(&server_output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&server_output.stdout).unwrap();
    assert_eq!(value["mock"], "stopped");
    assert_eq!(value["handled"], 1);
    assert!(String::from_utf8_lossy(&server_output.stderr).contains("mock"));
}

#[test]
fn completion_generates_shell_script_from_command_catalog() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", temp_dir(), "completion", "bash"])
        .output()
        .expect("completion bash");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["shell"], "bash");
    let script = value["script"].as_str().unwrap();
    assert!(script.contains("complete -W"));
    assert!(script.contains("page fetch"));
    assert!(script.contains("tools schema"));
}

#[test]
fn tools_schema_can_emit_profile_filtered_openai_tools() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            temp_dir(),
            "tools",
            "schema",
            "page.fetch",
            "--format",
            "openai",
            "--profile",
            "readonly",
        ])
        .output()
        .expect("tools schema");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["format"], "openai");
    assert_eq!(value["profile"], "readonly");
    assert_eq!(value["count"], 1);
    assert_eq!(value["tools"][0]["function"]["name"], "page_fetch");
    assert_eq!(
        value["tools"][0]["function"]["parameters"]["properties"]["recursive"]["type"],
        "boolean"
    );
    assert!(value["tools"][0]["function"]["parameters"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "target"));
}

#[test]
fn mcp_serve_exposes_manifest_and_stdio_jsonrpc() {
    let home = temp_dir();
    let manifest = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "mcp", "serve"])
        .output()
        .expect("mcp manifest");
    assert!(
        manifest.status.success(),
        "{}",
        String::from_utf8_lossy(&manifest.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&manifest.stdout).unwrap();
    assert_eq!(value["mcp"], "ready");
    assert!(value["tools"].as_array().unwrap().len() > 10);

    let mut child = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "mcp", "serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp stdio");
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"alias.list","arguments":{{}}}}}}"#
        )
        .unwrap();
    }
    let output = child.wait_with_output().expect("mcp stdio output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "notionli");
    assert!(lines[1]["result"]["tools"].as_array().unwrap().len() > 10);
    assert_eq!(lines[2]["result"]["isError"], false);
}

#[cfg(unix)]
#[test]
#[ignore = "binds a localhost TCP port; run explicitly where socket binds are allowed"]
fn mcp_serve_http_accepts_jsonrpc_requests() {
    let home = temp_dir();
    let port_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_listener.local_addr().unwrap().port();
    drop(port_listener);

    let child = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "mcp",
            "serve",
            "--http",
            "--port",
            &port.to_string(),
            "--once",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp http");

    let mut stream = connect_with_retry(port);
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"alias.list","arguments":{}}}"#;
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let response = read_http_response(stream);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(value["id"], 1);
    assert_eq!(value["result"]["isError"], false);

    let output = child.wait_with_output().expect("mcp http output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stopped: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stopped["mcp"], "stopped");
    assert_eq!(stopped["handled"], 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("http-jsonrpc"));
}

#[cfg(unix)]
#[test]
fn notion_requests_retry_once_after_rate_limit() {
    let home = temp_dir();
    let fake_curl = format!("{home}/fake-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
count_file="$(dirname "$0")/curl-attempts"
if [ -f "$count_file" ]; then
  count=$(cat "$count_file")
else
  count=0
fi
count=$((count + 1))
printf "%s" "$count" > "$count_file"
if [ "$count" -eq 1 ]; then
  printf '{"message":"slow down","retry_after_ms":1}\n429'
else
  printf '{"object":"user","id":"bot-user","attempt":%s}\n200' "$count"
fi
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--retry", "2", "auth", "whoami"])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("auth whoami through fake curl");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["bot"]["attempt"], 2);
    assert_eq!(
        fs::read_to_string(format!("{home}/curl-attempts"))
            .unwrap()
            .trim(),
        "2"
    );
}

#[cfg(unix)]
#[test]
fn apply_paths_use_notion_http_layer_with_fake_api() {
    let home = temp_dir();
    let fake_curl = format!("{home}/fake-notion-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
method="GET"
url=""
data=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="$1"
      ;;
    --data)
      shift
      data="$1"
      ;;
    -F)
      shift
      data="$data FORM:$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s %s %s\n' "$method" "$url" "$data" >> "$(dirname "$0")/curl-log"
id="${url##*/}"
case "$method $url" in
  "POST https://api.notion.com/v1/pages")
    printf '{"object":"page","id":"cccccccc-cccc-cccc-cccc-cccccccccccc","url":"https://notion.so/created","properties":{"Name":{"type":"title","title":[{"plain_text":"Smoke"}]}}}\n200'
    ;;
  GET*\ */pages/*)
    printf '{"object":"page","id":"%s","url":"https://notion.so/fetched","properties":{"Name":{"type":"title","title":[{"plain_text":"Fetched"}]}}}\n200' "$id"
    ;;
  PATCH*\ */pages/*)
    printf '{"object":"page","id":"%s","in_trash":true,"url":"https://notion.so/trashed","properties":{"Name":{"type":"title","title":[{"plain_text":"Trashed"}]}}}\n200' "$id"
    ;;
  *)
    printf '{"message":"unexpected request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let create = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "page",
            "create",
            "--parent",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--title",
            "Smoke",
            "--body",
            "hello",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("page create through fake Notion");
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&create.stdout).unwrap();
    assert_eq!(value["command"], "page.create");
    assert_eq!(value["changed"], true);
    assert_eq!(
        value["target"]["id"],
        "cccccccc-cccc-cccc-cccc-cccccccccccc"
    );

    let doctor = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "doctor",
            "round-trip",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("doctor round-trip through fake Notion");
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert_eq!(value["round_trip"], "ok");
    assert_eq!(value["fetched"], true);
    assert_eq!(value["trashed"], true);

    let log = fs::read_to_string(format!("{home}/curl-log")).unwrap();
    assert!(log.contains("POST https://api.notion.com/v1/pages"));
    assert!(log.contains("\"properties\":{\"title\""));
    assert!(!log.contains("\"properties\":{\"Name\""));
    assert!(
        log.contains("GET https://api.notion.com/v1/pages/cccccccc-cccc-cccc-cccc-cccccccccccc")
    );
    assert!(
        log.contains("PATCH https://api.notion.com/v1/pages/cccccccc-cccc-cccc-cccc-cccccccccccc")
    );
}

#[cfg(unix)]
#[test]
fn data_source_and_attachment_apply_paths_use_fake_api() {
    let home = temp_dir();
    let fake_curl = format!("{home}/fake-notion-data-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
method="GET"
url=""
data=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="$1"
      ;;
    --data)
      shift
      data="$1"
      ;;
    -F)
      shift
      data="$data FORM:$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s %s %s\n' "$method" "$url" "$data" >> "$(dirname "$0")/curl-log"
id="${url##*/}"
case "$method $url" in
  POST*\ */data_sources/*/query)
    printf '{"object":"list","results":[{"object":"page","id":"dddddddd-dddd-dddd-dddd-dddddddddddd","url":"https://notion.so/row","properties":{"Name":{"type":"title","title":[{"plain_text":"Existing row"}]}}}]}\n200'
    ;;
  "POST https://api.notion.com/v1/file_uploads")
    printf '{"object":"file_upload","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","status":"pending","upload_url":"https://api.notion.com/v1/file_uploads/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/send","filename":"note.txt","content_type":"text/plain","content_length":null}\n200'
    ;;
  POST*\ */file_uploads/*/send)
    printf '{"object":"file_upload","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","status":"uploaded","filename":"note.txt","content_type":"text/plain","content_length":14}\n200'
    ;;
  POST*\ */file_uploads/*/complete)
    printf '{"object":"file_upload","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee","status":"uploaded","filename":"note.txt","content_type":"text/plain","content_length":14,"number_of_parts":{"total":1,"sent":1}}\n200'
    ;;
  "POST https://api.notion.com/v1/pages")
    printf '{"object":"page","id":"eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee","url":"https://notion.so/new-row","properties":{"Name":{"type":"title","title":[{"plain_text":"Created row"}]}}}\n200'
    ;;
  PATCH*\ */pages/*)
    printf '{"object":"page","id":"%s","url":"https://notion.so/updated","properties":{"Name":{"type":"title","title":[{"plain_text":"Updated row"}]}}}\n200' "$id"
    ;;
  PATCH*\ */blocks/*/children)
    printf '{"object":"list","results":[{"object":"block","id":"ffffffff-ffff-ffff-ffff-ffffffffffff","type":"file"}]}\n200'
    ;;
  *)
    printf '{"message":"unexpected request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    let existing_id = "dddddddd-dddd-dddd-dddd-dddddddddddd";
    let related_id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

    let upsert = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "row",
            "upsert",
            &format!("data_source:{ds_id}"),
            "--key",
            "ExternalID=gh:123",
            "--set",
            "Status=Done",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("row upsert through fake Notion");
    assert!(
        upsert.status.success(),
        "{}",
        String::from_utf8_lossy(&upsert.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&upsert.stdout).unwrap();
    assert_eq!(value["command"], "page.update");
    assert_eq!(value["changed"], true);

    let bulk_update = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "ds",
            "bulk-update",
            &format!("data_source:{ds_id}"),
            "--where",
            "Status=Todo",
            "--set",
            "Status=Done",
            "--max-write",
            "1",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("bulk update through fake Notion");
    assert!(
        bulk_update.status.success(),
        "{}",
        String::from_utf8_lossy(&bulk_update.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&bulk_update.stdout).unwrap();
    assert_eq!(value["command"], "ds.bulk-update");
    assert_eq!(value["changed"], true);
    assert_eq!(value["target"]["row_count"], 1);

    let relate = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "row",
            "relate",
            &format!("page:{existing_id}"),
            "Depends On",
            &format!("page:{related_id}"),
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("row relate through fake Notion");
    assert!(
        relate.status.success(),
        "{}",
        String::from_utf8_lossy(&relate.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&relate.stdout).unwrap();
    assert_eq!(value["command"], "row.relate");
    assert_eq!(value["changed"], true);

    let attach = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "file",
            "attach",
            "https://example.com/brief.pdf",
            "--page",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("file attach through fake Notion");
    assert!(
        attach.status.success(),
        "{}",
        String::from_utf8_lossy(&attach.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&attach.stdout).unwrap();
    assert_eq!(value["command"], "file.attach");
    assert_eq!(value["changed"], true);

    let source = format!("{home}/note.txt");
    fs::write(&source, "hello notionli").unwrap();
    let upload = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--apply", "file", "upload", &source])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("native file upload through fake Notion");
    assert!(
        upload.status.success(),
        "{}",
        String::from_utf8_lossy(&upload.stderr)
    );
    let uploaded: serde_json::Value = serde_json::from_slice(&upload.stdout).unwrap();
    assert_eq!(
        uploaded["file_upload_id"],
        "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
    );
    assert_eq!(uploaded["status"], "uploaded");

    let multipart_upload = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "file",
            "upload",
            &source,
            "--multipart",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("multipart file upload through fake Notion");
    assert!(
        multipart_upload.status.success(),
        "{}",
        String::from_utf8_lossy(&multipart_upload.stderr)
    );
    let multipart: serde_json::Value = serde_json::from_slice(&multipart_upload.stdout).unwrap();
    assert_eq!(multipart["multipart"], true);
    assert_eq!(multipart["part_count"], 1);
    assert_eq!(multipart["status"], "uploaded");

    let attach_upload = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "file",
            "attach",
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "--page",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("attach native file upload through fake Notion");
    assert!(
        attach_upload.status.success(),
        "{}",
        String::from_utf8_lossy(&attach_upload.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&attach_upload.stdout).unwrap();
    assert_eq!(value["command"], "file.attach");
    assert_eq!(value["changed"], true);

    let image_source = format!("{home}/screenshot.png");
    fs::write(&image_source, "fake png bytes").unwrap();
    let attach_image = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "file",
            "attach",
            &image_source,
            "--page",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("attach image through fake Notion");
    assert!(
        attach_image.status.success(),
        "{}",
        String::from_utf8_lossy(&attach_image.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&attach_image.stdout).unwrap();
    assert_eq!(value["command"], "file.attach");
    assert_eq!(value["changed"], true);

    let log = fs::read_to_string(format!("{home}/curl-log")).unwrap();
    assert!(log.contains(&format!(
        "POST https://api.notion.com/v1/data_sources/{ds_id}/query"
    )));
    assert!(log.contains(&format!(
        "PATCH https://api.notion.com/v1/pages/{existing_id}"
    )));
    assert!(log.contains(
        "PATCH https://api.notion.com/v1/blocks/16d8004e-5f6a-42a6-9811-51c22ddada12/children"
    ));
    assert!(log.contains("POST https://api.notion.com/v1/file_uploads"));
    assert!(log.contains(
        "POST https://api.notion.com/v1/file_uploads/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/send"
    ));
    assert!(log.contains(
        "POST https://api.notion.com/v1/file_uploads/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/complete"
    ));
    assert!(log.contains("\"mode\":\"multi_part\""));
    assert!(log.contains("\"number_of_parts\":1"));
    assert!(log.contains("FORM:part_number=1"));
    assert!(log.contains("\"type\":\"image\""));
    assert!(log.contains("\"image\":{\"caption\":[],\"type\":\"file_upload\""));
    assert!(log.contains("\"type\":\"file_upload\""));
    assert!(log.contains("\"id\":\"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee\""));
}

#[test]
fn cache_search_supports_recent_stale_and_duplicates() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );
    seed_cache_object(
        home,
        "page",
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "roadmap-copy",
        "Roadmap",
        "2000-01-01T00:00:00Z",
    );
    seed_cache_object(
        home,
        "data_source",
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
        "tasks",
        "Tasks",
        "2026-05-04T12:00:00Z",
    );

    let recent = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "search", "roadmap", "--recent"])
        .output()
        .expect("recent search");
    assert!(
        recent.status.success(),
        "{}",
        String::from_utf8_lossy(&recent.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&recent.stdout).unwrap();
    assert_eq!(value["source"], "cache");
    assert_eq!(value["mode"], "recent");
    assert_eq!(value["results"].as_array().unwrap().len(), 2);

    let stale = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "search", "--stale"])
        .output()
        .expect("stale search");
    assert!(stale.status.success());
    let value: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert!(value["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"));

    let duplicates = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "search", "--duplicates"])
        .output()
        .expect("duplicate search");
    assert!(duplicates.status.success());
    let value: serde_json::Value = serde_json::from_slice(&duplicates.stdout).unwrap();
    assert_eq!(value["mode"], "duplicates");
    assert_eq!(value["duplicates"][0]["title"], "Roadmap");
    assert_eq!(value["duplicates"][0]["count"], 2);
}

#[test]
fn cache_search_supports_semantic_and_orphaned_modes() {
    let home = temp_dir();
    init_home(home);
    let parent_id = "11111111-1111-1111-1111-111111111111";
    let child_id = "22222222-2222-2222-2222-222222222222";
    let orphan_id = "33333333-3333-3333-3333-333333333333";
    let missing_parent_id = "99999999-9999-9999-9999-999999999999";
    seed_cache_object(
        home,
        "page",
        parent_id,
        "project-home",
        "Project Home",
        "2026-05-04T12:00:00Z",
    );
    seed_cache_object_with_parent(
        home,
        "page",
        child_id,
        "launch-brief",
        "Launch Brief",
        "page_id",
        parent_id,
    );
    seed_cache_object_with_parent(
        home,
        "page",
        orphan_id,
        "lost-launch-note",
        "Lost Launch Note",
        "page_id",
        missing_parent_id,
    );

    let semantic = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "search", "launch", "--semantic"])
        .output()
        .expect("semantic search");
    assert!(
        semantic.status.success(),
        "{}",
        String::from_utf8_lossy(&semantic.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&semantic.stdout).unwrap();
    assert_eq!(value["mode"], "semantic");
    assert!(value["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == child_id && item["score"].as_i64().unwrap() > 0));

    let orphaned = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "search", "--orphaned"])
        .output()
        .expect("orphaned search");
    assert!(
        orphaned.status.success(),
        "{}",
        String::from_utf8_lossy(&orphaned.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&orphaned.stdout).unwrap();
    assert_eq!(value["mode"], "orphaned");
    let results = value["results"].as_array().unwrap();
    assert!(results
        .iter()
        .any(|item| item["id"] == orphan_id && item["parent_id"] == missing_parent_id));
    assert!(!results.iter().any(|item| item["id"] == child_id));
}

#[test]
fn ds_export_writes_cached_rows_as_csv() {
    let home = temp_dir();
    init_home(home);
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    seed_cached_data_source_row(
        home,
        ds_id,
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
        "Fix login",
        "Todo",
        2,
    );
    seed_cached_data_source_row(
        home,
        ds_id,
        "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
        "Ship docs",
        "Done",
        1,
    );

    let out = format!("{home}/tasks.csv");
    let export = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "export",
            &format!("data_source:{ds_id}"),
            "--format",
            "csv",
            "--where",
            "Status=Todo",
            "--out",
            &out,
        ])
        .output()
        .expect("ds export");
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&export.stdout).unwrap();
    assert_eq!(value["row_count"], 1);
    let csv = fs::read_to_string(out).unwrap();
    assert!(csv.contains("Name"));
    assert!(csv.contains("Fix login"));
    assert!(csv.contains("Todo"));
    assert!(!csv.contains("Ship docs"));
}

#[test]
fn ds_bulk_update_and_archive_plan_against_cached_rows() {
    let home = temp_dir();
    init_home(home);
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    seed_cached_data_source_row(
        home,
        ds_id,
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
        "Fix login",
        "Todo",
        2,
    );
    seed_cached_data_source_row(
        home,
        ds_id,
        "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
        "Ship docs",
        "Done",
        1,
    );

    let update = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "bulk-update",
            &format!("data_source:{ds_id}"),
            "--where",
            "Status=Todo",
            "--set",
            "Status=Done",
            "--max-write",
            "5",
        ])
        .output()
        .expect("ds bulk-update");
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&update.stdout).unwrap();
    assert_eq!(value["command"], "ds.bulk-update");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["row_count"], 1);
    assert_eq!(value["target"]["rows"][0]["title"], "Fix login");
    assert_eq!(value["changes"][0]["max_write"], 5);

    let archive = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "bulk-archive",
            &format!("data_source:{ds_id}"),
            "--where",
            "Status=Done",
            "--max-write",
            "1",
        ])
        .output()
        .expect("ds bulk-archive");
    assert!(
        archive.status.success(),
        "{}",
        String::from_utf8_lossy(&archive.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&archive.stdout).unwrap();
    assert_eq!(value["command"], "ds.bulk-archive");
    assert_eq!(value["target"]["row_count"], 1);
    assert_eq!(value["target"]["rows"][0]["title"], "Ship docs");
}

#[test]
fn ds_deduplicate_plans_duplicate_archives_from_cached_rows() {
    let home = temp_dir();
    init_home(home);
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    seed_cached_data_source_row(
        home,
        ds_id,
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
        "Fix login",
        "Todo",
        2,
    );
    seed_cached_data_source_row(
        home,
        ds_id,
        "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
        "Fix login",
        "Done",
        1,
    );
    seed_cached_data_source_row(
        home,
        ds_id,
        "ffffffff-ffff-ffff-ffff-ffffffffffff",
        "Ship docs",
        "Todo",
        1,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "deduplicate",
            &format!("data_source:{ds_id}"),
            "--by",
            "Name",
            "--keep",
            "newest",
            "--max-write",
            "10",
        ])
        .output()
        .expect("ds deduplicate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "ds.deduplicate");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["source"], "cache");
    assert_eq!(value["target"]["group_count"], 1);
    assert_eq!(value["target"]["archive_count"], 1);
    assert_eq!(value["target"]["groups"][0]["key"], "fix login");
    assert_eq!(value["changes"][0]["max_write"], 10);
}

#[test]
fn bulk_rename_plans_title_changes_from_cache() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "draft-alpha",
        "Draft Alpha",
        "2026-05-04T12:00:00Z",
    );
    seed_cache_object(
        home,
        "page",
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "draft-beta",
        "Draft Beta",
        "2026-05-04T13:00:00Z",
    );
    seed_cache_object(
        home,
        "page",
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
        "final-gamma",
        "Final Gamma",
        "2026-05-04T14:00:00Z",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "bulk",
            "rename",
            "--pattern",
            "Draft",
            "--replace",
            "Final",
            "--max-write",
            "5",
        ])
        .output()
        .expect("bulk rename");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "bulk.rename");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["candidate_count"], 2);
    assert_eq!(value["target"]["renames"][0]["old_title"], "Draft Beta");
    assert_eq!(value["target"]["renames"][0]["new_title"], "Final Beta");
    assert_eq!(value["changes"][0]["max_write"], 5);
}

#[test]
fn ds_import_plans_csv_and_jsonl_rows() {
    let home = temp_dir();
    init_home(home);
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    let csv = format!("{home}/import.csv");
    fs::write(
        &csv,
        "Name,Status,Priority\n\"Fix, login\",Todo,2\nShip docs,Done,1\n",
    )
    .unwrap();

    let csv_import = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "import",
            &format!("data_source:{ds_id}"),
            "--csv",
            &csv,
        ])
        .output()
        .expect("ds import csv");
    assert!(
        csv_import.status.success(),
        "{}",
        String::from_utf8_lossy(&csv_import.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&csv_import.stdout).unwrap();
    assert_eq!(value["command"], "ds.import");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["row_count"], 2);
    assert!(value["target"]["planned"][0]["set"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "Name=Fix, login"));

    let jsonl = format!("{home}/import.jsonl");
    fs::write(
        &jsonl,
        [
            r#"{"ExternalID":"gh:123","Name":"Fix login","Status":"Todo"}"#,
            r#"{"ExternalID":"gh:124","Name":"Ship docs","Status":"Done"}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    let jsonl_import = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "import",
            &format!("data_source:{ds_id}"),
            "--jsonl-file",
            &jsonl,
            "--upsert-key",
            "ExternalID",
        ])
        .output()
        .expect("ds import jsonl");
    assert!(
        jsonl_import.status.success(),
        "{}",
        String::from_utf8_lossy(&jsonl_import.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&jsonl_import.stdout).unwrap();
    assert_eq!(value["target"]["planned"][0]["op"], "row.upsert");
    assert_eq!(value["changes"][0]["upsert_key"], "ExternalID");
}

#[test]
fn row_relate_builds_relation_patch_plan() {
    let home = temp_dir();
    init_home(home);
    let row_id = "dddddddd-dddd-dddd-dddd-dddddddddddd";
    let related_id = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee";
    seed_cache_object(
        home,
        "page",
        row_id,
        "task-a",
        "Task A",
        "2026-05-04T12:00:00Z",
    );
    seed_cache_object(
        home,
        "page",
        related_id,
        "task-b",
        "Task B",
        "2026-05-04T12:00:00Z",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "row",
            "relate",
            "Task A",
            "Depends On",
            "Task B",
            "--by-title",
        ])
        .output()
        .expect("row relate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "row.relate");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["target"]["id"], row_id);
    assert_eq!(value["changes"][0]["relation_prop"], "Depends On");
    assert_eq!(value["changes"][0]["related"]["id"], related_id);
}

#[test]
fn ds_move_builds_parent_change_plan() {
    let home = temp_dir();
    init_home(home);
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    let db_id = "99999999-9999-9999-9999-999999999999";
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "move",
            &format!("data_source:{ds_id}"),
            &format!("database:{db_id}"),
        ])
        .output()
        .expect("ds move");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "ds.move");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["data_source"]["id"], ds_id);
    assert_eq!(value["target"]["new_database"]["id"], db_id);
}

#[test]
fn ds_schema_diff_validate_and_lint_use_cached_schema() {
    let home = temp_dir();
    init_home(home);
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    seed_cached_data_source_schema(home, ds_id);

    let get = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "schema",
            &format!("data_source:{ds_id}"),
        ])
        .output()
        .expect("ds schema");
    assert!(
        get.status.success(),
        "{}",
        String::from_utf8_lossy(&get.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(value["source"], "cache");
    assert_eq!(value["schema"]["Status"]["type"], "select");

    let desired = format!("{home}/desired-schema.json");
    fs::write(
        &desired,
        serde_json::json!({
            "properties": {
                "Name": { "type": "title" },
                "Status": { "type": "select" },
                "Due": { "type": "date" }
            }
        })
        .to_string(),
    )
    .unwrap();
    let diff = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "schema",
            "diff",
            &format!("data_source:{ds_id}"),
            &desired,
        ])
        .output()
        .expect("ds schema diff");
    assert!(diff.status.success());
    let value: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(value["changed"], true);
    assert!(value["diff"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["property"] == "Due" && item["change"] == "add"));

    let apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "schema",
            "apply",
            &format!("data_source:{ds_id}"),
            &desired,
        ])
        .output()
        .expect("ds schema apply");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(value["command"], "ds.schema.apply");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["changed"], true);

    let rules = format!("{home}/rules.json");
    fs::write(
        &rules,
        serde_json::json!({
            "required_properties": ["Name", "Status"],
            "forbidden_properties": ["Deprecated"],
            "properties": {
                "Status": { "type": "select" }
            }
        })
        .to_string(),
    )
    .unwrap();
    let lint = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "ds",
            "lint",
            &format!("data_source:{ds_id}"),
            "--rules",
            &rules,
        ])
        .output()
        .expect("ds lint");
    assert!(lint.status.success());
    let value: serde_json::Value = serde_json::from_slice(&lint.stdout).unwrap();
    assert_eq!(value["valid"], true);
}

#[test]
fn policy_check_and_global_policy_enforcement_deny_commands() {
    let home = temp_dir();
    let policy = format!("{home}/policy.json");
    fs::write(
        &policy,
        serde_json::json!({
            "allow": ["resolve", "alias.list", "policy.check"],
            "deny": ["alias.set"]
        })
        .to_string(),
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home", home, "policy", "check", &policy, "alias", "set", "roadmap",
        ])
        .output()
        .expect("policy check");
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(value["command_path"], "alias.set");
    assert_eq!(value["allowed"], false);

    let denied = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--policy",
            &policy,
            "alias",
            "set",
            "roadmap",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("policy denied command");
    assert!(!denied.status.success());
    let value: serde_json::Value = serde_json::from_slice(&denied.stderr).unwrap();
    assert_eq!(value["error"]["code"], "permission_denied");

    let allowed = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--policy", &policy, "alias", "list"])
        .output()
        .expect("policy allowed command");
    assert!(
        allowed.status.success(),
        "{}",
        String::from_utf8_lossy(&allowed.stderr)
    );
}

#[test]
fn comment_resolve_plans_then_records_local_resolution() {
    let home = temp_dir();
    init_home(home);

    let plan = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "comment", "resolve", "comment_123"])
        .output()
        .expect("comment resolve plan");
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(value["command"], "comment.resolve");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["status"], "planned");

    let apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "comment",
            "resolve",
            "comment_123",
        ])
        .output()
        .expect("comment resolve apply");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(value["changed"], true);
    assert_eq!(value["target"]["status"], "resolved");

    let db = format!("{home}/profiles/default/cache.sqlite");
    let rows = Command::new("sqlite3")
        .args([
            "-json",
            &db,
            "SELECT status FROM comment_resolutions WHERE comment_id = 'comment_123'",
        ])
        .output()
        .expect("query comment resolutions");
    assert!(
        rows.status.success(),
        "{}",
        String::from_utf8_lossy(&rows.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&rows.stdout).unwrap();
    assert_eq!(value[0]["status"], "resolved");
}

#[test]
fn page_links_mentions_and_files_use_cached_page_json() {
    let home = temp_dir();
    init_home(home);
    let page_id = "ffffffff-ffff-ffff-ffff-ffffffffffff";
    seed_cached_page_artifacts(home, page_id);

    let links = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "page", "links", &format!("page:{page_id}")])
        .output()
        .expect("page links");
    assert!(links.status.success());
    let value: serde_json::Value = serde_json::from_slice(&links.stdout).unwrap();
    assert!(value["links"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["url"] == "https://example.com/spec"));

    let mentions = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "page",
            "mentions",
            &format!("page:{page_id}"),
        ])
        .output()
        .expect("page mentions");
    assert!(mentions.status.success());
    let value: serde_json::Value = serde_json::from_slice(&mentions.stdout).unwrap();
    assert!(value["mentions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["user"]["id"] == "user_123"));

    let files = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "page", "files", &format!("page:{page_id}")])
        .output()
        .expect("page files");
    assert!(files.status.success());
    let value: serde_json::Value = serde_json::from_slice(&files.stdout).unwrap();
    assert!(value["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["url"] == "https://example.com/brief.pdf"));
}

#[test]
fn meeting_list_and_get_actions_use_cached_meeting_blocks() {
    let home = temp_dir();
    init_home(home);
    let block_id = "abcabcab-cabc-4abc-8abc-abcabcabcabc";
    seed_cached_meeting_block(home, block_id);

    let list = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "meeting", "list"])
        .output()
        .expect("meeting list");
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert!(value["meetings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == block_id && item["action_count"] == 2));

    let get = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "meeting",
            "get",
            block_id,
            "--summary",
            "--transcript",
            "--actions",
        ])
        .output()
        .expect("meeting get");
    assert!(
        get.status.success(),
        "{}",
        String::from_utf8_lossy(&get.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&get.stdout).unwrap();
    assert!(value["summary"]
        .as_str()
        .unwrap()
        .contains("Weekly planning"));
    assert_eq!(value["actions"].as_array().unwrap().len(), 2);
    assert!(value["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["text"] == "Brian drafts launch notes"));
}

#[test]
fn workflow_run_plans_and_executes_json_workflows() {
    let home = temp_dir();
    init_home(home);
    let workflow_dir = format!("{home}/workflows");
    fs::create_dir_all(&workflow_dir).unwrap();
    fs::write(
        format!("{workflow_dir}/launch.json"),
        serde_json::json!({
            "steps": [
                {
                    "op": "alias.set",
                    "name": "{{ALIAS}}",
                    "reference": "page:16d8004e5f6a42a6981151c22ddada12"
                }
            ]
        })
        .to_string(),
    )
    .unwrap();

    let plan = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "workflow",
            "run",
            "launch",
            "--set",
            "ALIAS=roadmap",
        ])
        .output()
        .expect("workflow plan");
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["step_count"], 1);
    assert!(value["results"].as_array().unwrap().is_empty());

    let apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "workflow",
            "run",
            "launch",
            "--set",
            "ALIAS=roadmap",
        ])
        .output()
        .expect("workflow apply");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(value["dry_run"], false);
    assert_eq!(value["results"].as_array().unwrap().len(), 1);

    let resolve = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "resolve", "roadmap"])
        .output()
        .expect("resolve workflow alias");
    assert!(resolve.status.success());

    fs::write(
        format!("{workflow_dir}/review.yaml"),
        r#"steps:
  - op: alias.set
    name: "{{YAML_ALIAS}}"
    reference: "page:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
"#,
    )
    .unwrap();
    let yaml_apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "workflow",
            "run",
            "review",
            "--set",
            "YAML_ALIAS=brief",
        ])
        .output()
        .expect("workflow yaml apply");
    assert!(
        yaml_apply.status.success(),
        "{}",
        String::from_utf8_lossy(&yaml_apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&yaml_apply.stdout).unwrap();
    assert_eq!(value["step_count"], 1);
    assert_eq!(value["results"].as_array().unwrap().len(), 1);

    let resolve = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "resolve", "brief"])
        .output()
        .expect("resolve yaml workflow alias");
    assert!(resolve.status.success());
}

#[test]
fn template_register_and_apply_render_markdown_plan() {
    let home = temp_dir();
    init_home(home);
    let source = format!("{home}/launch.md");
    fs::write(&source, "# Launch Plan\n\nOwner: {{OWNER}}\n").unwrap();

    let register = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home", home, "template", "register", "launch", "--from", &source,
        ])
        .output()
        .expect("template register");
    assert!(
        register.status.success(),
        "{}",
        String::from_utf8_lossy(&register.stderr)
    );

    let apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "template",
            "apply",
            "launch",
            "--parent",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--set",
            "OWNER=Priya",
        ])
        .output()
        .expect("template apply");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(value["command"], "template.apply");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["title"], "Launch Plan");
    assert!(value["changes"][0]["markdown"]
        .as_str()
        .unwrap()
        .contains("Owner: Priya"));
}

#[test]
fn op_undo_plans_then_executes_stored_inverse_command() {
    let home = temp_dir();
    let set = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "alias",
            "set",
            "roadmap",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("set alias");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );
    seed_oplog(
        home,
        "op_alias_remove",
        "alias.set",
        "notionli alias remove roadmap",
        "complete",
    );

    let plan = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "op", "undo", "op_alias_remove"])
        .output()
        .expect("undo plan");
    assert!(plan.status.success());
    let value: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["status"], "planned");

    let apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--apply", "op", "undo", "op_alias_remove"])
        .output()
        .expect("undo apply");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(value["dry_run"], false);
    assert_eq!(value["status"], "undone");

    let resolve = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "resolve", "roadmap"])
        .output()
        .expect("resolve removed alias");
    assert!(!resolve.status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "op", "status", "op_alias_remove"])
        .output()
        .expect("op status");
    assert!(status.status.success());
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["status"], "undone");
}

#[test]
fn audit_list_and_show_expose_logged_write_receipts() {
    let home = temp_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "comment",
            "resolve",
            "comment_123",
        ])
        .output()
        .expect("comment resolve for audit");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let operation_id = receipt["operation_id"].as_str().unwrap();

    let list = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "audit", "list"])
        .output()
        .expect("audit list");
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert!(value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["operation_id"] == operation_id));

    let show = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "audit", "show", operation_id])
        .output()
        .expect("audit show");
    assert!(
        show.status.success(),
        "{}",
        String::from_utf8_lossy(&show.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&show.stdout).unwrap();
    assert_eq!(value["operation_id"], operation_id);
    assert_eq!(value["command"], "comment.resolve");
    assert_eq!(value["profile"], "default");
    assert_eq!(value["changes"].as_array().unwrap().len(), 1);
}

#[test]
fn direct_local_alias_set_does_not_emit_receipt() {
    let home = temp_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "alias",
            "set",
            "roadmap",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("alias set");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("operation_id").is_none());
    assert_eq!(value["alias"], "roadmap");
}

#[test]
fn batch_apply_plans_and_executes_jsonl_operations() {
    let home = temp_dir();
    init_home(home);
    let ops = format!("{home}/ops.jsonl");
    fs::write(
        &ops,
        [
            r#"{"op":"alias.set","name":"roadmap","reference":"page:16d8004e5f6a42a6981151c22ddada12"}"#,
            r#"{"command":["alias","set","tasks","data_source:248104cd477e80afbc30000bd28de8f9"]}"#,
        ]
        .join("\n"),
    )
    .unwrap();

    let plan = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "batch", "apply", &ops])
        .output()
        .expect("batch plan");
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&plan.stdout).unwrap();
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["count"], 2);
    assert!(value["results"].as_array().unwrap().is_empty());

    let apply = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--apply", "batch", "apply", &ops])
        .output()
        .expect("batch apply");
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&apply.stdout).unwrap();
    assert_eq!(value["dry_run"], false);
    assert_eq!(value["results"].as_array().unwrap().len(), 2);

    let resolve = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "resolve", "roadmap"])
        .output()
        .expect("resolve roadmap");
    assert!(resolve.status.success());
    let value: serde_json::Value = serde_json::from_slice(&resolve.stdout).unwrap();
    assert_eq!(
        value["result"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );
}

#[test]
fn alias_round_trip_uses_local_sqlite_state() {
    let home = temp_dir();
    let set = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "alias",
            "set",
            "roadmap",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("set alias");
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );

    let resolve = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "resolve", "roadmap"])
        .output()
        .expect("resolve alias");
    assert!(
        resolve.status.success(),
        "{}",
        String::from_utf8_lossy(&resolve.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&resolve.stdout).unwrap();
    assert_eq!(
        value["result"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );
}

#[test]
fn page_patch_is_dry_run_by_default() {
    let home = temp_dir();
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "page",
            "patch",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--section",
            "Action Items",
            "--append-text",
            "Follow up.",
        ])
        .output()
        .expect("patch dry run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["changed"], false);
}

#[cfg(unix)]
#[test]
fn page_patch_apply_uses_real_block_api_paths() {
    let home = temp_dir();
    let fake_curl = format!("{home}/fake-page-patch-curl.sh");
    let patch_md = format!("{home}/patch.md");
    fs::write(&patch_md, "New decision.").unwrap();
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
method="GET"
url=""
data=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="$1"
      ;;
    --data)
      shift
      data="$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s %s %s\n' "$method" "$url" "$data" >> "$(dirname "$0")/curl-log"
case "$method $url" in
  GET*\ */blocks/*/children*)
    printf '{"object":"list","results":[{"object":"block","id":"heading-decisions","type":"heading_2","heading_2":{"rich_text":[{"plain_text":"Decisions"}]},"has_children":false},{"object":"block","id":"old-decision","type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Old decision."}]},"has_children":false},{"object":"block","id":"heading-next","type":"heading_2","heading_2":{"rich_text":[{"plain_text":"Next"}]},"has_children":false}],"has_more":false}\n200'
    ;;
  PATCH*\ */blocks/*/children)
    printf '{"object":"list","results":[{"object":"block","id":"new-decision","type":"paragraph","paragraph":{"rich_text":[{"plain_text":"New decision."}]},"has_children":false}],"has_more":false}\n200'
    ;;
  PATCH*\ */blocks/old-decision)
    printf '{"object":"block","id":"old-decision","archived":true}\n200'
    ;;
  *)
    printf '{"message":"unexpected request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "page",
            "patch",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--section",
            "Decisions",
            "--replace-md",
            &patch_md,
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("page patch through fake Notion");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "page.patch");
    assert_eq!(value["changed"], true);
    assert_eq!(value["target"]["mode"], "replace_section");

    let log = fs::read_to_string(format!("{home}/curl-log")).unwrap();
    assert!(log.contains("/blocks/16d8004e-5f6a-42a6-9811-51c22ddada12/children"));
    assert!(log.contains("PATCH https://api.notion.com/v1/blocks/old-decision"));
    assert!(log.contains("\"after\":\"heading-decisions\""));
    assert!(!log.contains("/pages/16d8004e-5f6a-42a6-9811-51c22ddada12/markdown"));
}

#[cfg(unix)]
#[test]
fn page_patch_replace_block_apply_uses_block_update() {
    let home = temp_dir();
    let fake_curl = format!("{home}/fake-page-replace-block-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
method="GET"
url=""
data=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="$1"
      ;;
    --data)
      shift
      data="$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s %s %s\n' "$method" "$url" "$data" >> "$(dirname "$0")/curl-log"
case "$method $url" in
  PATCH*\ */blocks/block_abc)
    printf '{"object":"block","id":"block_abc","type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Updated"}]}}\n200'
    ;;
  *)
    printf '{"message":"unexpected request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "--apply",
            "page",
            "patch",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--op",
            "replace_block",
            "--block",
            "block_abc",
            "--text",
            "Updated",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("page patch replace block through fake Notion");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "page.patch");
    assert_eq!(value["target"]["mode"], "replace_block");

    let log = fs::read_to_string(format!("{home}/curl-log")).unwrap();
    assert!(log.contains("PATCH https://api.notion.com/v1/blocks/block_abc"));
    assert!(log.contains("\"paragraph\""));
    assert!(!log.contains("/pages/16d8004e-5f6a-42a6-9811-51c22ddada12/markdown"));
}

#[cfg(unix)]
#[test]
fn page_worktree_push_apply_uses_block_replacement() {
    let home = temp_dir();
    let worktree = format!("{home}/roadmap-worktree");
    fs::create_dir_all(&worktree).unwrap();
    fs::write(format!("{worktree}/page.md"), "# Roadmap\n\nUpdated body.").unwrap();
    fs::write(
        format!("{worktree}/notionli-worktree.json"),
        r#"{
  "notionli_worktree_version": 1,
  "target": {
    "object_type": "page",
    "id": "16d8004e-5f6a-42a6-9811-51c22ddada12",
    "title": "Roadmap",
    "url": null
  }
}"#,
    )
    .unwrap();

    let fake_curl = format!("{home}/fake-worktree-push-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
method="GET"
url=""
data=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="$1"
      ;;
    --data)
      shift
      data="$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s %s %s\n' "$method" "$url" "$data" >> "$(dirname "$0")/curl-log"
case "$method $url" in
  GET*\ */blocks/*/children*)
    printf '{"object":"list","results":[{"object":"block","id":"old-heading","type":"heading_1","heading_1":{"rich_text":[{"plain_text":"Roadmap"}]},"has_children":false},{"object":"block","id":"old-body","type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Old body."}]},"has_children":false}],"has_more":false}\n200'
    ;;
  PATCH*\ */blocks/old-heading|PATCH*\ */blocks/old-body)
    printf '{"object":"block","archived":true}\n200'
    ;;
  PATCH*\ */blocks/*/children)
    printf '{"object":"list","results":[{"object":"block","id":"new-heading","type":"heading_1","heading_1":{"rich_text":[{"plain_text":"Roadmap"}]},"has_children":false}],"has_more":false}\n200'
    ;;
  *)
    printf '{"message":"unexpected request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home", home, "--apply", "page", "worktree", "push", &worktree,
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("page worktree push through fake Notion");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "page.worktree.push");
    assert_eq!(value["target"]["mode"], "replace_page");

    let log = fs::read_to_string(format!("{home}/curl-log")).unwrap();
    assert!(log.contains("/blocks/16d8004e-5f6a-42a6-9811-51c22ddada12/children"));
    assert!(log.contains("PATCH https://api.notion.com/v1/blocks/old-heading"));
    assert!(log.contains("PATCH https://api.notion.com/v1/blocks/old-body"));
    assert!(!log.contains("/pages/16d8004e-5f6a-42a6-9811-51c22ddada12/markdown"));
}

#[test]
fn page_duplicate_builds_copy_plan_with_destination() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            temp_dir(),
            "page",
            "duplicate",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--to",
            "page:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ])
        .output()
        .expect("page duplicate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "page.duplicate");
    assert_eq!(value["dry_run"], true);
    assert_eq!(
        value["target"]["source"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );
    assert_eq!(
        value["target"]["to"]["id"],
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
    );
}

#[test]
fn page_edit_builds_editor_round_trip_plan() {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            temp_dir(),
            "page",
            "edit",
            "page:16d8004e5f6a42a6981151c22ddada12",
            "--section",
            "Notes",
            "--append-only",
        ])
        .output()
        .expect("page edit");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "page.edit");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["section"], "Notes");
    assert_eq!(value["target"]["append_only"], true);
}

#[test]
fn page_worktree_checkout_and_push_plan_use_local_files() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );
    let out = format!("{home}/roadmap-worktree");
    let checkout = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home", home, "page", "worktree", "checkout", "roadmap", "--out", &out,
        ])
        .output()
        .expect("page worktree checkout");
    assert!(
        checkout.status.success(),
        "{}",
        String::from_utf8_lossy(&checkout.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&checkout.stdout).unwrap();
    assert_eq!(value["target"]["title"], "Roadmap");
    let markdown_path = format!("{out}/page.md");
    let markdown = fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("# Roadmap"));

    fs::write(&markdown_path, "# Roadmap Updated\n\nShip it.\n").unwrap();
    let push = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "page", "worktree", "push", &out])
        .output()
        .expect("page worktree push");
    assert!(
        push.status.success(),
        "{}",
        String::from_utf8_lossy(&push.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&push.stdout).unwrap();
    assert_eq!(value["command"], "page.worktree.push");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["changes"][0]["title"], "Roadmap Updated");
    assert_eq!(value["changed"], false);
}

#[test]
fn profile_create_use_and_show_round_trip() {
    let home = temp_dir();
    for args in [
        vec!["--home", home, "profile", "create", "work"],
        vec!["--home", home, "profile", "use", "work"],
        vec!["--home", home, "profile", "show", "work"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_notionli"))
            .args(args)
            .output()
            .expect("profile command");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let show_named = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "profile", "show", "work"])
        .output()
        .expect("profile show named");
    assert!(show_named.status.success());
    let value: serde_json::Value = serde_json::from_slice(&show_named.stdout).unwrap();
    assert_eq!(value["profile"], "work");
}

#[test]
fn select_and_selected_support_dot_resolution() {
    let home = temp_dir();
    let set = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "select",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("select");
    assert!(set.status.success());

    let selected = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "selected"])
        .output()
        .expect("selected");
    assert!(selected.status.success());
    let value: serde_json::Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(
        value["selected"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );

    let resolve_dot = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "resolve", "."])
        .output()
        .expect("resolve dot");
    assert!(resolve_dot.status.success());
}

#[test]
fn file_upload_list_and_status_stage_local_files() {
    let home = temp_dir();
    init_home(home);
    let source = format!("{home}/note.txt");
    fs::write(&source, "hello notionli").unwrap();

    let upload = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "file", "upload", &source])
        .output()
        .expect("file upload");
    assert!(
        upload.status.success(),
        "{}",
        String::from_utf8_lossy(&upload.stderr)
    );
    let uploaded: serde_json::Value = serde_json::from_slice(&upload.stdout).unwrap();
    assert_eq!(uploaded["status"], "staged");
    assert_eq!(uploaded["bytes"], 14);
    let file_upload_id = uploaded["file_upload_id"].as_str().unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "file", "status", file_upload_id])
        .output()
        .expect("file status");
    assert!(status.status.success());
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["file_upload_id"], file_upload_id);
    assert_eq!(value["file_name"], "note.txt");

    let list = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "file", "list"])
        .output()
        .expect("file list");
    assert!(list.status.success());
    let value: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert!(value["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["file_upload_id"] == file_upload_id));

    let quiet = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "--quiet", "file", "upload", &source])
        .output()
        .expect("quiet file upload");
    assert!(quiet.status.success());
    assert!(String::from_utf8(quiet.stdout)
        .unwrap()
        .starts_with("file_"));

    let attach = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "file",
            "attach",
            file_upload_id,
            "--page",
            "page:16d8004e5f6a42a6981151c22ddada12",
        ])
        .output()
        .expect("file attach");
    assert!(
        attach.status.success(),
        "{}",
        String::from_utf8_lossy(&attach.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&attach.stdout).unwrap();
    assert_eq!(value["command"], "file.attach");
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["target"]["file"]["source"], "staged_upload");
    assert_eq!(
        value["target"]["target"]["id"],
        "16d8004e-5f6a-42a6-9811-51c22ddada12"
    );
}

#[test]
fn snapshot_create_and_diff_compare_cached_objects() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );
    seed_cache_object(
        home,
        "page",
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "brief",
        "Brief",
        "2026-05-04T12:00:00Z",
    );

    let old_dir = format!("{home}/old-snapshot");
    let old = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "snapshot", "create", "--out", &old_dir])
        .output()
        .expect("snapshot create old");
    assert!(
        old.status.success(),
        "{}",
        String::from_utf8_lossy(&old.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&old.stdout).unwrap();
    assert_eq!(value["snapshot"], "created");
    assert_eq!(value["object_count"], 2);

    update_cache_object_title(
        home,
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "Roadmap Updated",
    );
    delete_cache_object(home, "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    seed_cache_object(
        home,
        "data_source",
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
        "tasks",
        "Tasks",
        "2026-05-04T12:00:00Z",
    );

    let new_dir = format!("{home}/new-snapshot");
    let new = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "snapshot", "create", "--out", &new_dir])
        .output()
        .expect("snapshot create new");
    assert!(new.status.success());

    let diff = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "snapshot", "diff", &old_dir, &new_dir])
        .output()
        .expect("snapshot diff");
    assert!(
        diff.status.success(),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert!(value["added"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["object_id"] == "cccccccc-cccc-cccc-cccc-cccccccccccc"));
    assert!(value["removed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["object_id"] == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"));
    assert!(value["changed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["object_id"] == "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"));
}

#[test]
fn snapshot_restore_page_and_row_build_restorable_plans() {
    let home = temp_dir();
    init_home(home);
    let page_id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    let row_id = "dddddddd-dddd-dddd-dddd-dddddddddddd";
    let ds_id = "248104cd-477e-80af-bc30-000bd28de8f9";
    seed_cache_object(
        home,
        "page",
        page_id,
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );
    seed_cached_data_source_row(home, ds_id, row_id, "Fix login", "Todo", 2);

    let snapshot_dir = format!("{home}/restore-snapshot");
    let create = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "snapshot", "create", "--out", &snapshot_dir])
        .output()
        .expect("snapshot create");
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );

    let page = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "snapshot",
            "restore-page",
            page_id,
            "--from",
            &snapshot_dir,
        ])
        .output()
        .expect("snapshot restore page");
    assert!(
        page.status.success(),
        "{}",
        String::from_utf8_lossy(&page.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&page.stdout).unwrap();
    assert_eq!(value["command"], "snapshot.restore-page");
    assert_eq!(value["dry_run"], true);

    let row = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "snapshot",
            "restore-row",
            row_id,
            "--from",
            &snapshot_dir,
        ])
        .output()
        .expect("snapshot restore row");
    assert!(
        row.status.success(),
        "{}",
        String::from_utf8_lossy(&row.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&row.stdout).unwrap();
    assert_eq!(value["command"], "snapshot.restore-row");
    assert!(value["changes"][0]["property_names"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "Status"));
}

#[test]
fn sync_status_pull_and_diff_use_cache_and_snapshots() {
    let home = temp_dir();
    init_home(home);
    seed_cache_object(
        home,
        "page",
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "roadmap",
        "Roadmap",
        "2026-05-04T12:00:00Z",
    );

    let status = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "sync", "status"])
        .output()
        .expect("sync status");
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["status"], "ready");
    assert_eq!(value["cache"]["object_count"], 1);

    let pull = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "sync",
            "pull",
            "--since",
            "2026-05-01T00:00:00Z",
        ])
        .env_remove("NOTION_API_KEY")
        .output()
        .expect("sync pull");
    assert!(pull.status.success());
    let value: serde_json::Value = serde_json::from_slice(&pull.stdout).unwrap();
    assert_eq!(value["pulled"], 1);

    let mirror = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "sync",
            "run",
            "--mirror-to",
            "vaultli://notion/",
        ])
        .output()
        .expect("sync mirror");
    assert!(
        mirror.status.success(),
        "{}",
        String::from_utf8_lossy(&mirror.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&mirror.stdout).unwrap();
    assert_eq!(value["sync"]["mirror"]["kind"], "vaultli");
    assert_eq!(value["sync"]["mirror"]["object_count"], 1);
    let manifest = format!("{home}/mirrors/notion/manifest.json");
    assert!(fs::metadata(&manifest).is_ok());
    let mirrored = fs::read_to_string(format!(
        "{home}/mirrors/notion/objects/page-aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa.md"
    ))
    .unwrap();
    assert!(mirrored.contains("# Roadmap"));

    let old_dir = format!("{home}/snapshots/001-old");
    let old = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "snapshot", "create", "--out", &old_dir])
        .output()
        .expect("old sync snapshot");
    assert!(old.status.success());

    seed_cache_object(
        home,
        "page",
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "brief",
        "Brief",
        "2026-05-04T12:00:00Z",
    );
    let new_dir = format!("{home}/snapshots/002-new");
    let new = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "snapshot", "create", "--out", &new_dir])
        .output()
        .expect("new sync snapshot");
    assert!(new.status.success());

    let diff = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "sync", "diff"])
        .output()
        .expect("sync diff");
    assert!(
        diff.status.success(),
        "{}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert!(value["added"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["object_id"] == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"));
}

#[cfg(unix)]
#[test]
fn sync_pull_live_search_caches_changed_objects() {
    let home = temp_dir();
    let fake_curl = format!("{home}/fake-sync-pull-curl.sh");
    fs::write(
        &fake_curl,
        r#"#!/bin/sh
method="GET"
url=""
data=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -X)
      shift
      method="$1"
      ;;
    --data)
      shift
      data="$1"
      ;;
    http*)
      url="$1"
      ;;
  esac
  shift
done
printf '%s %s %s\n' "$method" "$url" "$data" >> "$(dirname "$0")/curl-log"
case "$method $url" in
  "POST https://api.notion.com/v1/search")
    printf '{"object":"list","results":[{"object":"page","id":"aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa","url":"https://notion.so/new","last_edited_time":"2026-05-04T13:00:00Z","properties":{"Name":{"type":"title","title":[{"plain_text":"New Roadmap"}]}}},{"object":"page","id":"bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb","url":"https://notion.so/old","last_edited_time":"2026-05-01T09:00:00Z","properties":{"Name":{"type":"title","title":[{"plain_text":"Old Brief"}]}}}],"has_more":false,"next_cursor":null}\n200'
    ;;
  *)
    printf '{"message":"unexpected request","method":"%s","url":"%s"}\n500' "$method" "$url"
    ;;
esac
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_curl).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_curl, permissions).unwrap();

    let pull = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args([
            "--home",
            home,
            "sync",
            "pull",
            "--since",
            "2026-05-03T00:00:00Z",
        ])
        .env("NOTION_API_KEY", "secret_test")
        .env("NOTIONLI_CURL", &fake_curl)
        .output()
        .expect("live sync pull through fake Notion");
    assert!(
        pull.status.success(),
        "{}",
        String::from_utf8_lossy(&pull.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&pull.stdout).unwrap();
    assert_eq!(value["pull"]["mode"], "live-search");
    assert_eq!(value["pull"]["cached_count"], 1);
    assert_eq!(value["pull"]["skipped_since_count"], 1);
    assert_eq!(
        value["pull"]["cached"][0]["id"],
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
    );

    let status = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "sync", "status"])
        .output()
        .expect("sync status after live pull");
    assert!(status.status.success());
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["cache"]["object_count"], 1);
    let log = fs::read_to_string(format!("{home}/curl-log")).unwrap();
    assert!(log.contains("POST https://api.notion.com/v1/search"));
    assert!(log.contains("\"timestamp\":\"last_edited_time\""));
}

fn temp_dir() -> &'static str {
    let dir = tempfile::tempdir().unwrap();
    Box::leak(dir.keep().to_string_lossy().to_string().into_boxed_str())
}

#[cfg(unix)]
fn connect_with_retry(port: u16) -> TcpStream {
    for _ in 0..25 {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            return stream;
        }
        thread::sleep(Duration::from_millis(20));
    }
    TcpStream::connect(("127.0.0.1", port)).expect("connect to localhost test server")
}

#[cfg(unix)]
fn read_http_response(mut stream: TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => bytes.extend_from_slice(&buffer[..read]),
            Err(error) if error.kind() == ErrorKind::ConnectionReset && !bytes.is_empty() => break,
            Err(error) => panic!("read HTTP response: {error}"),
        }
    }
    String::from_utf8(bytes).expect("HTTP response is utf-8")
}

fn init_home(home: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_notionli"))
        .args(["--home", home, "schema", "errors"])
        .output()
        .expect("init home");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn seed_cache_object(
    home: &str,
    object_type: &str,
    object_id: &str,
    slug: &str,
    title: &str,
    updated_at: &str,
) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let raw_json = format!(
        "{{\"object\":\"{}\",\"id\":\"{}\",\"properties\":{{}}}}",
        object_type, object_id
    );
    let sql = format!(
        "INSERT INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('{object_type}', '{object_id}', '{slug}', '{title}', NULL, '{raw_json}', '{updated_at}')"
    );
    let output = Command::new("sqlite3")
        .args([&db, &sql])
        .output()
        .expect("seed sqlite cache");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn seed_cache_object_with_parent(
    home: &str,
    object_type: &str,
    object_id: &str,
    slug: &str,
    title: &str,
    parent_key: &str,
    parent_id: &str,
) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let mut raw = serde_json::json!({
        "object": object_type,
        "id": object_id,
        "parent": {
            "type": parent_key
        },
        "properties": {
            "Name": {
                "type": "title",
                "title": [{ "plain_text": title }]
            }
        }
    });
    raw["parent"][parent_key] = serde_json::json!(parent_id);
    let raw_json = raw.to_string();
    let sql = format!(
        "INSERT INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('{}', '{}', '{}', '{}', NULL, '{}', '2026-05-04T12:00:00Z')",
        object_type.replace('\'', "''"),
        object_id.replace('\'', "''"),
        slug.replace('\'', "''"),
        title.replace('\'', "''"),
        raw_json.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn seed_cached_data_source_row(
    home: &str,
    data_source_id: &str,
    object_id: &str,
    name: &str,
    status: &str,
    priority: i64,
) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let raw_json = serde_json::json!({
        "object": "page",
        "id": object_id,
        "parent": {
            "type": "data_source_id",
            "data_source_id": data_source_id
        },
        "properties": {
            "Name": {
                "type": "title",
                "title": [{ "plain_text": name }]
            },
            "Status": {
                "type": "select",
                "select": { "name": status }
            },
            "Priority": {
                "type": "number",
                "number": priority
            }
        }
    })
    .to_string();
    let sql = format!(
        "INSERT INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('page', '{}', '{}', '{}', NULL, '{}', '2026-05-04T12:00:00Z')",
        object_id.replace('\'', "''"),
        name.to_lowercase().replace(' ', "-").replace('\'', "''"),
        name.replace('\'', "''"),
        raw_json.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn seed_cached_data_source_schema(home: &str, data_source_id: &str) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let raw_json = serde_json::json!({
        "object": "data_source",
        "id": data_source_id,
        "properties": {
            "Name": { "type": "title", "title": {} },
            "Status": { "type": "select", "select": {} },
            "Priority": { "type": "number", "number": {} }
        }
    })
    .to_string();
    let sql = format!(
        "INSERT INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('data_source', '{}', 'tasks', 'Tasks', NULL, '{}', '2026-05-04T12:00:00Z')",
        data_source_id.replace('\'', "''"),
        raw_json.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn seed_cached_page_artifacts(home: &str, page_id: &str) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let raw_json = serde_json::json!({
        "object": "page",
        "id": page_id,
        "url": "https://notion.so/example",
        "properties": {
            "Name": {
                "type": "title",
                "title": [
                    {
                        "plain_text": "Project Brief",
                        "href": "https://example.com/spec",
                        "text": { "content": "Project Brief" }
                    },
                    {
                        "type": "mention",
                        "mention": {
                            "type": "user",
                            "user": { "id": "user_123" }
                        }
                    }
                ]
            },
            "Attachment": {
                "type": "files",
                "files": [
                    {
                        "name": "brief.pdf",
                        "type": "file",
                        "file": {
                            "url": "https://example.com/brief.pdf",
                            "expiry_time": "2026-05-05T00:00:00Z"
                        }
                    }
                ]
            }
        }
    })
    .to_string();
    let sql = format!(
        "INSERT INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('page', '{}', 'project-brief', 'Project Brief', 'https://notion.so/example', '{}', '2026-05-04T12:00:00Z')",
        page_id.replace('\'', "''"),
        raw_json.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn seed_cached_meeting_block(home: &str, block_id: &str) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let raw_json = serde_json::json!({
        "object": "block",
        "id": block_id,
        "type": "meeting_notes",
        "meeting_notes": {
            "rich_text": [
                { "plain_text": "Weekly planning sync" },
                { "plain_text": "- [ ] Brian drafts launch notes" },
                { "plain_text": "Action: Priya reviews schema export" }
            ]
        }
    })
    .to_string();
    let sql = format!(
        "INSERT INTO objects (object_type, object_id, slug, title, url, raw_json, updated_at) VALUES ('block', '{}', 'weekly-planning-sync', 'Weekly planning sync', NULL, '{}', '2026-05-04T12:00:00Z')",
        block_id.replace('\'', "''"),
        raw_json.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn update_cache_object_title(home: &str, object_id: &str, title: &str) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let raw_json = format!(
        "{{\"object\":\"page\",\"id\":\"{}\",\"title\":\"{}\"}}",
        object_id, title
    );
    let sql = format!(
        "UPDATE objects SET title = '{}', raw_json = '{}', updated_at = '2026-05-04T13:00:00Z' WHERE object_id = '{}'",
        title.replace('\'', "''"),
        raw_json.replace('\'', "''"),
        object_id.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn delete_cache_object(home: &str, object_id: &str) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let sql = format!(
        "DELETE FROM objects WHERE object_id = '{}'",
        object_id.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn seed_oplog(home: &str, operation_id: &str, command: &str, inverse: &str, status: &str) {
    let db = format!("{home}/profiles/default/cache.sqlite");
    let sql = format!(
        "INSERT INTO oplog (operation_id, command, target, receipt_json, inverse_command, created_at, status) VALUES ('{operation_id}', '{command}', '{{}}', '{{}}', '{}', '2026-05-04T12:00:00Z', '{status}')",
        inverse.replace('\'', "''")
    );
    sqlite_exec_for_test(&db, &sql);
}

fn sqlite_exec_for_test(db: &str, sql: &str) {
    let output = Command::new("sqlite3")
        .args([db, sql])
        .output()
        .expect("run sqlite");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn has_command_path(command: &serde_json::Value, path: &[&str]) -> bool {
    let Some((head, tail)) = path.split_first() else {
        return true;
    };
    command["subcommands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|subcommand| {
            subcommand["name"] == *head && (tail.is_empty() || has_command_path(subcommand, tail))
        })
}
