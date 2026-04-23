# framerli — Product Requirements Document & Development Plan

**Status:** Draft v0.1
**Author:** Brian Weisberg
**Last updated:** 2026-04-23
**Target:** Open-source, agent-native CLI for the Framer Server API

---

## TL;DR

`framerli` is a command-line tool that wraps the Framer Server API (currently JS/TS-SDK only, WebSocket-based) into a predictable, non-interactive, agent-native interface. It serves two audiences: AI agents operating in reasoning→action→observation loops (the primary user), and human developers running ad-hoc or scripted Framer operations from a shell (the secondary user).

The Framer Server API is powerful but ergonomically hostile to anything that isn't a bespoke Node script: it's a stateful WebSocket, it has cold-start latency, it exposes ~200 methods across CMS, canvas, code files, localization, redirects, and deploy, and it has no CLI, no REST surface, and no native presence in CI. `framerli` fills that gap.

The tool ships in phases. **v0.1** is a Node-based thin wrapper that covers auth, project info, CMS item CRUD, publish/deploy, and JSON output — enough to run Framer from cron or Claude Code and prove the design. **v0.2** adds assets, `cms sync` adapters, and an MCP server mode. **v0.3** introduces a daemon architecture for warm connections and the declarative `site.yaml` layer. **v1.0** is a stable, distributable binary (Node + pkg, with optional Rust-daemon fast path) with a Claude Code skill package.

The bet: agents are the dominant Framer Server API consumer for the next 24 months, and the tool that wins this category will be the one that treats agents as first-class users, not a retrofit on top of a human-oriented CLI.

---

## 1. Problem & Motivation

### 1.1 What's broken today

Framer opened its Server API in early 2026 with an explicit call-out for AI agents, MCP servers, and external sync workflows. The API is substantial — it shares ~90% of its surface with the Plugin API (CMS, canvas, code files, localization, redirects, styles, assets, custom code) and adds server-only methods (`publish`, `deploy`, `getChangedPaths`, `getChangeContributors`, `disconnect`).

But the only way to use it is a Node.js SDK (`framer-api`) over a stateful WebSocket. That creates five concrete friction points:

1. **No CLI.** Every script is a bespoke `node script.js` that has to handle `connect`/`disconnect`, stringify args, parse results, and shape output.
2. **Stateful WebSocket with cold starts.** Each new script pays a 1–2s connection tax. Agents that fire bursts of operations get hit on every invocation.
3. **Not agent-shaped.** The SDK returns unstructured JS objects, throws on errors in ways that don't map to exit codes, and has no notion of idempotency, dry-run, diff, or structured progress.
4. **No declarative model.** There is no "here is the desired state of my CMS schema / redirects / styles — reconcile it" primitive. Every workflow is imperative.
5. **No first-class MCP or CI story.** Framer's own marketing calls out MCP as a motivating example, but there's no turnkey way to expose the Server API as an MCP tool server.

### 1.2 Who's trying to do what today

Three workloads dominate real-world usage, based on Framer's own examples, community posts, and third-party plugin marketplaces (FramerSync, AnySync, FloNocode's Notion→Framer tutorials):

- **Content sync.** Pull from Notion / Airtable / Google Sheets / RSS / a CSV into a Framer CMS collection, then publish. Currently requires either a browser-bound plugin (runs only when Framer is open) or a custom Node script.
- **Agent-driven site edits.** Claude Desktop / Claude Code / Cursor driving changes like "update all pricing copy," "add a new blog post," "run an accessibility audit." Currently requires an open Framer with the Framer MCP plugin running — the "tunnel only works while the app is open" constraint is a dealbreaker for cron, CI, and headless agents.
- **CI/CD publish gates.** "Promote the preview to production after tests pass." Currently ad-hoc shell+Node wrappers per team.

`framerli` serves all three directly.

### 1.3 Why now

Three forces make this the right moment:

- The Framer Server API shipped its public beta and is free-to-use during that window — the surface is concrete and the friction cost is zero.
- MCP has gone mainstream: Claude Desktop, Claude Code, Cursor, Windsurf, and ChatGPT all consume MCP servers, making "wrap an API as MCP" a reusable 10×-leverage move.
- Agent-native CLI design has a maturing set of principles (non-interactive defaults, structured output, idempotency, scoped secrets, semantic truncation) — the community has converged on what "good" looks like.

---

## 2. Goals & Non-Goals

### 2.1 Goals

- **G1.** A single-binary CLI that exposes every publicly-documented Framer Server API capability, organized into discoverable command groups.
- **G2.** Agent-first defaults: non-interactive, JSON output on non-TTY stdout, structured errors with exit codes, dry-run on every write, idempotency where the API allows.
- **G3.** A first-class MCP server mode — one command, stdio transport, ready to drop into any MCP-capable host.
- **G4.** CMS sync adapters for the top sources: Notion, Airtable, Google Sheets, RSS, generic HTTP/JSON, CSV. Declarative config. Idempotent. Diffable.
- **G5.** A bi-directional code-files sync so developers can edit Framer code components in their local editor with git, types, and their existing tooling.
- **G6.** A declarative site spec (`site.yaml`) covering CMS schemas, redirects, styles, and custom code — `plan` / `apply` / `diff` semantics.
- **G7.** Secret hygiene: never persist plaintext keys, integrate with keychain on macOS and a session-lease model elsewhere.
- **G8.** Shippable as `npm i -g framerli`, `brew install framerli`, and `npx framerli` — zero-setup from an isolated agent VM.
- **G9.** A Claude Code skill package shipped alongside the CLI so agents discover it with no prompt engineering.

### 2.2 Non-goals

- **Not a GUI or web UI.** Headless only.
- **Not a replacement for the Framer app.** Design work still happens in Framer; `framerli` is for programmatic operations only.
- **Not a competitor to Framer's own plugins** — `framerli` is complementary. Plugins live in the app; `framerli` lives in the shell.
- **Not trying to reverse-engineer or stabilize the WebSocket wire protocol.** We wrap the official SDK. If Framer ships a REST or Rust SDK later, we adapt.
- **Not shipping a web-scraping migration tool from WordPress/Squarespace/Webflow.** Users bring their own CSV/JSON; `framerli` handles the Framer side of the pipe.
- **Not a full static-site framework.** No build step, no templating engine beyond what's useful for `cms sync` adapters.

---

## 3. Background: Framer Server API — what shapes the design

