---
name: notionli
description: |
  Use notionli for agent-safe Notion workspace operations: search, page
  reads, block edits, database (data source) inspection, row upserts by
  external key, comments, and user lookups. Writes default to a dry-run
  plan; pass `--apply` to commit. Trigger on: "update Notion", "search
  the workspace", "upsert a Notion row", "create a Notion comment",
  "find the Notion page about X", "Notion database schema", or any task
  where an agent needs to read or mutate Notion content with auditable,
  rehearsable commands.
---

# notionli

`notionli` is a Rust Notion CLI built for autonomous agents first and
human terminal users second. It provides JSON envelopes, structured
errors, dry-run-by-default writes, local profile state with aliases
and audit, and integration-token auth via `NOTION_API_KEY`,
`--token-cmd`, or the macOS Keychain.

The full operator-facing reference is
[`README.md`](./README.md). The platform contract is in
[`docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md).

## When to use notionli

- Search a Notion workspace for pages, databases, or blocks.
- Inspect a database (data source) schema before writing rows.
- Upsert rows into a Notion database by an external key (idempotent).
- Read a page's blocks for an agent context window.
- Add or read comments on a page or block.
- Resolve a known Notion target via aliases instead of long UUIDs.
- Rehearse a write workflow against a real workspace without touching
  it (dry-run is the default).

## When **not** to use notionli

- The task is to bulk-export a workspace — Notion's official export is
  a better fit.
- The task is rich-text editing of long-form Notion pages — `notionli`
  exposes structured operations, not a Notion editor.
- The task is to drive Markdown documents — use `mdli` and emit Notion
  rows separately if a Notion sync is also required.

## Agent Contract

- stdout in non-TTY mode is the JSON envelope; diagnostics go to
  stderr.
- **Writes are dry-run by default.** Pass `--apply` to commit.
- Mutations append to a local audit log and create operation
  receipts under `~/.local/share/notionli`.
- Address content by Notion UUID. Create aliases for ergonomics:

  ```bash
  notionli alias set tasks data_source:248104cd477e80afbc30000bd28de8f9
  ```

- Resolve a default selected target with `notionli use <alias>`; then
  subsequent commands can address `.` instead of the alias.
- Auth precedence: `--token-cmd` → `NOTION_API_KEY` → Keychain entry.

## Recommended Workflows

### 1. First-time setup

```bash
export NOTION_API_KEY=secret_...
notionli auth whoami
notionli alias set tasks data_source:<uuid>
notionli use tasks
```

### 2. Search the workspace

```bash
notionli search "Q3 launch plan" --json
```

### 3. Inspect a database before writing

```bash
notionli ds show tasks --json
notionli schema --command "row upsert"
```

Use this to learn the property schema before composing an upsert.

### 4. Idempotent row upsert by external key

```bash
notionli row upsert tasks \
  --key ExternalID=gh:123 \
  --set "Status=In Progress" \
  --set "Title=Fix the dashboard"
```

This is a dry-run plan. To commit:

```bash
notionli row upsert tasks --key ExternalID=gh:123 --set "Status=Done" --apply
```

### 5. Read page context for an agent prompt

```bash
notionli page show <page-uuid> --json
notionli block list <page-uuid> --json
```

### 6. Audit and re-apply a recorded operation

```bash
notionli op list
notionli op show <receipt-id>
```

## Failure Recovery

| Symptom | What to do |
|---|---|
| `E_AUTH_MISSING` | Confirm `NOTION_API_KEY` (or `--token-cmd` / Keychain entry); re-run `notionli auth whoami`. |
| Property not found on upsert | Run `notionli ds show <alias>` and confirm the property name (case-sensitive). |
| Rate-limited (429) | Honor the retry-after; back off and re-issue. Treat as retryable. |
| Plan looks wrong in dry-run | Adjust `--set` flags. Never re-run with `--apply` until the plan reads correctly. |
| Stale alias | `notionli alias set <name> <uuid>` overwrites; `notionli alias list` to audit. |

## Schema discovery

```bash
notionli schema
notionli schema --command "row upsert"
notionli tools         # implementation status by command group
```

`notionli tools` is especially useful for an agent that needs to know
which command surfaces are live versus stubbed.
