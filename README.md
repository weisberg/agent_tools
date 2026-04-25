# agent_tools

Agent-native command-line tools for turning everyday work into structured,
scriptable, automation-ready workflows.

`agent_tools` is a workshop for CLIs that are designed from the ground up for AI
agents and power users: JSON-first output, stable errors, dry runs, predictable
state, and docs that explain when to use each tool. The goal is simple: give an
agent the same dependable leverage a good Unix toolbox gives a human, but for
modern work across documents, spreadsheets, clipboards, knowledge bases, Jira,
Notion, Framer, PowerPoint, shell execution, and more.

## Why This Repo Exists

Most automation tools were built for humans first and agents second. They print
beautiful prose, hide state in surprising places, and make error recovery a
guessing game. These tools take the opposite stance:

- **Structured by default:** machine-readable output, schemas, and stable exit
  behavior wherever possible.
- **Safe to rehearse:** dry-run plans, local deterministic state, and mock modes
  make risky workflows inspectable before they mutate anything.
- **Composable:** small command surfaces that can be chained, embedded in skills,
  or wrapped by MCP servers.
- **Built for real artifacts:** Excel workbooks, Jira issues, rich clipboard
  payloads, Notion databases, Framer projects, PowerPoint decks, DOCX files,
  visualization templates, and file-backed knowledge bases.

## Featured Tools

### `xli` — Excel Workbook Automation

Rust-native Excel operations for agents that need to inspect, create, read,
write, format, validate, and template real `.xlsx` workbooks.

Highlights:

- JSON-first `inspect`, `read`, `write`, `format`, `sheet`, `batch`, `apply`,
  `create`, `lint`, `recalc`, `validate`, `doctor`, `template`, and `schema`.
- Atomic commits, dry runs, and fingerprint compare-and-swap for safer edits.
- CSV, Markdown table, and JSON workbook creation.
- Report-building ergonomics such as named number formats, column selection,
  renames, hidden columns, titles, alignment, and total rows.

Start here: [`tools/xli/README.md`](tools/xli/README.md)

### `jirali` — Agent-Safe Jira CLI

A Jira CLI designed for autonomous agents first and terminal users second.

Highlights:

- Stable JSON stdout, structured JSON errors, and meaningful exit codes.
- Local deterministic state for tests, rehearsal, and offline planning.
- Live Jira REST and GraphQL escape hatches for Atlassian Cloud/Data Center.
- Atlassian Cloud URL handling that normalizes `/jira/` web UI links and
  documents direct vs. scoped API token REST bases.
- Core issue, comment, JQL, ADF, auth, audit, sprint, link, attachment,
  worklog, hierarchy, release, report, and planning surfaces.

Start here: [`tools/jirali/README.md`](tools/jirali/README.md)

### `vaultli` — File-Based Knowledge Vaults

Turn a directory of Markdown, SQL, templates, runbooks, and other assets into a
searchable, validated knowledge base without introducing a database.

Highlights:

- YAML frontmatter for metadata.
- Sidecar Markdown for non-Markdown assets.
- Derived `INDEX.jsonl` for fast lookup and filtering.
- Validation for broken sources, duplicate IDs, dangling refs, and stale index
  state.

Start here: [`tools/vaultli/README.md`](tools/vaultli/README.md)

### `clipli` — Clipboard Intelligence For macOS

Make the system clipboard programmable. Capture formatted content, convert rich
formats, templatize snippets, render them with fresh data, and generate
Excel-friendly clipboard payloads and table images.

Highlights:

- Inspect, read, and write HTML, RTF, plain text, SVG, PNG, TIFF, and PDF clipboard
  content.
- Capture formatted clipboard content as reusable templates.
- Render templates with JSON data and paste them back with formatting intact.
- Convert between RTF, HTML, plain text, and Jinja2-style templates.
- Generate Excel-style tables from CSV as editable HTML or copied SVG/PNG images,
  then edit clipboard tables by A1 cell reference.

Start here: [`tools/clipli/README.md`](tools/clipli/README.md)

### `framerli` — Framer Project Control

A Rust control-plane CLI for the Framer Server API, with a Node bridge to the
official `framer-api` SDK.