Five API facts that drive every architectural choice below.

| Fact | Source | Implication |
|---|---|---|
| **Stateful WebSocket**, not REST. Cold start 1–2s, then warm. Idle connections are dropped after a window. | Framer Server API FAQ | Daemon or connection-pooling strategy is load-bearing for agent burst workloads. |
| **JS/TS SDK only** (`framer-api` npm package). No other official language SDK. | Framer npm | CLI runtime is Node-first. Rust is a v2+ optimization, not a v0 requirement. |
| **API keys are project-bound and user-bound.** Generated in Site Settings. Authenticate as the user who created them. | Server API Quick Start | Multi-project/multi-profile is day-one. No "org-wide key" exists. |
| **Not transactional.** Scripts can fail partway through. | Server API FAQ | Every mutating command needs explicit failure granularity in output. Progress streaming is mandatory for multi-item ops. |
| **~90% of Plugin API is available** (CMS, canvas, code files, localization, redirects, styles, custom code, assets, traits) plus server-only (`getChangedPaths`, `getChangeContributors`, `publish`, `deploy`). | Server API Reference | Command surface is large. Must be organized into discoverable groups. |

Connection lifecycle, reduced to CLI semantics:

```
framerli invocation
  └─ connect(projectUrl, apiKey)         [~1–2s cold, ~50ms warm]
        └─ run command(s)
              └─ disconnect()            [or `using` auto-disconnect]
```

This single round trip is the per-invocation cost model. It's the single biggest reason a daemon is the right long-term architecture.

---

## 4. Personas & Use Cases

### 4.1 Primary persona: The AI Agent

An AI agent operating in a reasoning→action→observation loop, with shell access to `framerli`. Examples:

- Claude Code editing a Framer site that holds the team's marketing copy
- A Cowork-hosted agent syncing weekly product release notes from Linear into a Framer blog collection
- An n8n / Make workflow calling `framerli cms sync` on a cron

**The agent cares about:**

- Outputs that parse cleanly without regex
- Predictable exit codes so control flow works
- Non-zero exit codes that tell it what to retry vs give up on
- Dry-run before destructive ops, to show the user a diff
- Bounded context: if a command returns 10,000 items, semantic truncation so the agent's context window doesn't explode
- Discoverability: ideally, one command to ask "what can you do?" and get a machine-readable answer

### 4.2 Secondary persona: The Human Developer

A developer comfortable in a terminal — the kind of person running `kubectl`, `gh`, `wrangler`. Examples:

- Shipping a one-off bulk edit to the CMS before a product launch
- Promoting a preview to prod from a CI/CD GitHub Action
- Debugging why a CMS field isn't rendering, by inspecting node attrs from the canvas

**The developer cares about:**

- Tab completion, `--help`, man pages
- Pretty TTY output with color, tables, and progress bars when run interactively
- The escape hatch: `framerli exec script.ts` to run arbitrary SDK code against a warm connection

### 4.3 Representative use cases

1. **Notion→Framer blog sync on cron.** `framerli cms sync --config notion-blog.yaml && framerli publish --promote`
2. **CI gate on marketing site.** GitHub Action: on push, `framerli status` → run link-check → `framerli publish` → post preview URL to PR.
3. **Bulk copy rewrite.** `framerli text replace --from "early access" --to "private beta" --dry-run` → review → rerun without `--dry-run`.
4. **Schema migration.** Product team adds a `category` field to the blog collection. Author edits `schema.yaml`, commits, CI runs `framerli cms schema apply --file schema.yaml`.
5. **MCP in Claude Desktop.** User says "Add a blog post about our Q2 release using our standard template." Claude calls `framerli`-backed MCP tools to draft, preview, and publish.
6. **Code-component dev loop.** Developer runs `framerli code pull ./components/` once; edits `.tsx` files locally with full TS tooling; `framerli code push --watch ./components/` syncs on save.
7. **Site audit.** `framerli project audit` flags oversized images, nodes with unapproved colors, and collections approaching Framer's "module too large" limit.

---

## 5. Design Principles

These are binding for every command.

### 5.1 Agent-native by default

| Principle | Implementation |
|---|---|
| **No silent hangs** | All commands support `--non-interactive` (default when stdout is not a TTY). Never block on y/n prompts. Missing required args → immediate failure with a structured error. |
| **Parsable truth** | `--output json` (default on non-TTY), `--output jsonl` for streams, `--output human` for TTY. Data goes to stdout; logs/progress/warnings go to stderr. No ANSI escapes on non-TTY. |
| **Structured errors** | Every error emits `{error: {code, message, hint, sdk_method, retryable}}` to stdout in JSON mode. Error codes are stable and documented. |
| **Exit codes** | `0` success. `1` general error. `2` usage error. `3` permission denied. `4` not found. `5` conflict / idempotency collision. `6` rate-limited. `7` cold-start timeout. `10` Framer-side error. |
| **Idempotency** | Mutating commands are idempotent by default where the API allows. `cms items add` upserts by ID. For non-idempotent APIs, we add client-side `--if-not-exists` / `--if-match` flags. |
| **Dry-run everywhere** | Every mutating command accepts `--dry-run`. Emits the same structured output shape as a real run but with a `would_*` prefix on change events and no side effects. |
| **Semantic truncation** | For list commands, we paginate by default (`--limit 50`) and include a `truncated: true` flag plus a continuation token in output. Never dump 10k items into an agent's context unasked. |
| **Discoverability** | `framerli tools` prints the full command tree as JSON schema. `framerli explain <cmd>` prints a machine-readable description of what the command does, its args, and its output shape. |

### 5.2 Human-friendly when it's a human

- TTY detection: ANSI colors, tables, progress bars, spinners.
- `--help` with examples for every subcommand.
- A `framerli docs <topic>` command that opens the relevant Framer dev docs in the browser.

### 5.3 Safe by default

- Destructive commands (`remove`, `delete`, `clear`) require `--yes` on non-TTY.
- `deploy` (promote to production) requires either `--yes` or an explicit `--require-approval` flow (see §10.3).
- No command ever persists the API key in a readable file. Keychain + session-lease model only (see §10).


---

## 6. Architecture

### 6.1 Runtime choice

**v0 and v1 are Node.js / TypeScript.** The SDK is Node; fighting that in v0 is premature optimization. We use:

