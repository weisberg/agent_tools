# clipli

`clipli` is a macOS clipboard intelligence CLI for agents and power users.

It turns rich clipboard content into something programmable: you can inspect what is on the clipboard, capture formatted content as reusable templates, render those templates with fresh data, convert between formats like RTF and HTML, keep a privacy-aware clipboard history, and generate Excel-friendly clipboard payloads from CSV.

## What It Does

`clipli` is built around a simple loop:

```text
Copy from an app -> capture or inspect -> transform or templatize -> render -> paste back with formatting
```

That makes it useful for workflows like:

- saving a formatted table, slide fragment, or document snippet as a reusable template
- filling the same template with new values and pasting it back into Office or browser apps
- converting clipboard or file content between HTML, Jinja2 templates, RTF, and plain text
- recording, searching, and restoring prior clipboard payloads without storing likely secrets by default
- generating Excel-style tables from CSV as editable HTML or copied SVG/PNG images
- making targeted edits to clipboard table cells without rebuilding the whole artifact

## Core Capabilities

### Clipboard inspection and I/O

`clipli` can inspect the current clipboard and read or write several rich formats used by macOS apps, including HTML, RTF, plain text, SVG, PNG, TIFF, and PDF.

Typical commands:

```bash
clipli inspect
clipli read --type html
clipli write --type html -i snippet.html
clipli doctor
```

### Clipboard history and watch mode

`clipli` can record clipboard entries into a local history store, search text payloads and metadata, and restore a prior entry to the clipboard. History is privacy-aware by default: text that looks like it contains secrets is recorded as metadata only unless you explicitly choose `--sensitive redact` or `--sensitive allow`.

Typical commands:

```bash
clipli watch --once
clipli watch --max-items 10 --sensitive redact
clipli history list --json
clipli history search revenue
clipli history show <ID> --content
clipli history restore <ID>
clipli history restore <ID> --dry-run -o restored.bin
```

### Template capture and reuse

You can copy formatted content from an app, save it as a named template, optionally templatize literal values into variables, then render that template later with new data.

Typical commands:

```bash
clipli capture --name quarterly_report --templatize
clipli paste quarterly_report -D '{"quarter":"Q2","revenue":"$4.2M"}'
clipli render quarterly_report --data-file rows.json --output-dir ./out
```

### Template lifecycle management

Templates are stored on disk and managed like reusable assets. `clipli` includes version history, restore, linting, search, and import/export support.

Typical commands:

```bash
clipli list
clipli show quarterly_report
clipli versions quarterly_report
clipli restore quarterly_report --version 20260420T130000Z
clipli lint quarterly_report
clipli search revenue
clipli export quarterly_report
clipli import quarterly_report.clipli
```

### Conversion and rendering

`clipli` includes pipeline-friendly format conversion and Jinja2-compatible rendering. It can convert RTF to HTML, HTML to plain text, HTML to Jinja2, and Jinja2 back to rendered HTML.

Typical commands:

```bash
clipli convert --from rtf --to html -i document.rtf
clipli convert --from html --to plain -i captured.html
clipli convert --from html --to j2 --strategy heuristic -i table.html
clipli convert --from j2 --to html -D '{"name":"Alice"}' -i template.j2
```

### Excel-focused workflows

`clipli` can turn CSV into Excel-compatible HTML and place it on the clipboard, copy the same table as SVG or PNG when an image is requested, then refine pasted HTML table content by A1-style cell reference.

Use the default `html` mode when the table should remain editable after pasting into Excel. Use `--copy-as svg` or `--copy-as png` when the user explicitly asks for an image artifact; those modes copy only the requested image format to the clipboard.

Typical commands:

```bash
clipli excel data.csv --col "Revenue:currency:right"
clipli excel data.csv --copy-as svg
clipli excel data.csv --copy-as png
clipli excel data.csv --copy-as svg --dry-run > preview.svg
clipli excel data.csv --copy-as png --dry-run --out-file preview.png
clipli excel-edit --set-bg "D4:#A0D771" --set-fg "D4:#628048"
```

### Shell integration

Generate shell completions directly from the CLI definition:

```bash
clipli completions bash
clipli completions zsh
clipli completions fish
```

## Command Overview

Current top-level commands:

- `inspect` — show the clipboard formats currently available
- `read` — read clipboard content to stdout or a file
- `write` — write content from stdin or a file to the clipboard
- `capture` — save clipboard content as a named template
- `paste` — render a template with data and write it to the clipboard
- `watch` — record current or changing clipboard payloads into history
- `history` — list, search, show, record, or restore clipboard history entries
- `list`, `show`, `edit`, `delete` — manage saved templates
- `versions`, `restore` — inspect and roll back template history
- `lint`, `search` — validate and discover templates
- `export`, `import` — move templates between machines
- `excel`, `excel-edit` — build Excel-style clipboard content as editable HTML or SVG/PNG images, then tweak editable HTML tables
- `render` — render a template to files or stdout without touching the clipboard
- `convert` — convert between supported formats
- `doctor` — check local environment, config, store, pasteboard, and agent readiness
- `completions` — print shell completion scripts

## Automation Notes

Most commands that are useful to agents or scripts support `--json`. Failures in JSON mode use a consistent envelope:

```json
{"ok": false, "error": "message", "code": "ERROR_CODE"}
```

The current v1.0 compatibility target is to keep top-level `ok`, `error`, and `code` stable for JSON failures. Success payloads are command-specific but should remain structured JSON objects or arrays, not prose.

`capture --strategy agent` supports two modes:

- without `--agent-command`, `clipli` writes a JSON templatization request to stdout and reads one JSON response from stdin
- with `--agent-command`, `clipli` launches the command directly, writes the request to its stdin, and reads the response from stdout

The agent response shape is:

```json
{
  "template": "<p>Hello {{ name }}</p>",
  "variables": [
    {
      "name": "name",
      "type": "string",
      "default_value": "Alice",
      "description": "Person name"
    }
  ]
}
```

`clipli` validates agent responses before saving them, including Jinja syntax, variable names, basic HTML structure preservation, and suspicious content such as scripts, event handlers, iframes, and `javascript:` URLs.

## Platform Notes

- `clipli` is designed for macOS.
- Clipboard operations require a macOS GUI session.
- Non-clipboard commands like `convert`, `lint`, and parts of `render` are easier to use in automation and CI-like contexts.
- RTF-to-HTML conversion relies on the macOS `textutil` tool.
- History entries are stored under the clipli config directory in `history/index.jsonl` plus `history/payloads/`.
- Clipboard history can contain sensitive user data. The default `--sensitive skip` policy stores metadata only when common secret markers are detected.

## Build

Build the binary from this directory:

```bash
cargo build --release
```

Then run it from `target/release/clipli`, or during development with:

```bash
cargo run -- --help
```

Check local readiness with:

```bash
cargo run -- doctor
cargo run -- doctor --json --skip-clipboard
```

Run the practical verification set used for current development:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

## Where to Look Next

- [CLIPLI_SPEC.md](/Users/weisberg/Documents/Development/agent_tools/tools/clipli/CLIPLI_SPEC.md) for the fuller product spec
- [CLIPLI_PLAN.md](/Users/weisberg/Documents/Development/agent_tools/tools/clipli/CLIPLI_PLAN.md) for roadmap and implementation status
- [clipli/SKILL.md](/Users/weisberg/Documents/Development/agent_tools/tools/clipli/clipli/SKILL.md) for agent-facing workflow guidance
