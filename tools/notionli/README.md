# notionli

`notionli` is an agent-safe Notion CLI implemented in Rust from the PRD in this
directory. This release marks the v1.0 baseline command surface.

Current implementation highlights:

For agent-facing operating guidance, see [`SKILL.md`](SKILL.md).
For release notes, see [`CHANGELOG.md`](CHANGELOG.md).

- JSON envelopes and structured errors with PRD exit codes.
- Global output controls for compact JSON (`--json`), NDJSON streams
  (`--jsonl`), quick IDs (`--quiet`), and simple tables (`--format table`).
- OAuth auth via `auth login` for public Notion connections, storing
  credentials under `~/.config/notionli`.
- Legacy integration-token auth via `NOTION_API_KEY`,
  `~/.config/NOTION_API_KEY`, `--token-cmd`, or macOS Keychain.
- Rate-limit retries for live Notion API calls with global `--retry N`.
- Local profile state under `~/.local/share/notionli`, backed by `cache.sqlite`
  through the system `sqlite3` command.
- Aliases, selected target (`.`), local object resolution, operation receipts,
  audit log, and dry-run-by-default writes.
- `audit list|show` for reviewing JSONL audit records from applied writes.
- Executable `op undo` for operations with stored inverse commands.
- JSONL `batch apply` with dry-run planning, `--apply` execution, and
  `--continue-on-error` support.
- Cache-backed `bulk rename --pattern OLD --replace NEW` plans and applies
  title cleanup across cached pages or a scoped target.
- Local file staging for dry-run `file upload`, `file list`, and `file status`
  under the active Notionli home.
- Native Notion file uploads for `file upload --apply`, including sequential
  multipart sends through `--multipart`, reusable uploaded-file records, and
  `file attach --apply` for staged/local/uploaded/external files.
- Cache snapshots with `snapshot create --out DIR` and `snapshot diff OLD NEW`
  for local added/removed/changed object review, plus page/row restore plans and
  `--apply` restoration.
- Cache-backed `ds export --format jsonl|csv|md` with simple `KEY=VALUE`
  filtering and optional `--out` file writes.
- Data-source `bulk-update` and `bulk-archive` with cache-backed dry-run plans
  and live Notion writes on `--apply`.
- Cache-backed `ds deduplicate` plans, with `--apply` archiving duplicates
  after keeping the newest or oldest row for a chosen property.
- Data-source `import --csv` and `import --jsonl-file` with dry-run plans, live row
  creation, and optional `--upsert-key` matching.
- Data-source parent moves through `ds move DATA_SOURCE NEW_DATABASE`.
- Row relation updates through `row relate TARGET RELATION_PROP RELATED`.
- Cache-first `ds schema`, `ds schema diff`, `ds schema validate`, and
  `ds schema apply`, plus `ds lint --rules`, for local data-source schema
  review and live schema changes on `--apply`.
- JSON policy checks and global `--policy FILE` enforcement with allow/deny
  command rules.
- Cache-backed `page links`, `page mentions`, and `page files` extraction from
  stored page JSON.
- Page duplication plans and `--apply` copy creation under a `--to` parent.
- Editor-backed `page edit` round-trips through `NOTIONLI_EDITOR`/`EDITOR`
  with optional `--section` and `--append-only`.
- `page worktree checkout|push` for filesystem-based Markdown edit workflows.
- `page patch --apply` maps Markdown edits onto live Notion block APIs for
  append, section replace, whole-page child replacement, insert-after-block,
  replace-block, and remove-block flows.
- Local comment resolution tracking through `comment resolve`, used by
  unresolved comment listing filters.
- Cache-backed `meeting list` and `meeting get --actions`, including simple
  action-item extraction from meeting note text.
- YAML/JSON/JSONL `workflow run` with `--set KEY=VALUE` substitution, dry-run
  planning, and `--apply` execution.
- Markdown `template register` and `template apply` with `{{KEY}}`
  substitution and Notion page creation on `--apply`.
- Lightweight `completion bash|zsh|fish` script generation from the live command
  catalog.
- `sync pull` live-search cache hydration when token auth is configured, with
  cache-only fallback, plus local `sync status` and `sync diff` over the SQLite
  cache and latest snapshot pair.
- `sync run --mirror-to vaultli://notion/` for a file-backed knowledge-base
  mirror.
- Local `webhook list|create|delete` registrations, `webhook serve` localhost
  event capture with optional `--on-event` hooks, and `watch` direct-poll cache
  change detection.
- Offline `mock serve` manifests plus an `--apply` localhost Notion mock server
  for deterministic tests and demos through `NOTIONLI_API_BASE`.
- `fixture record|replay` for deterministic command-output capture.
- `doctor round-trip` dry-run plans and `--apply` live create/fetch/trash
  permission checks.
- Core Notion calls through the system `curl` command.
- Cache-backed `search --recent`, `search --stale`, and `search --duplicates`
  for local discovery without a live API token.
- Cache-backed `search --semantic` relevance ranking and `search --orphaned`
  parent-reference audits.
- Generated command-tree introspection and agent tool schemas through
  `schema commands` and `tools schema --format json-schema|openai|anthropic|mcp`.
- MCP stdio and localhost HTTP JSON-RPC bridge support for `initialize`,
  `tools/list`, and `tools/call` requests.
