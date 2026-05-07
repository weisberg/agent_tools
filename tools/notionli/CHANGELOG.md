# Changelog

## 1.0.0

`notionli` 1.0.0 establishes the baseline agent-safe Notion CLI surface.

Highlights:

- Split Rust implementation into focused modules for CLI definitions, context,
  errors, Notion HTTP calls, output, schema/tool introspection, storage, content
  transforms, query compilation, resolution, and command execution.
- Dry-run-by-default write receipts with operation logging and executable undo
  support for commands that have stored inverses.
- OAuth auth through `auth login` for public Notion connections, with credentials
  stored under `~/.config/notionli`, plus legacy integration-token auth through
  `NOTION_API_KEY`, `--token-cmd`, and macOS Keychain.
- Rate-limit retry handling for live Notion API calls.
- Cache-backed search modes for recent, stale, duplicate, semantic, and orphaned
  object discovery.
- Page, block, data-source, row, comment, file, meeting, sync, snapshot, batch,
  template, workflow, webhook, watch, mock, fixture, policy, schema,
  tool-schema, completion, TUI summary, and MCP stdio command groups.
- Data-source CSV/JSONL export and import, bulk update/archive, schema
  diff/validate/apply/lint, parent moves, and cache-backed deduplication
  planning/application.
- Markdown template application, YAML/JSON/JSONL workflows, snapshots, local file
  staging, native Notion file upload and attachment with single-part and
  sequential multipart send support, external URL file attachment, local comment
  resolution state, and editor-backed page edits.
- Cache-backed bulk title renames with scoped dry-run plans and live page title
  updates on `--apply`.
- Page worktrees for checking out cached pages as Markdown and pushing edited
  Markdown back through a dry-run-first replace flow.
- Block-backed `page patch --apply` for common Markdown edit flows, replacing
  the earlier synthetic page-markdown apply path with Notion block append,
  archive, and update requests.
- Local webhook registrations, `webhook serve` localhost event capture with
  optional `--on-event` hook execution, direct-poll watch checkpoints with
  optional `--on-change` hook execution, mock manifests, an `--apply` localhost
  Notion mock server, and fixture record/replay for deterministic offline
  automation.
- `sync run --mirror-to` for file-backed mirrors, including `vaultli://...`
  destinations under the active Notionli home.
- `sync pull` live-search hydration that caches changed Notion objects when
  token auth is configured, while preserving cache-only fallback for offline
  workflows.
- Generated command-tree and tool schemas for JSON Schema, OpenAI, Anthropic,
  and MCP consumers.
- Packaged live smoke-test script at `scripts/live_smoke.sh`, plus
  `scripts/fake_notion_curl.sh` for offline smoke-sequence verification.
- Packaged `scripts/release_audit.sh` to run and summarize the local release
  gate bundle.

Verification:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`
- `cargo package --allow-dirty`
- Offline `scripts/live_smoke.sh` run with `NOTIONLI_CURL` pointed at the fake
  Notion API shim.
- `scripts/release_audit.sh`
- Fake-Notion tests for retry behavior, native single-part and multipart file
  upload/attach, and representative write-heavy apply paths through the HTTP
  wrapper.

Live workspace validation:

- Full live verification requires `NOTION_API_KEY` and a disposable shared
  parent page via `NOTIONLI_SMOKE_PARENT_PAGE`.
- Run `./scripts/live_smoke.sh` from `tools/notionli` to exercise the live
  auth, round-trip, page create/fetch/rename/append/trash, external file attach,
  comment add, sync, and TUI summary flow.
