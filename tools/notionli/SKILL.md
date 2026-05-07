---
name: notionli
description: |
  Use notionli for agent-safe Notion workspace operations: search, page
  reads, block edits, database/data-source inspection, row upserts,
  comments, users, files, snapshots, sync, and auditable dry-run writes.
  Trigger on Notion automation, schema discovery, database row edits,
  page/block mutations, or any task that needs structured Notion access.
---

# notionli

`notionli` is a Rust Notion CLI built for autonomous agents first and
human terminal users second. It provides JSON envelopes, structured
errors, local profile state, aliases, receipts, audit logs, and
dry-run-by-default writes.

The full operator-facing reference is [`README.md`](./README.md). The
platform contract is in
[`docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md).

## When to use notionli

- Search a Notion workspace for pages, databases, data sources, or blocks.
- Inspect database/data-source schema before writing rows.
- Upsert rows by an external key for idempotent automation.
- Read page blocks for an agent context window.
- Add or read comments on a page or block.
- Resolve recurring Notion targets through aliases instead of raw IDs.
- Rehearse write workflows against a real workspace without touching it.

## When not to use notionli

- Bulk-exporting a whole workspace; Notion's official export is a better fit.
- Rich text editing of long-form pages; `notionli` exposes structured ops.
- Driving Markdown documents; use `mdli` and sync to Notion separately.

## Agent Contract

- stdout in non-TTY mode is the JSON envelope; diagnostics go to stderr.
- Reads may execute directly.
- Writes are dry-run plans unless `--apply` is supplied.
- Mutations append to local audit logs and operation receipts.
- Use aliases for recurring targets so scripts do not depend on raw IDs.
- Prefer `schema` and `tools` before relying on an unfamiliar command shape.
- Use `--jsonl` for stream processing and `--quiet` when piping primary IDs.
- Keep integration tokens out of command output and committed files.

Auth sources include `--token-cmd`, `NOTION_API_KEY`,
`~/.config/NOTION_API_KEY`, stored OAuth credentials, and macOS Keychain.

## Safe Default Workflow

1. Check auth and command surface:

   ```bash
   notionli auth whoami
   notionli tools schema --format mcp
   notionli schema commands
   ```

2. Search or select the target:

   ```bash
   notionli search "Project Plan"
   notionli search "Project Plan" --semantic
   notionli search --recent
   notionli alias set tasks data_source:248104cd477e80afbc30000bd28de8f9
   notionli use tasks
   ```

3. Preview writes:

   ```bash
   notionli row upsert tasks --key ExternalID=gh:123 --set "Status=In Progress"
   ```

4. Apply only when the user asked for mutation:

   ```bash
   notionli row upsert tasks \
     --key ExternalID=gh:123 \
     --set "Status=In Progress" \
     --apply
   ```

5. Use operation receipts to confirm results:

   ```bash
   notionli op list
   notionli op show <receipt-id>
   notionli op undo <receipt-id>
   ```

6. For multi-step work, prefer batch/workflow files over ad hoc shell loops:

   ```bash
   notionli batch apply ops.jsonl
   notionli workflow run launch --set ALIAS=roadmap
   ```

## Command Map

| Need | Command group |
|---|---|
| Authentication | `auth` |
| Search workspace objects | `search` |
| Page operations | `page` |
| Block operations | `block` |
| Database/data-source operations | `db`, `ds` |
| Row create/update/upsert/list | `row` |
| Comments | `comment` |
| Users | `user` |
| Files and attachments | `file` |
| Snapshots and sync | `snapshot`, `sync` |
| Webhook/watch automation | `webhook create`, `webhook serve`, `watch` |
| Batch/template/workflow automation | `batch`, `bulk`, `template`, `workflow` |
| Offline fixtures and mocks | `fixture`, `mock` |
| Policies | `policy` |
| MCP bridge | `mcp serve --stdio`, `mcp serve --http --port 8080` |
| Receipts and audit trail | `op`, `audit` |
| Schemas and command discovery | `schema`, `tools` |

## High-Value Commands

```bash
notionli ds show tasks --json
notionli ds export tasks --format csv --out tasks.csv
notionli ds deduplicate tasks --by Name --keep newest
notionli ds import tasks --jsonl-file tasks.jsonl --upsert-key ExternalID
notionli ds schema diff tasks desired-schema.json
notionli ds schema apply tasks desired-schema.json
notionli page show roadmap --json
notionli block list roadmap --json
notionli page duplicate roadmap --to archive
notionli page edit roadmap --section Notes --append-only
notionli page worktree checkout roadmap --out ./roadmap-worktree
notionli page worktree push ./roadmap-worktree
notionli --apply file upload ./brief.md
notionli --apply file upload ./large-brief.pdf --multipart
notionli --apply file attach ./brief.md --page roadmap
notionli snapshot create --out ./notion-snapshot
notionli snapshot diff old-snapshot new-snapshot
notionli sync run --mirror-to vaultli://notion/
notionli webhook create --events page.content_updated --url https://example.com/hook
notionli --apply webhook serve --port 8080 --out ./webhook-events.jsonl
notionli watch --events page.content_updated --all-shared --on-change ./sync.sh --apply
notionli fixture record --command "schema errors"
notionli mock serve
NOTIONLI_API_BASE=http://127.0.0.1:8080/v1 notionli auth whoami
notionli doctor round-trip roadmap
```

## Local Release Gates

From `tools/notionli`:

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

Live Notion smoke testing requires a token from `NOTION_API_KEY` or
`~/.config/NOTION_API_KEY` and a disposable shared page/data source. Start with
`notionli --apply doctor round-trip <page>`.

## Mutation Policy

- User asks "show", "find", "summarize", "inspect", "list": run reads.
- User asks "draft", "plan", "preview": omit `--apply`.
- User asks "update", "create", "delete", "apply", "write": preview first
  when blast radius is unclear, then rerun with `--apply` when the plan matches.

## Failure Recovery

| Symptom | What to do |
|---|---|
| `E_AUTH_MISSING` | Confirm `NOTION_API_KEY`, `~/.config/NOTION_API_KEY`, token command, OAuth credentials, or Keychain profile. |
| Property not found on upsert | Run `notionli ds show <alias>` and confirm the exact property name. |
| Rate-limited (429) | Honor retry-after, back off, and re-issue. Treat as retryable. |
| Plan looks wrong in dry-run | Adjust keys, aliases, or `--set` values. Do not pass `--apply`. |
| Stale alias | `notionli alias set <name> <uuid>` overwrites; `notionli alias list` audits. |
