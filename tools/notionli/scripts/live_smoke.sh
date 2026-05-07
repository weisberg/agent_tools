#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
crate_dir="$(cd "$script_dir/.." && pwd)"

if [[ -z "${NOTION_API_KEY:-}" && ! -s "${XDG_CONFIG_HOME:-$HOME/.config}/NOTION_API_KEY" ]]; then
  echo "A Notion token is required via NOTION_API_KEY or ~/.config/NOTION_API_KEY." >&2
  exit 2
fi

if [[ -z "${NOTIONLI_SMOKE_PARENT_PAGE:-}" ]]; then
  echo "NOTIONLI_SMOKE_PARENT_PAGE is required. Use a disposable shared page ID." >&2
  exit 2
fi

export NOTIONLI_HOME="${NOTIONLI_HOME:-$(mktemp -d)}"

run() {
  echo "+ notionli $*" >&2
  if [[ -n "${NOTIONLI_BIN:-}" ]]; then
    "$NOTIONLI_BIN" "$@"
  else
    cargo run --quiet --manifest-path "$crate_dir/Cargo.toml" -- "$@"
  fi
}

parent="page:${NOTIONLI_SMOKE_PARENT_PAGE}"
smoke_title="notionli smoke $(date -u +%Y%m%dT%H%M%SZ)"

run auth whoami
run doctor api
run --apply doctor round-trip "$parent"

created_json="$(run --apply page create \
  --parent "$parent" \
  --title "$smoke_title" \
  --body "Created by notionli live smoke test.")"

created_id="$(printf '%s' "$created_json" | sed -n 's/.*"id": *"\([^"]*\)".*/\1/p' | head -n 1)"
if [[ -z "$created_id" ]]; then
  echo "Could not parse created page ID from page create output." >&2
  printf '%s\n' "$created_json" >&2
  exit 1
fi

run page fetch "page:$created_id" --format md
run page links "page:$created_id"
run --apply page rename "page:$created_id" "$smoke_title renamed"
run --apply page append "page:$created_id" --text "Append check."
run --apply file attach "https://example.com/notionli-smoke.txt" --page "page:$created_id"
upload_path="$NOTIONLI_HOME/notionli-smoke-upload.txt"
printf 'notionli upload smoke\n' > "$upload_path"
uploaded_file_id="$(run --quiet --apply file upload "$upload_path")"
run --apply file attach "$uploaded_file_id" --page "page:$created_id"
run --apply comment add --page "page:$created_id" --text "notionli smoke comment"
run --apply page trash "page:$created_id"

run sync status
run tui

echo "notionli live smoke completed. NOTIONLI_HOME=$NOTIONLI_HOME" >&2