- **Runtime:** Node 22+ (for the `using` keyword path; fall back to explicit `disconnect` on older Node).
- **CLI framework:** `commander` or `oclif` — leaning toward `commander` for lighter bundle. TBD in Phase 0 spike.
- **Config:** `cosmiconfig` for `framerli.toml` / `.framerlirc` discovery; `zod` for schema validation.
- **MCP:** `@modelcontextprotocol/sdk` stdio transport.
- **Bundling:** `pkg` or `@vercel/ncc` for single-file distribution. Homebrew formula wraps the bundled binary.

**v2 may introduce a Rust CLI with a Node daemon sidecar** — see §6.3.

### 6.2 Execution modes

`framerli` runs in one of three modes, transparent to most commands:

```
┌──────────────────┐   in-process   ┌──────────────────┐
│  framerli <cmd>  ├───────────────▶│  framer-api SDK  │
│   (Node 22+)     │                │   (WebSocket)    │
└──────────────────┘                └──────────────────┘
     (default v0)

┌──────────────────┐   stdio IPC    ┌──────────────────┐    ┌──────────────────┐
│  framerli <cmd>  ├───────────────▶│   framerd        ├───▶│  framer-api SDK  │
│    (thin CLI)    │                │  (warm daemon)   │    │   (WebSocket)    │
└──────────────────┘                └──────────────────┘    └──────────────────┘
     (v0.3+)

┌──────────────────┐   MCP stdio    ┌──────────────────┐    ┌──────────────────┐
│  Claude / host   ├───────────────▶│ framerli mcp     ├───▶│  framer-api SDK  │
│   (agent UI)     │                │ (tool server)    │    │   (WebSocket)    │
└──────────────────┘                └──────────────────┘    └──────────────────┘
     (v0.2+)
```

- **In-process mode.** Default for v0/v1. Every invocation spawns Node, runs, disconnects. Simple. Cold-start tax paid every time.
- **Daemon mode (`framerd`).** v0.3+. A long-running local process holds warm WebSocket connections keyed by `(project, api-key)`. CLI invocations talk to it over a Unix domain socket (`$XDG_RUNTIME_DIR/framerli/framerd.sock`). Opt in via `framerli daemon start` or set `FRAMERLI_DAEMON=1`. Auto-spawn on first use is a v0.4 consideration.
- **MCP mode.** v0.2+. `framerli mcp` runs the process as an MCP stdio server, exposing each command as an MCP tool with a JSON-schema input/output contract.

### 6.3 Why a daemon (and why not in v0)

The cold-start tax is real but small in absolute terms (1–2s). For a one-shot command from CI or cron, it's fine. For an agent firing 20 ops in a session, it's 20–40s of wasted latency.

`framerd` fixes this. Architecturally, this is the same pattern behind `sqlservd` (Athena daemon with warm auth) and `agentcli` (Rust daemon with UDS transport). The rewrite effort is real, though — Rust CLI + Node daemon adds a cross-language boundary. The v0 plan keeps everything in Node and defers the split until we've seen enough agent sessions to justify it.

**Decision:** Node-only through v0.2. Evaluate daemon in v0.3 based on observed agent latencies. If latency matters, split into Rust CLI + Node daemon (like `agentcli`); if not, keep a single Node daemon and a Node thin-client CLI.

### 6.4 Configuration hierarchy

Resolution order (highest wins):

1. CLI flags
2. Environment variables (`FRAMER_API_KEY`, `FRAMERLI_PROJECT`, `FRAMERLI_PROFILE`, …)
3. Project-local `framerli.toml` in CWD or any parent directory
4. Global `~/.config/framerli/config.toml`
5. Built-in defaults

Example `framerli.toml`:

```toml
default_profile = "marketing-site"

[profile.marketing-site]
project = "https://framer.com/projects/Sites--aabbccddeeff"
key_source = "keychain"            # or "env:FRAMER_API_KEY", "lease:bitwarden"
output = "json"

[profile.marketing-site.sync]
config = "./sync/notion-blog.yaml"

[profile.playground]
project = "https://framer.com/projects/Sites--112233445566"
key_source = "env:FRAMERLI_PLAYGROUND_KEY"
```

### 6.5 Output model

Non-TTY default is `--output json`. Every command emits exactly one JSON document to stdout. Examples:

**Single-result command:**
```json
{
  "ok": true,
  "data": { "name": "Marketing Site", "id": "abc123", "version": 42 },
  "meta": { "sdk_method": "getProjectInfo", "ms": 1204, "cold_start": true }
}
```

**List command (paginated):**
```json
{
  "ok": true,
  "data": {
    "items": [],
    "count": 50,
    "truncated": true,
    "cursor": "eyJvZmZzZXQiOjUwfQ=="
  },
  "meta": { "sdk_method": "getItems", "ms": 890 }
}
```

**Error:**
```json
{
  "ok": false,
  "error": {
    "code": "E_SLUG_COLLISION",
    "message": "Slug 'hello-world' already exists in collection 'Blog'.",
    "hint": "Pass --update to upsert by slug, or change the slug field.",
    "sdk_method": "addItems",
    "retryable": false,
    "details": { "collection": "Blog", "slug": "hello-world" }
  },
  "meta": { "ms": 412 }
}
```

**Streaming (jsonl):**
```
{"event":"progress","item":"post-1","status":"synced","ms":120}
{"event":"progress","item":"post-2","status":"synced","ms":98}
{"event":"progress","item":"post-3","status":"skipped","reason":"unchanged"}
{"event":"summary","total":3,"synced":2,"skipped":1,"failed":0,"ms":418}
```

---

## 7. Command Reference

Grouped by API area. Every group is a noun; every terminal command is a verb. Commands marked ★ are high-leverage features where `framerli` adds meaningful value over a thin SDK wrapper.

### 7.1 Auth & profile

| Command | Description |
|---|---|
| `framerli auth login` | Prompt for API key (or read from stdin / flag), store in keychain. |
| `framerli auth list` | List stored credentials by profile. Keys are displayed as `****last4`. |
| `framerli auth remove <profile>` | Remove a stored credential. |
| `framerli auth test` | Verify credential by connecting and calling `getProjectInfo`. |
| `framerli project use <profile-or-url>` | Set active profile for the current shell or project. |
| `framerli project info` | Wraps `getProjectInfo()` and `getPublishInfo()`. |
| `framerli project audit` ★ | Scans the project for oversized images, collections near module limits, unused assets, nodes violating design tokens. Emits structured findings. |
| `framerli whoami` | Wraps `getCurrentUser()`. |
| `framerli can <method>` | Wraps `isAllowedTo()`. Agent pre-flight check before a mutation. |