- `tui` terminal dashboard summary for cache, selected target, and recent ops.
- MVP commands for `search`, `page`, `block`, `db`, `ds`, `row`, `comment`,
  `user`, `op`, `schema`, and `tools`.

Build and run:

```bash
cargo build
NOTION_OAUTH_CLIENT_ID=<client-id> NOTION_OAUTH_CLIENT_SECRET=<client-secret> \
  cargo run -- auth login
cargo run -- auth login --client-id <client-id> --client-secret <client-secret>
cargo run -- auth login --no-browser
cargo run -- auth login --code <returned-code>
mkdir -p ~/.config && printf %s secret_... > ~/.config/NOTION_API_KEY
cargo run -- auth whoami
cargo run -- auth login
cargo run -- --retry 5 auth whoami
cargo run -- doctor round-trip roadmap
cargo run -- --apply doctor round-trip roadmap
cargo run -- alias set tasks data_source:248104cd477e80afbc30000bd28de8f9
cargo run -- row upsert tasks --key ExternalID=gh:123 --set "Status=In Progress"
cargo run -- tools schema page.fetch --format openai --profile readonly
cargo run -- batch apply ops.jsonl --dry-run
cargo run -- bulk rename --pattern Draft --replace Final
cargo run -- file upload ./brief.md
cargo run -- --apply file upload ./brief.md
cargo run -- --apply file upload ./large-brief.pdf --multipart
cargo run -- file attach https://example.com/brief.pdf --page roadmap
cargo run -- --apply file attach ./brief.md --page roadmap
cargo run -- snapshot create --out ./notion-snapshot
cargo run -- snapshot restore-page <page-id> --from ./notion-snapshot
cargo run -- ds export tasks --format csv --out tasks.csv
cargo run -- ds deduplicate tasks --by Name --keep newest
cargo run -- ds bulk-update tasks --where Status=Todo --set Status=Done --max-write 10
cargo run -- ds import tasks --jsonl-file tasks.jsonl --upsert-key ExternalID
cargo run -- ds move tasks archive-db
cargo run -- row relate TASK-123 "Depends On" TASK-122
cargo run -- ds schema diff tasks desired-schema.json
cargo run -- ds schema apply tasks desired-schema.json
cargo run -- --policy notionli.policy.json row update TASK-123 --set Status=Done
cargo run -- page duplicate roadmap --to archive
cargo run -- page edit roadmap --section Notes --append-only
cargo run -- page worktree checkout roadmap --out ./roadmap-worktree
cargo run -- page worktree push ./roadmap-worktree
cargo run -- page links roadmap
cargo run -- comment resolve comment_123 --apply
cargo run -- meeting get <block-id> --actions
cargo run -- workflow run launch --set ALIAS=roadmap --apply
cargo run -- template apply launch --parent roadmap --set OWNER=Priya
cargo run -- completion bash
cargo run -- sync status
cargo run -- sync run --mirror-to vaultli://notion/
cargo run -- webhook create --events page.content_updated --url https://example.com/hook
cargo run -- --apply webhook serve --port 8080 --out ./webhook-events.jsonl
cargo run -- watch --events page.content_updated --all-shared
cargo run -- fixture record --command "schema errors"
cargo run -- mock serve
cargo run -- --apply mock serve --port 8080
cargo run -- search launch --semantic
cargo run -- search --orphaned
cargo run -- mcp serve
cargo run -- mcp serve --http --port 8080
cargo run -- tui
```

Writes are dry-run plans unless `--apply` is supplied.

Release verification:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
bash -n scripts/live_smoke.sh
bash -n scripts/fake_notion_curl.sh
bash -n scripts/release_audit.sh
NOTION_API_KEY=secret_fake \
NOTIONLI_SMOKE_PARENT_PAGE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
NOTIONLI_CURL="$PWD/scripts/fake_notion_curl.sh" \
NOTIONLI_HOME="$(mktemp -d)" \
./scripts/live_smoke.sh
./scripts/release_audit.sh
cargo build --release
cargo package --allow-dirty
```

Live Notion verification requires a workspace shared with the integration and a
real token from `NOTION_API_KEY` or `~/.config/NOTION_API_KEY`. Use a disposable
page/data source because the smoke commands create, update, relate, attach,
archive, and trash objects.

```bash
export NOTIONLI_HOME="$(mktemp -d)"
notionli auth whoami
notionli doctor api
notionli --apply doctor round-trip page:<shared-page-id>
notionli page create --parent page:<shared-page-id> --title "notionli smoke" --body "hello" --apply
notionli search "notionli smoke" --recent
```

The same smoke flow is available as a script:

```bash
export NOTIONLI_SMOKE_PARENT_PAGE=<shared-page-id>
./scripts/live_smoke.sh
```

For CI or offline release checks, point `NOTIONLI_CURL` at
`scripts/fake_notion_curl.sh` to validate the smoke command sequence without
touching a real workspace.

`scripts/release_audit.sh` runs the local release gate bundle and emits a final
JSON status object. To include the localhost socket mock integration, run with
`NOTIONLI_RUN_SOCKET_TESTS=1` in an environment where port binding is allowed.
To include a real workspace smoke in that bundle, set `NOTIONLI_RUN_LIVE_SMOKE=1`
along with the parent page variable and a token in either `NOTION_API_KEY` or
`~/.config/NOTION_API_KEY`.
