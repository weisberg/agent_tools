#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
crate_dir="$(cd "$script_dir/.." && pwd)"
cd "$crate_dir"

run() {
  echo "+ $*" >&2
  "$@"
}

require_schema_path() {
  local path="$1"
  if ! grep -Eq "\"command\"[[:space:]]*:[[:space:]]*\"$path\"" "$schema_catalog"; then
    echo "schema catalog is missing command path: $path" >&2
    exit 1
  fi
}

require_package_file() {
  local path="$1"
  if ! grep -Fxq "$path" "$package_list"; then
    echo "cargo package list is missing: $path" >&2
    exit 1
  fi
}

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

schema_catalog="$tmp_dir/schema-catalog.json"
package_list="$tmp_dir/package-list.txt"

run cargo fmt --check
run cargo clippy --all-targets -- -D warnings
run cargo test
run bash -n scripts/live_smoke.sh
run bash -n scripts/fake_notion_curl.sh
run bash -n scripts/release_audit.sh

if [[ ! -x "$crate_dir/target/debug/notionli" ]]; then
  run cargo build --quiet
fi
echo "+ offline live_smoke.sh" >&2
NOTION_API_KEY=secret_fake \
NOTIONLI_SMOKE_PARENT_PAGE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
NOTIONLI_CURL="$crate_dir/scripts/fake_notion_curl.sh" \
NOTIONLI_BIN="$crate_dir/target/debug/notionli" \
NOTIONLI_HOME="$tmp_dir/notionli-home" \
./scripts/live_smoke.sh > "$tmp_dir/offline-smoke.jsonl"

run "$crate_dir/target/debug/notionli" --home "$tmp_dir/schema-home" tools list > "$schema_catalog"
require_schema_path "page.fetch"
require_schema_path "page.patch"
require_schema_path "page.worktree.checkout"
require_schema_path "page.worktree.push"
require_schema_path "ds.query"
require_schema_path "ds.deduplicate"
require_schema_path "row.upsert"
require_schema_path "file.upload"
require_schema_path "file.attach"
require_schema_path "webhook.create"
require_schema_path "webhook.serve"
require_schema_path "watch"
require_schema_path "mock.serve"
require_schema_path "fixture.record"
require_schema_path "mcp.serve"
require_schema_path "bulk.rename"
require_schema_path "sync.run"
require_schema_path "sync.pull"
require_schema_path "audit.list"
require_schema_path "audit.show"

run cargo clean --release
run cargo build --release --jobs 1
run cargo package --allow-dirty --list > "$package_list"
require_package_file "README.md"
require_package_file "CHANGELOG.md"
require_package_file "SKILL.md"
require_package_file "scripts/live_smoke.sh"
require_package_file "scripts/fake_notion_curl.sh"
require_package_file "scripts/release_audit.sh"

socket_tests="skipped"
if [[ "${NOTIONLI_RUN_SOCKET_TESTS:-0}" == "1" ]]; then
  run cargo test mock_serve_apply_starts_http_notion_fixture -- --ignored
  run cargo test mcp_serve_http_accepts_jsonrpc_requests -- --ignored
  run cargo test webhook_serve_captures_http_events -- --ignored
  socket_tests="run"
else
  echo "socket tests skipped: set NOTIONLI_RUN_SOCKET_TESTS=1 where localhost binds are allowed." >&2
fi

token_config_path="${XDG_CONFIG_HOME:-$HOME/.config}/NOTION_API_KEY"
token_available=false
if [[ -n "${NOTION_API_KEY:-}" || -s "$token_config_path" ]]; then
  token_available=true
fi

if [[ "$token_available" == "true" && -n "${NOTIONLI_SMOKE_PARENT_PAGE:-}" && "${NOTIONLI_RUN_LIVE_SMOKE:-0}" == "1" ]]; then
  echo "+ live live_smoke.sh" >&2
  NOTIONLI_HOME="${NOTIONLI_HOME:-$tmp_dir/live-home}" ./scripts/live_smoke.sh
else
  echo "live smoke skipped: set NOTION_API_KEY or ~/.config/NOTION_API_KEY, NOTIONLI_SMOKE_PARENT_PAGE, and NOTIONLI_RUN_LIVE_SMOKE=1 to run it." >&2
fi

cat <<JSON
{
  "ok": true,
  "checks": [
    "cargo fmt --check",
    "cargo clippy --all-targets -- -D warnings",
    "cargo test",
    "shell syntax",
    "offline live_smoke.sh",
    "command catalog required paths",
    "cargo build --release",
    "cargo package --allow-dirty --list"
  ],
  "socket_tests": "$socket_tests",
  "live_smoke": "$([[ "$token_available" == "true" && -n "${NOTIONLI_SMOKE_PARENT_PAGE:-}" && "${NOTIONLI_RUN_LIVE_SMOKE:-0}" == "1" ]] && printf run || printf skipped)"
}
JSON