### 7.2 CMS — item CRUD and bulk ops

| Command | Description |
|---|---|
| `framerli cms collections list` | All collections in the project, with managed/unmanaged flag. |
| `framerli cms collection show <slug>` | Collection metadata: fields, item count, slug field, managedBy. |
| `framerli cms fields list <collection>` | Field schema for a collection. |
| `framerli cms fields add <collection> --name Category --type enum --cases 'Tech,Design,Ops'` | Add a field to a managed collection. |
| `framerli cms fields remove <collection> <field-id>` | Remove a field. |
| `framerli cms fields reorder <collection> --order id1,id2,id3` | Reorder. |
| `framerli cms items list <collection> [--where …] [--limit N] [--cursor X] [--jsonl]` | Paginated, streamable item list. |
| `framerli cms items get <collection> <id-or-slug>` | Fetch one item. |
| `framerli cms items add <collection> --file items.ndjson` | Upsert by ID. NDJSON for streaming. |
| `framerli cms items remove <collection> <id...>` | Remove by ID. |
| `framerli cms items reorder <collection> --order …` | Reorder. |
| `framerli cms import <collection> --from csv\|json\|ndjson\|markdown-dir\|rss --map mapping.yaml` ★ | Parse external data, transform per mapping spec, upsert. Handles image download + upload + asset-ID linking automatically. |
| `framerli cms export <collection> --format csv\|json\|ndjson\|markdown-dir` ★ | Round-trippable with `import`. |
| `framerli cms schema dump <collection>` ★ | Emit collection schema as YAML. |
| `framerli cms schema apply --file schema.yaml` ★ | Reconcile collection schema to match YAML. Terraform-style. |
| `framerli cms schema diff --file schema.yaml` ★ | Show pending schema changes without applying. |
| `framerli cms sync --config sync.yaml [--watch]` ★ | Pull from external source, transform, upsert, report. Full diff-driven workflow. |

#### 7.2.1 `cms sync` adapter catalog (v0.2)

Ship with six first-party adapters:

- `notion` — reads a Notion database, maps properties to CMS fields.
- `airtable` — reads an Airtable base/table.
- `gsheets` — reads a Google Sheet.
- `rss` — reads an RSS/Atom feed (blogs, podcasts).
- `http-json` — generic: fetch a JSON URL, JSONPath-extract records, map.
- `csv` / `ndjson` — local file.

Example `sync.yaml`:

```yaml
source:
  type: notion
  database_id: ${NOTION_DB_ID}
  token: ${NOTION_TOKEN}

target:
  collection: Blog
  id_from: source.id           # stable Notion page ID → Framer item ID
  slug_from: source.properties.slug

mapping:
  Title: source.properties.title.title[0].plain_text
  Body:
    from: source.properties.body
    type: formattedText
    format: markdown           # Framer auto-converts
  Cover:
    from: source.properties.cover.files[0].file.url
    type: image
    upload: true               # download + upload to Framer asset store
    resolution: large          # one of: lossless|full|large|medium|small|auto
  PublishedAt:
    from: source.properties.date.date.start
    type: date

rules:
  on_source_missing: skip      # or: delete_in_target
  on_slug_collision: error     # or: suffix, overwrite
  publish_after_sync: preview  # or: promote, false
```

### 7.3 Publish & deploy

