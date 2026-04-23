# clipli

`clipli` is a macOS clipboard intelligence CLI for agents and power users.

It turns rich clipboard content into something programmable: you can inspect what is on the clipboard, capture formatted content as reusable templates, render those templates with fresh data, convert between formats like RTF and HTML, and generate Excel-friendly clipboard payloads from CSV.

## What It Does

`clipli` is built around a simple loop:

```text
Copy from an app -> capture or inspect -> transform or templatize -> render -> paste back with formatting
```

That makes it useful for workflows like:

- saving a formatted table, slide fragment, or document snippet as a reusable template
- filling the same template with new values and pasting it back into Office or browser apps
- converting clipboard or file content between HTML, Jinja2 templates, RTF, and plain text
- generating Excel-compatible HTML from CSV so it pastes cleanly into Excel
- making targeted edits to clipboard table cells without rebuilding the whole artifact

## Core Capabilities

### Clipboard inspection and I/O

`clipli` can inspect the current clipboard and read or write several rich formats used by macOS apps, including HTML, RTF, plain text, PNG, TIFF, and PDF.

Typical commands:

```bash
clipli inspect
clipli read --type html
clipli write --type html -i snippet.html
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

`clipli` can turn CSV into Excel-compatible HTML and place it on the clipboard, then refine pasted table content by A1-style cell reference.

Typical commands:

```bash
clipli excel data.csv --col "Revenue:currency:right"
clipli excel-edit --set-bg "D4:#A0D771" --set-fg "D4:#628048"
```

## Command Overview

Current top-level commands:

- `inspect` — show the clipboard formats currently available
- `read` — read clipboard content to stdout or a file
- `write` — write content from stdin or a file to the clipboard
- `capture` — save clipboard content as a named template
- `paste` — render a template with data and write it to the clipboard
- `list`, `show`, `edit`, `delete` — manage saved templates
- `versions`, `restore` — inspect and roll back template history
- `lint`, `search` — validate and discover templates
- `export`, `import` — move templates between machines
- `excel`, `excel-edit` — build and tweak Excel-friendly clipboard content
- `render` — render a template to files or stdout without touching the clipboard
- `convert` — convert between supported formats

## Platform Notes

- `clipli` is designed for macOS.
- Clipboard operations require a macOS GUI session.
- Non-clipboard commands like `convert`, `lint`, and parts of `render` are easier to use in automation and CI-like contexts.
- RTF-to-HTML conversion relies on the macOS `textutil` tool.

## Build

Build the binary from this directory:

```bash
cargo build --release
```

Then run it from `target/release/clipli`, or during development with:

```bash
cargo run -- --help
```

## Where to Look Next

- [CLIPLI_SPEC.md](/Users/weisberg/Documents/Development/agent_tools/tools/clipli/CLIPLI_SPEC.md) for the fuller product spec
- [CLIPLI_PLAN.md](/Users/weisberg/Documents/Development/agent_tools/tools/clipli/CLIPLI_PLAN.md) for roadmap and implementation status
- [CLIPLI_SKILL.md](/Users/weisberg/Documents/Development/agent_tools/tools/clipli/CLIPLI_SKILL.md) for agent-facing workflow guidance