Highlights:

- Rust handles command parsing, JSON envelopes, dry-run plans, approval gates,
  profile config, audit logging, and stable exit behavior.
- The Node bridge owns live Framer API calls.
- Mock bridge mode exercises the full Rust-to-Node path without credentials.
- Config can come from CLI flags, environment variables, local YAML/TOML, or a
  global profile.

Start here: [`tools/framerli/README.md`](tools/framerli/README.md)

### `notionli` — Agent-Safe Notion CLI

A Rust Notion CLI that wraps common Notion operations in predictable, auditable
agent workflows.

Highlights:

- JSON envelopes and structured errors with stable exit codes.
- Integration-token auth via `NOTION_API_KEY`, token commands, or macOS
  Keychain.
- Local profile state, aliases, selected targets, receipts, audit logs, and
  dry-run-by-default writes.
- MVP command groups for search, pages, blocks, databases, data sources, rows,
  comments, users, schemas, and tools.

Start here: [`tools/notionli/README.md`](tools/notionli/README.md)

## The Lab

These projects are specs, prototypes, or focused workspaces that extend the same
agent-native philosophy into more surfaces.

| Tool | What It Wants To Unlock |
|---|---|
| `deckli` | Live PowerPoint control through a CLI-to-Office.js bridge. |
| `docli` | High-level, transaction-safe `.docx` creation, inspection, patching, validation, and rendering. |
| `vizli` | Template-driven visualizations and explainers with sidecar discovery and verifiable rendering. |
| `bashli` | Structured shell execution that replaces raw bash strings with JSON/YAML task specs and structured results. |
| `barli` | A macOS menu bar app that discovers Python actions and hot-reloads menu workflows. |

Useful entry points:

- [`tools/deckli/DECKLI_SPECS.md`](tools/deckli/DECKLI_SPECS.md)
- [`tools/docli/docli-spec.md`](tools/docli/docli-spec.md)
- [`tools/vizli/VIZLI_README.md`](tools/vizli/VIZLI_README.md)
- [`tools/bashli/bashli-spec-final.md`](tools/bashli/bashli-spec-final.md)
- [`tools/barli/README.md`](tools/barli/README.md)

## Common Design Language

Across the repo, tools aim to share the same agent-friendly contract:

```json
{
  "ok": true,
  "result": {},
  "meta": {
    "tool": "example.command",
    "duration_ms": 12,
    "dry_run": false
  }
}
```

When a command fails, the ideal output is just as parseable:

```json
{
  "ok": false,
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "The requested range is invalid.",
    "suggestion": "Check the sheet name and A1 range."
  }
}
```

That contract is what makes these tools useful inside coding agents, skills,
automation scripts, and future MCP bridges.

## Build And Explore

The repository contains a mix of Python and Rust projects. Start with the
specific tool README for the exact build command.

For the Python workspace:

```bash
uv sync
uv run pytest
```

For Rust tools, enter the tool directory and use Cargo:

```bash
cargo test
cargo run -- --help
```

## Documentation

- [`AGENTS.md`](AGENTS.md): instructions for agents working in this repo.
- [`docs/tooli.md`](docs/tooli.md): concise guide for building Python CLIs with
  `tooli`.
- [`docs/skills.md`](docs/skills.md): skill authoring guide and local skills
  inventory.
- [`docs/tool-roadmap.md`](docs/tool-roadmap.md): broader tool inventory and
  legacy planning notes.
- [`docs/RUST_CRATES_FOR_TOOLS.md`](docs/RUST_CRATES_FOR_TOOLS.md): Rust crate
  notes for CLI tools.
- [`docs/excel_format_on_clipboard.md`](docs/excel_format_on_clipboard.md):
  notes on Excel-compatible clipboard HTML.

## Status

This is an active toolshed, not a single polished product. Some tools are
usable CLIs today, some are evolving rapidly, and some are deliberately preserved
as specs until the implementation catches up. That is the point: this repo is
where agent workflows become concrete, tested, and reusable.

If you find a bug, stale command, missing capability, or anything that makes a
tool harder for an agent to use, record it in
[`tooli_feedback.md`](tooli_feedback.md) or the relevant tool docs.