| Command | Description |
|---|---|
| `framerli status` ★ | Wraps `getChangedPaths()`. The "git status" of a Framer project. Emits added/removed/modified paths + change count. |
| `framerli contributors [--from <ver>] [--to <ver>]` | Wraps `getChangeContributors()`. |
| `framerli publish [--promote] [--require-approval]` | Preview publish. `--promote` does `publish` + `deploy` in one step. |
| `framerli deploy <deployment-id>` | Promote a specific deployment to production. |
| `framerli deployments list` | Local history of deployments made via `framerli`. |
| `framerli deploy rollback` ★ | Promote the previous known-good production deployment. Uses locally-tracked history (the API doesn't expose a native rollback). |

### 7.4 Canvas / nodes

Power-user and agent-driven surgical edits. Wraps the generic `getNode`, `setAttributes`, `createFrameNode`, `cloneNode`, `removeNode` surface.

| Command | Description |
|---|---|
| `framerli node get <id>` | Fetch one node, attributes included. |
| `framerli node tree [--depth N] [--root <id>]` | Dump a subtree. |
| `framerli node find --type TextNode --where 'text~"TODO"'` ★ | Query nodes by type + attribute predicate. Wraps `getNodesWithType` / `getNodesWithAttribute` with a saner DSL. |
| `framerli node set <id> --attrs '{"text":"…"}'` | Wraps `setAttributes`. |
| `framerli node clone <id> [--parent <id>]` | Clone a node. |
| `framerli node remove <id>` | Requires `--yes` on non-TTY. |

### 7.5 Text — site-wide copy ops

High-leverage specifically because "update all instances of X" is the #1 agent editing task.

| Command | Description |
|---|---|
| `framerli text search <pattern> [--regex] [--page /path]` ★ | Find text across the canvas. |
| `framerli text replace --from "…" --to "…" [--dry-run] [--regex]` ★ | Bulk find-and-replace. Always dry-run on first pass; explicit confirmation for writes. |
| `framerli text list [--page /path]` | Dump all text content. |

### 7.6 Code files

The bi-directional sync is the marquee feature.

| Command | Description |
|---|---|
| `framerli code list` | All code files in the project. |
| `framerli code cat <id>` | Print file content. |
| `framerli code write <id> --file x.tsx` | Update content. |
| `framerli code write <name> --file x.tsx --create` | Create if missing. |
| `framerli code rename <id> <new-name>` | Rename a code file. |
| `framerli code remove <id>` | Remove a code file. |
| `framerli code versions <id>` | Wraps `getVersions()`. |
| `framerli code typecheck <id>` / `framerli code lint <id>` | Wraps SDK type-check/lint. |
| `framerli code pull <dir>` ★ | Pull all code files into a local directory, one file per code file. |
| `framerli code push <dir> [--watch]` ★ | Push local files back. `--watch` syncs on save (fsevents / inotify). |

### 7.7 Assets

| Command | Description |
|---|---|
| `framerli assets upload <path> [--resolution lossless\|full\|large\|medium\|small\|auto]` | Upload one image/file. Returns asset ID. |
| `framerli assets upload --dir ./images/ [--resolution large]` | Batch upload. Streams progress as JSONL. |
| `framerli assets svg add <file.svg>` | Wraps `addSvg`. |

Resolution defaults to `auto`. The `project audit` command flags assets uploaded at `lossless` when a lower level would work, for module-size optimization.

### 7.8 Styles

| Command | Description |
|---|---|
| `framerli styles colors list` / `create --name X --rgba …` / `remove <id>` | Color style CRUD. |
| `framerli styles text list` / `create` / `remove` | Text style CRUD. |
| `framerli styles export --out styles.json` ★ | Declarative dump. |
| `framerli styles apply --file styles.json` ★ | Reconcile to match file. |

### 7.9 Fonts

| Command | Description |
|---|---|
| `framerli fonts list` | All available fonts. |
| `framerli fonts get <family> [--weight N] [--style italic]` | Specific font handle. |

### 7.10 Localization

| Command | Description |
|---|---|
| `framerli i18n locales list` | List locales. |
| `framerli i18n groups list` | List localization groups. |
| `framerli i18n export --locale fr --out fr.json` ★ | Export all localizable strings for a locale. |
| `framerli i18n import --file fr.json` ★ | Batch-set translations. |
| `framerli i18n diff --locale fr` ★ | Report untranslated / stale strings. |

### 7.11 Redirects

| Command | Description |
|---|---|
| `framerli redirects list` | List redirects. |
| `framerli redirects add --from /old --to /new [--expand-locales]` | Add a redirect. |
| `framerli redirects remove <id...>` | Remove redirects. |
| `framerli redirects reorder --order id1,id2,…` | Reorder redirect precedence. |
| `framerli redirects import --file redirects.csv` ★ | WordPress / Webflow migration helper. |

### 7.12 Custom code (script injection)

| Command | Description |
|---|---|
| `framerli custom-code get --location headStart\|headEnd\|bodyStart\|bodyEnd` | Retrieve injected code. |
| `framerli custom-code set --location bodyEnd --file gtm.html` | Inject HTML at a location. |
| `framerli custom-code clear --location bodyEnd` | Clear injected code at a location. |

### 7.13 Declarative: the `site.yaml` layer (v0.3)

One file. One source of truth. Three commands.

| Command | Description |
|---|---|
| `framerli plan -f site.yaml` ★ | Print diff: what would change across schemas, styles, redirects, custom code, sync configs. |
| `framerli apply -f site.yaml [--auto-approve]` ★ | Apply the plan. Emits per-resource progress. |
| `framerli diff -f site.yaml` ★ | Just the diff, no plan narrative. |

Example `site.yaml`:

```yaml
version: 1
project: marketing-site

styles:
  colors:
    - name: Brand/Primary
      hex: "#E20712"
    - name: Brand/Accent
      hex: "#7A3FF2"
  text:
    - name: Display/XL
      family: Inter
      weight: 800
      size: 72

collections:
  - name: Blog
    slug_field: title
    fields:
      - id: title
        type: string
        required: true
      - id: body
        type: formattedText
      - id: cover
        type: image
      - id: category
        type: enum
        cases: [Tech, Design, Ops]
      - id: publishedAt
        type: date

redirects:
  - from: /old-blog/(.*)
    to: /blog/$1

custom_code:
  - location: headEnd
    file: ./public/gtm.html

sync:
  - config: ./sync/notion-blog.yaml
```

### 7.14 Agent-mode primitives

| Command | Description |
|---|---|
| `framerli mcp` ★ | Start MCP stdio server. One tool per command group. JSON-schema I/O. |
| `framerli daemon start\|stop\|status` | Manage the warm-connection daemon. |
| `framerli exec <script.ts>` | Run arbitrary `framer-api` code against the current session. Escape hatch. |
| `framerli introspect [--depth shallow\|full]` ★ | Compact JSON summary of the whole project: collections + field schemas, pages, styles, redirects, code files. The fastest way to give an agent a mental model of an unfamiliar project. |
| `framerli tools` ★ | Print the command tree as JSON schema. For agent discovery. |
| `framerli explain <cmd>` ★ | Machine-readable description of a command's args, output shape, and side effects. |
| `framerli session begin\|end` | Explicit session bracketing for burst workloads. |
| `framerli record <file>` / `replay <file>` | Capture command+output sessions for deterministic agent-eval harnesses. |

---

## 8. The MCP server (`framerli mcp`)

### 8.1 Why it's first-class

Framer's own announcement called out MCP-from-Slack as a motivating example. Claude Desktop, Claude Code, Cursor, Windsurf, and ChatGPT all consume MCP servers. An MCP-shaped `framerli` is the fastest path to adoption.

### 8.2 Tool catalog

Each command group maps to an MCP tool namespace. Not every CLI command becomes a tool — we curate for the agent-useful subset and omit the dangerous escape hatches.

| MCP tool | Wraps CLI |
|---|---|
| `framer_project_info` | `project info` |
| `framer_project_audit` | `project audit` |
| `framer_cms_collections_list` | `cms collections list` |
| `framer_cms_items_list` | `cms items list` (paginated, returns cursor) |
| `framer_cms_items_add` | `cms items add` |
| `framer_cms_import` | `cms import` |
| `framer_cms_sync_run` | `cms sync` |
| `framer_text_search` | `text search` |
| `framer_text_replace` | `text replace` (requires approval) |
| `framer_status` | `status` |
| `framer_publish` | `publish` (requires approval for `--promote`) |
| `framer_introspect` | `introspect` |

Destructive tools (`text_replace`, `publish --promote`, `cms schema apply`, `deploy`) gate on MCP's elicitation flow: the server returns a preview and waits for host approval before running.

### 8.3 Transport

Stdio for v0.2. HTTP/SSE transport is a v0.4 consideration for remote/hosted deployments.

### 8.4 Skills (Claude Code)

Ship `.claude/skills/framerli/SKILL.md` in the repo and as part of the npm package. The skill teaches Claude the common workflows directly:

- `framerli-cms-sync` — how to construct a sync config, validate, run, publish.
- `framerli-copy-edit` — how to search, replace, preview, apply text changes safely.
- `framerli-schema-migration` — how to add fields to a collection idempotently.
- `framerli-publish-with-approval` — the preview → review → promote pattern.

This is the delivery mechanism that matches the rest of the skill-based ecosystem work.

---

## 9. Observability

### 9.1 Stderr log channel

All logs go to stderr. Levels via `FRAMERLI_LOG=debug|info|warn|error` or `-v`/`-q` flags. Format is human on TTY, JSON Lines on non-TTY.

### 9.2 Progress streaming

For long-running ops (`cms sync`, `cms import`, `assets upload --dir`, `code pull`, `code push --watch`, `apply -f site.yaml`), progress events stream to stderr in JSON Lines. Agents can parse them in real time; humans see a progress bar rendered from them.

### 9.3 Metrics

Built-in `--time` flag adds per-call timings (SDK method, network, total) to the `meta` block of every response. Useful for latency debugging and for agents that want to amortize cold-start cost decisions.

### 9.4 Audit log

An append-only local log at `~/.local/share/framerli/audit.ndjson` records every mutating command: timestamp, profile, command, args (with secrets redacted), result, SDK method, duration. Survives across daemon restarts. Disable with `--no-audit` (not recommended for agent-driven environments). Format:

```json
{"ts":"2026-04-23T14:22:01Z","profile":"marketing-site","cmd":"cms items add","args":{"collection":"Blog","file":"items.ndjson"},"result":"ok","count":42,"ms":1203}
```

---

## 10. Security

### 10.1 API key storage

| Context | Storage mechanism |
|---|---|
| macOS | Keychain via `keytar` or direct `security` CLI. |
| Linux | `secret-service` (libsecret) via `keytar`, fallback to `~/.config/framerli/keys` with `0600` perms and a deprecation warning. |
| Windows | Windows Credential Manager via `keytar`. |
| CI/CD | `FRAMER_API_KEY` env var, no persistence. |
| Agent VMs (Cowork / isolated) | Session lease from Bitwarden / 1Password / Keeper / `agent-secrets`. |

### 10.2 Session leases

For agent-driven environments, the recommended pattern is the time-bounded session lease:

1. The orchestrator (human or service account) requests a lease from a secrets provider — a short-lived credential scoped to one project for N minutes.
2. `framerli` reads the lease (`key_source = "lease:bitwarden"` in `framerli.toml`) and holds it only in memory.
3. Lease expires automatically. No key residue on disk or in environment variables an agent might dump.

v0.3 ships with Bitwarden integration (most common in Brian's stack context). 1Password, Keeper, and AWS Secrets Manager follow.

### 10.3 Agent blast-radius controls

Configurable via `framerli.toml` per profile:

```toml
[profile.marketing-site.limits]
max_calls_per_minute = 120
max_deletes_per_session = 5
max_items_per_add_call = 500
require_approval_for = ["deploy", "cms schema apply", "cms collection remove"]
dry_run_first = ["text replace", "cms items remove"]
```

When an agent hits `require_approval_for`, `framerli` exits non-zero with `E_APPROVAL_REQUIRED` and a pending-action token. An MCP host sees this as an elicitation prompt.

### 10.4 Security risk → mitigation matrix

| Risk | Mitigation |
|---|---|
| Credential exfiltration via shell history / `env` dump | Keychain + in-memory-only leases; never accept key on CLI flag in v1+; redact in audit log. |
| Prompt injection causing unintended writes | Strict arg validation (zod schemas), non-interactive enforcement, dry-run-first defaults. |
| Unauthorized site-wide changes | `require_approval_for`, per-session limits, audit log for post-hoc review. |
| Key persistence beyond task lifetime | TTL-based leases; `auth remove` explicitly clears keychain. |
| Malicious code injection via `code write` | Built-in `code typecheck` / `code lint` before `push`; `--strict` flag fails push on any finding. |

---

## 11. Ecosystem Integration

### 11.1 CI/CD

`framerli` is `gh`-shaped and drop-in for GitHub Actions, GitLab CI, CircleCI. Every exit code is documented. Every command supports `--output json`. A `framerli/setup-action` GitHub Action is part of v0.2.

### 11.2 Webhooks

`framerli` is not itself a webhook server — but `framerli` commands are what webhook consumers invoke. Ship example configs for Make, Zapier, n8n, and raw curl → shell.

### 11.3 Competitive positioning

| Feature | Notion/Airtable plugins | Make/Zapier sync | FramerSync / AnySync | **framerli** |
|---|---|---|---|---|
| Automation | Manual (UI click) | Hybrid | Hybrid | **Fully automated** |
| Agent support | None | Low | Low | **Agent-native** |
| Source types | 1 SaaS | Many SaaS | REST APIs | **Any CSV/JSON/HTTP/Notion/Airtable/Sheets/RSS** |
| State management | Plugin-owned | Plugin-owned | Plugin-owned | **User/agent-controlled, declarative** |
| Context window handling | N/A | N/A | N/A | **Semantic truncation + jsonl streaming** |
| Transport | Plugin API (in-browser) | Plugin API | Plugin API | **Server API (WebSocket)** |
| Runs headless | No | Partial | No | **Yes** |
| MCP integration | No | No | No | **Yes (first-class)** |
| Schema-as-code | No | No | No | **Yes** |

The gap `framerli` fills: headless, agent-native, declarative, MCP-ready. Nothing else in the market is all four.

---

## 12. Development Plan

Six phases, roughly 5–7 months of part-time work.

### Phase 0 — Spike & validate (1 week)

**Goal:** Eliminate unknowns before committing to a design.

**Deliverables:**
- Connect to a real Framer project via the SDK, measure cold-start latency in practice.
- Validate all 5 critical SDK methods work as documented: `connect`, `getProjectInfo`, `getCollections`, `addItems`, `publish`.
- Verify `using` keyword works on Node 22+ (may need polyfill); otherwise explicit `disconnect`.
- Decide: `commander` vs `oclif` (run one spike each, pick on bundle size + UX).
- Decide: initial distribution — npm only for v0.1; brew + pkg binary by v0.2.

**Acceptance:** A scratch `framerli-spike project info` works end-to-end against a real project with measured cold-start numbers in a one-page report.

### Phase 1 — v0.1 Core (3 weeks)

**Goal:** A usable thin wrapper for the most common agent workflows. Ships to npm.

**Scope:**
- `auth login`, `auth list`, `auth remove`, `auth test` (with keychain storage)
- `project info`, `project use`, `whoami`, `can`
- `cms collections list`, `cms collection show`, `cms fields list`
- `cms items list` (paginated, `--jsonl`), `cms items get`, `cms items add`, `cms items remove`
- `status`, `publish`, `deploy`, `deployments list`
- `--output json` / `--output jsonl` / `--output human` with TTY autodetection
- Structured error envelope, documented exit codes
- `framerli.toml` config discovery
- Base audit log

**Out of scope (deferred):** assets, text, node, code, i18n, redirects, custom-code, styles, MCP, sync adapters, daemon, declarative.

**Acceptance criteria:**
- Can run an end-to-end "import 50 items from NDJSON → publish preview" workflow from the CLI
- All commands pass a contract test: non-interactive, JSON output on pipe, correct exit codes for the error taxonomy
- `framerli tools` emits a JSON-schema command tree
- Distributed via `npm i -g framerli` (scoped package `@framerli/cli` if `framerli` is taken)
- README + `--help` coverage for every command
- 80%+ unit test coverage on output/error/config code paths

### Phase 2 — v0.2 Assets, sync, MCP (6 weeks)

**Goal:** The two workflows people actually care about (CMS sync, MCP) work out of the box.

**Scope:**
- `assets upload` (single + directory), with resolution flag
- `cms import` / `cms export` for csv, ndjson, json, markdown-dir
- `cms sync --config sync.yaml` with adapters: `notion`, `csv`, `http-json`
- `cms fields add/remove/reorder`, `cms schema dump/diff/apply`
- `cms items reorder`
- Full `text` group: `search`, `replace`, `list`
- Full `redirects` group: `list`, `add`, `remove`, `reorder`, `import`
- `custom-code` group
- `framerli mcp` — stdio transport, curated tool catalog (see §8.2)
- `framerli introspect`
- `framerli explain`
- Homebrew formula

**Acceptance criteria:**
- Notion→Framer blog sync works end-to-end with image upload, including `--watch`
- Claude Desktop can run an `framerli mcp` server and perform a "sync + preview + promote" flow with approval gating
- `cms schema apply` is idempotent (re-running against the current state is a no-op)
- At least one real Framer site migrated onto a `framerli`-managed sync config (dogfood)

### Phase 3 — v0.3 Canvas, code, declarative (6 weeks)

**Goal:** Unlock the full API surface and the Terraform-style declarative story.

**Scope:**
- `node` group: `get`, `tree`, `find`, `set`, `clone`, `remove`
- `code` group: `list`, `cat`, `write`, `rename`, `remove`, `versions`, `typecheck`, `lint`, `pull`, `push --watch`
- `styles` group: full CRUD + `export`/`apply`
- `fonts` group: `list`, `get`
- `i18n` group: `locales list`, `groups list`, `export`, `import`, `diff`
- `project audit` — scans for oversized assets, color-system violations, module-size warnings
- **Daemon spike:** stand up `framerd`, measure warm-vs-cold latency gain in agent workloads; decide Rust split
- **Declarative `site.yaml`:** `plan`, `apply`, `diff` with schemas, styles, redirects, custom code, sync
- Additional sync adapters: `airtable`, `gsheets`, `rss`
- Session lease integration: Bitwarden

**Acceptance criteria:**
- `code pull` → edit in VS Code with Framer types installed → `code push --watch` works cleanly
- `site.yaml apply` produces the same end state whether run on a fresh project or an existing one
- Observed p50 latency for a 20-command agent session with daemon is <10s total (vs ~30s without)

### Phase 4 — v0.4 Hardening & rollback (4 weeks)

**Goal:** Production-worthy.

**Scope:**
- `deploy rollback` with local deployment-history tracking
- `framerli daemon auto-spawn`: CLI detects no daemon, spawns one, client reconnects
- Blast-radius controls: `require_approval_for`, `max_calls_per_minute`, etc.
- `record` / `replay` for deterministic test harnesses
- Claude Code skill package (`.claude/skills/framerli/`)
- Full distribution: npm, Homebrew, single-file binary via `pkg`
- MCP HTTP/SSE transport evaluation
- Docs site (VitePress or similar)

**Acceptance criteria:**
- `framerli record session.ndjson` → `framerli replay session.ndjson` against a reset project produces byte-identical audit output
- The Claude Code skill enables a fresh agent session to complete the Notion sync workflow end-to-end with no additional prompting
- Install → first successful command in <60s on a clean macOS box

### Phase 5 — v1.0 launch (2 weeks)

**Goal:** Stable public release.

**Scope:**
- Semantic-versioning guarantees declared
- `framerli --version`, `framerli doctor`
- Telemetry (opt-in): anonymous command-usage counts to inform future priorities
- Launch posts: personal blog, Framer Community, Hacker News, r/FramerDev, threads/Twitter
- Sample repos: `framerli-example-notion-blog`, `framerli-example-ci-publish`, `framerli-example-mcp`

**Acceptance criteria:**
- 50 GitHub stars within 30 days (soft signal; launch-worthy quality is the real bar)
- Zero breaking changes required within 60 days of launch
- At least 3 external users running `framerli` in production

---

## 13. Testing Strategy

| Layer | Approach |
|---|---|
| Unit | `vitest`. Target: config parsing, output formatting, error mapping, exit-code logic, sync mapping transforms. Aim for 80%+ on these. |
| Integration | Mocked `framer-api` via a recorded-fixture harness. Record real SDK responses once with `nock` or equivalent; replay in CI. |
| E2E | A staging Framer project + a dedicated API key, exercised in GitHub Actions against every mutating command. Tagged `@live` and run on release branches only. |
| MCP | Model Context Protocol Inspector (`mcp-inspector`) tests for tool schema correctness + approval flow. |
| Agent eval | The `record` / `replay` harness (Phase 4): agent sessions captured as NDJSON, replayed deterministically on PR to catch regressions in the agent-useful subset of behaviors. |
| Contract | Every command must pass: (a) returns JSON on non-TTY, (b) returns non-zero exit code on error, (c) redacts secrets in audit log, (d) supports `--dry-run` if it mutates. Enforced in a shared test matrix. |

---

## 14. Distribution & Packaging

| Channel | Form | Target release |
|---|---|---|
| npm | `@framerli/cli` (global + programmatic API exports) | v0.1 |
| Homebrew | `brew install framerli` tap, then upstream to `homebrew-core` post-v1.0 | v0.2 |
| Single-file binary | `pkg` or `@vercel/ncc` bundle, one per OS/arch | v0.4 |
| Docker | `framerli/framerli:latest` image for CI/CD | v0.2 |
| GitHub Release | Binaries + checksums + SBOM | Every release |
| Claude Code skill | `.claude/skills/framerli/` zip published with each release | v0.4 |
| MCP Registry | Listed in `modelcontextprotocol/servers` registry | v0.2 |

---

## 15. Open Questions & Decisions Required

| # | Question | Owner | Needed by |
|---|---|---|---|
| Q1 | Node-only for v0, or daemon-from-day-one? | Brian | Phase 0 |
| Q2 | `commander` vs `oclif`? | Brian | Phase 0 |
| Q3 | Scope the name: stay `framerli` (CMS-forward branding, but hosts many non-CMS groups) or rebrand to `framerctl` / `fmr`? | Brian | Phase 1 start |
| Q4 | Is there value in a JSON-schema-published site.yaml, or does YAML with doc comments suffice? | Brian | Phase 3 |
| Q5 | Rust-CLI + Node-daemon split: commit to it in v0.3 or stay all-Node through v1.0? | Brian | Phase 3, post-daemon spike |
| Q6 | Telemetry on by default (opt-out) or off by default (opt-in)? | Brian | Phase 5 |
| Q7 | How do we handle Framer's future REST API (if one ships) — swap the transport silently or add a new `--transport rest` flag? | Framer roadmap + Brian | Monitor |
| Q8 | Does `framerli` absorb non-Framer destinations someday (Webflow, Sanity, Contentful) — or stay Framer-scoped? Scope-creep risk. | Brian | v1.0 decision |

---

## 16. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Framer changes the Server API in breaking ways during beta | High | Medium | Pin SDK version; ship patches fast; test matrix covers SDK versions. |
| Framer ships their own CLI | Medium | High | `framerli` is still the agent-native/MCP/sync layer; pivot positioning to "batteries-included wrapper." |
| WebSocket cold start tax turns out to be worse than expected for agent use | Medium | High | Daemon in v0.3 solves it; MCP in v0.2 partly solves it (single connection per host session). |
| Framer starts charging aggressively per-call post-beta | Medium | Medium | `framerli` already has rate-limit controls; surface per-invocation quota tracking. |
| Adoption stalls because devs prefer inline Node scripts | Medium | Medium | Double down on the agent/MCP path where the gap is widest; inline-script devs are not the primary ICP. |
| Dogfood project on Vanguard properties blocked by SEC/FINRA compliance | Medium | Low | Dogfood on personal projects first; Vanguard is a later-stage consumer, not v0. |

---

## Appendix A — Command → SDK method mapping

Partial, representative. Full mapping table maintained in `docs/sdk-mapping.md`.

| CLI command | SDK method |
|---|---|
| `project info` | `getProjectInfo`, `getPublishInfo` |
| `status` | `getChangedPaths` |
| `contributors` | `getChangeContributors` |
| `publish` | `publish` |
| `deploy` | `deploy(deploymentId)` |
| `cms collections list` | `getCollections` + `getManagedCollections` |
| `cms items list` | `collection.getItems` |
| `cms items add` | `collection.addItems` / `managedCollection.addItems` |
| `cms items remove` | `collection.removeItems` |
| `cms fields add` | `collection.addFields` / `managedCollection.setFields` |
| `cms schema apply` | `managedCollection.setFields` |
| `node get` | `getNode` |
| `node set` | `setAttributes` |
| `node find` | `getNodesWithType` + `getNodesWithAttribute` |
| `code pull` | `getCodeFiles`, `codeFile.content` |
| `code push` | `codeFile.setFileContent`, `createCodeFile` |
| `code typecheck` | `codeFile.typecheck` |
| `assets upload` | `uploadImage` / `uploadFile`, `uploadImages` / `uploadFiles` |
| `i18n export` | `getLocales`, `getLocalizationGroups` |
| `i18n import` | `setLocalizationData` |
| `redirects add` | `addRedirects` |
| `custom-code set` | `setCustomCode` |
| `whoami` | `getCurrentUser` |
| `can` | `isAllowedTo` |

---

## Appendix B — Error code taxonomy

Stable codes, documented and never repurposed.

| Code | Exit | Meaning | Retryable |
|---|---|---|---|
| `E_USAGE` | 2 | Bad CLI args | No |
| `E_AUTH_MISSING` | 3 | No credential configured | No |
| `E_AUTH_INVALID` | 3 | Credential rejected by Framer | No |
| `E_PERM_DENIED` | 3 | `isAllowedTo` returned false | No |
| `E_NOT_FOUND` | 4 | Project/collection/item/node not found | No |
| `E_SLUG_COLLISION` | 5 | Slug conflict on item add | No |
| `E_FIELD_TYPE_MISMATCH` | 5 | Field value doesn't match declared type | No |
| `E_COLLECTION_READONLY` | 5 | Attempted write to an unmanaged-by-us collection | No |
| `E_APPROVAL_REQUIRED` | 5 | Blocked by `require_approval_for` | Yes (with approval) |
| `E_RATE_LIMITED` | 6 | Framer or local rate limit hit | Yes (with backoff) |
| `E_COLD_START_TIMEOUT` | 7 | WebSocket connect timed out | Yes |
| `E_NETWORK` | 8 | Transport error below WebSocket layer | Yes |
| `E_FRAMER_INTERNAL` | 10 | Framer returned a 5xx or its equivalent | Yes |

---

## Appendix C — Glossary

- **Managed collection** — a CMS collection whose schema and items are owned by a plugin / API identity. Items and fields can only be edited programmatically by that identity.
- **Unmanaged collection** — a CMS collection edited primarily by humans in the Framer UI. The Server API can read and write items but does not own the schema.
- **Preview deployment** — the result of `publish()`. A versioned URL that is not yet production.
- **Promoted / production deployment** — a preview that has been promoted via `deploy(deploymentId)`.
- **Cold start** — the 1–2s penalty the first time a new WebSocket connects to Framer within a freshness window.
- **Session lease** — a time-bounded API credential issued by a secrets manager, held only in process memory.
- **Blast-radius control** — a profile-level limit on mutations per session/minute (agent safety).
- **Sync adapter** — a first-party module that maps an external source (Notion/Airtable/etc.) into the `cms sync` pipeline.

---
