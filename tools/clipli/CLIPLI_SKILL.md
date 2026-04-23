---
name: clipli
description: |
  macOS clipboard intelligence CLI for agents and power users. Use when working with
  the system clipboard, generating formatted Excel tables from CSV, editing clipboard
  content, managing reusable HTML templates, or converting between formats (RTF, HTML,
  plain text, Jinja2). Triggers on: (1) clipboard read/write/inspect operations,
  (2) creating Excel-pasteable tables from data, (3) editing cell formatting in clipboard
  HTML, (4) capturing clipboard content as reusable templates, (5) RTF/HTML/plain text
  conversion, (6) template management (versioning, linting, search, import/export),
  (7) checking local clipboard/config readiness with doctor, (8) agent-command
  templatization and batch rendering.
  Requires macOS. Binary must be built first with cargo build --release.
---

# clipli — Clipboard Intelligence CLI

## Setup

Build from the repo root: `cd tools/clipli && cargo build --release`

Binary in this repo: `tools/clipli/target/release/clipli`

Current binary version: `clipli 0.4.0`

All clipboard operations require a macOS GUI session. Non-clipboard commands (`doctor --skip-clipboard`, `convert`, `lint`, `render`, `excel --dry-run`) work in automation and CI-like contexts.

## Readiness Check

Run `doctor` first when the environment is unknown, when clipboard access fails, or before using `clipli` from automation:

```bash
clipli doctor
clipli doctor --json
clipli doctor --json --skip-clipboard
```

`doctor` checks platform, config parsing, template-store writability, `textutil` availability, pasteboard access, and external agent command launchability when configured.

In sandboxed agent environments, `doctor` may report the template store as not writable if the sandbox blocks `~/Library/Application Support/clipli`. Treat that as environment signal, not necessarily a product failure.

## Agent-first summary

If a new agent is handed `clipli` as a skill, the main thing to understand is that it mixes safe inspection commands with commands that mutate live user state.

- `write`, `capture`, `paste`, `excel`, and `excel-edit` change the current macOS clipboard.
- `capture`, `edit`, `delete`, `restore`, and `import` persist changes under the clipli config store, typically `~/Library/Application Support/clipli/templates/` on macOS.
- `doctor`, `inspect`, `read`, `convert`, `search`, `show`, `lint`, and `render` are the safest first moves.
- `excel --dry-run`, `excel-edit --dry-run`, and `paste --dry-run` let you preview output before touching the clipboard.
- `capture` reads whatever is on the clipboard right now. It does not capture from a file path.
- `paste` renders from stored template data. It does not use the current clipboard as input.
- `render` writes files or stdout only. It does not touch the clipboard.
- `capture --strategy agent --agent-command <cmd>` invokes an external process directly, passes a JSON request on stdin, and validates JSON from stdout.
- If the clipboard format is unknown, start with `inspect`.

## Safe Vs Mutating Commands

| Command family | Side effect |
|---|---|
| `doctor`, `inspect`, `read`, `convert`, `list`, `show`, `search`, `versions`, `lint`, `render` | Safe read/preview operations |
| `excel --dry-run`, `excel-edit --dry-run`, `paste --dry-run` | Safe preview operations |
| `write`, `paste`, `excel`, `excel-edit` | Mutate the live clipboard |
| `capture`, `edit`, `delete`, `restore`, `import` | Change the persistent template store |

## Safe Default Workflow

When the user has not explicitly asked you to overwrite the clipboard, the safest sequence is:

1. Use `inspect` to see what is on the clipboard now.
2. Use `read`, `show`, `search`, or `lint` to understand the existing content or template.
3. Use `convert`, `render`, `excel --dry-run`, `excel-edit --dry-run`, or `paste --dry-run` to preview the exact output.
4. Only then use a clipboard-mutating command.

If the environment itself is uncertain, run `clipli doctor --json --skip-clipboard` before step 1.

## The Core Loop

```
Copy from App  →  clipli capture  →  reusable template  →  clipli paste  →  Paste into App
```

Alternatively, for structured data without capturing:

```
CSV file  →  clipli excel  →  clipboard  →  Cmd+V into Excel
            clipli excel-edit  →  refine formatting  →  Cmd+V
```

## Choosing the Right Command

| I want to... | Use |
|---|---|
| Turn a CSV into a formatted Excel table | `clipli excel data.csv` |
| Tweak colors/values on an existing clipboard table | `clipli excel-edit --set-bg "C3:#A0D771"` |
| Put arbitrary HTML on the clipboard | `clipli write --type html -i file.html` |
| Generate a table from JSON with a Jinja2 template | `clipli paste --from-table` |
| Save clipboard content for reuse later | `clipli capture --name my_template` |
| Fill a saved template with new data | `clipli paste my_template -D '{...}'` |
| See what's on the clipboard | `clipli inspect` |
| Check whether clipli is usable here | `clipli doctor --json` |
| Convert RTF to HTML | `clipli convert --from rtf --to html` |
| Render many outputs without touching the clipboard | `clipli render my_template --data-file rows.json` |
| Let an external LLM tool templatize captured HTML | `clipli capture --name my_template --templatize --strategy agent --agent-command my-agent` |

## 1. CSV to Excel Table

Reads a CSV, generates Excel-native HTML, writes it to the clipboard. User then Cmd+V into Excel.

```bash
# Simplest — default formatting, all columns
clipli excel data.csv

# With column formatting (repeatable flag: NAME:FORMAT[:ALIGN])
clipli excel data.csv \
  --col "Revenue:currency:right" \
  --col "Margin:percent_int:center" \
  --col "SKU:text" \
  --bold "SKU" \
  --header-bg "#007873" \
  --font "Aptos Display"

# Kitchen sink
clipli excel data.csv \
  --title "Q1 2026 Product Report" \
  --col "Revenue:currency:right" \
  --col "Margin:percent_int:center" \
  --col "Last Updated:datetime_iso:right" \
  --bold "SKU" --italic "Category" --wrap "Product Name" \
  --align "Status:center" \
  --fg-color "Status:#C62828" \
  --color-if "Margin:>=:50:#A0D771:#628048" \
  --color-if "Margin:<:30:#C92E25:white" \
  --link "SKU:https://example.com/dp/{}" \
  --total-row --total-formula \
  --rename "Units Sold:Units" \
  --columns "SKU,Product Name,Revenue,Margin,Status" \
  --hide "Internal ID" \
  --row-height 24 --header-height 28

# Preview without copying to clipboard
clipli excel data.csv --col "Revenue:currency" --dry-run > preview.html

# Pipe from stdin
cat data.csv | clipli excel -
```

**`--col` format syntax:** `COLUMN_NAME:FORMAT[:ALIGNMENT]`

| Format | Display | Notes |
|--------|---------|-------|
| `currency` | $4,230,000 | Red negatives |
| `accounting` | $4,230,000 | Dash for zero, parentheses for negatives |
| `percent` | 15.60% | Excel treats input as fractional (0.156 = 15.6%) |
| `percent_int` | 42% | Integer percentage |
| `percent_1dp` | 15.6% | One decimal |
| `integer` | 12,819 | Comma-grouped, no decimals |
| `text` | B0BFBRL47B | Force text — prevents number auto-detection |
| `datetime_iso` | 2026-03-25 14:30 | ISO datetime |
| `standard` | 1234.5678 | Like General with more decimals |

**`--color-if` syntax:** `COLUMN:OPERATOR:VALUE:BG_HEX:FG_HEX`

Operators: `>=`, `<=`, `>`, `<`, `==`, `!=`, `contains`, `empty`, `not_empty`. First matching rule wins.

**`--link` syntax:** `COLUMN:URL_PATTERN` where `{}` is replaced by the cell value.

**Styles:** `--style table` (default — banded rows, blue-gray borders) or `--style plain` (thick outer border, thin gridlines).

Column widths are controlled by Excel on paste (auto-fit) — cannot be set via clipboard HTML.

## 2. Edit Clipboard Cells

Reads the current clipboard HTML, modifies specific cells by A1 reference, writes it back. Use after `clipli excel` to refine formatting without regenerating from CSV.

Cell references: `A1` = row 1 col 1 (header row), `B2` = row 2 col 2 (first data cell), `AA5` works for columns beyond Z.

```bash
# Change a value
clipli excel-edit --set "B3:New Product Name"

# Color a cell (background + text)
clipli excel-edit --set-bg "D4:#A0D771" --set-fg "D4:#628048"

# Format and align
clipli excel-edit --set-format "E2:currency" --set-align "E2:right"

# Bold, italic, wrap
clipli excel-edit --set-bold "A2" --set-italic "C5" --set-wrap "B2"

# Add a formula
clipli excel-edit --set-formula "E7:=SUM(E2:E6)"

# Combine multiple edits (one clipboard read/write cycle)
clipli excel-edit \
  --set-bg "F6:#C92E25" --set-fg "F6:white" \
  --set-bold "D4" --set "A7:Total"

# Preview the modified HTML without writing to clipboard
clipli excel-edit --set-bg "D4:#A0D771" --dry-run
```

Edits are cumulative — run `excel-edit` multiple times to build up formatting.

## 3. Clipboard Inspection and I/O

**Inspect** — see what types and sizes are on the clipboard:

```bash
clipli inspect          # human-readable table
clipli inspect --json   # machine-readable for scripting
```

Output shows UTI type identifiers and byte sizes. Use this to discover what's available before reading.

**Read** — extract a specific type from the clipboard:

```bash
clipli read --type html                     # HTML to stdout
clipli read --type plain                    # plain text to stdout
clipli read --type rtf                      # RTF to stdout
clipli read --type html -o captured.html    # save to file
clipli read --type png -o screenshot.png    # binary types REQUIRE --output
clipli read --type html -c                  # clean Office HTML cruft before output
```

Supported types: `html`, `rtf`, `plain`, `png`, `tiff`, `pdf`. Binary types (`png`, `tiff`, `pdf`) require `--output` (can't print binary to terminal).

**Write** — put content onto the clipboard:

```bash
clipli write --type html -i report.html     # from file
clipli write --type plain -i notes.md       # plain text from file
echo "hello" | clipli write --type plain    # from stdin
clipli write --type png -i image.png        # binary content
```

When writing HTML, `--with-plain` (default: true) auto-generates a plain-text fallback so apps that don't accept HTML still get content.

## 4. Format Conversion

Reads from stdin (or `-i file`), converts, outputs to stdout (or `-o file`). Does not touch the clipboard.

```bash
# RTF to HTML (via macOS textutil)
clipli convert --from rtf --to html -i document.rtf -o document.html
cat document.rtf | clipli convert --from rtf --to html > output.html

# HTML to plain text (strip tags, decode entities, tab-delimit tables)
clipli convert --from html --to plain < page.html

# HTML to Jinja2 template (extract variables from literal values)
clipli convert --from html --to j2 --strategy heuristic < table.html
# Detects: dates, currency ($), percentages, emails, large numbers, quarters

# Jinja2 template to rendered HTML
clipli convert --from j2 --to html -D '{"name":"Alice","revenue":"$5.2M"}' -i template.j2
```

## 5. Capture Clipboard as Template

Copy formatted content in any app (Excel, PowerPoint, Word, browser), then run:

```bash
# Basic — save clipboard HTML as a named template
clipli capture --name quarterly_report

# With variable extraction — literal values become {{ var_name }} placeholders
clipli capture --name quarterly_report --templatize

# Full options
clipli capture --name quarterly_report \
  --templatize \
  --strategy heuristic \
  --description "Q1 earnings table from Excel" \
  --tags finance,quarterly \
  --preview \
  --force
```

`--preview` opens the cleaned or templatized HTML in the browser before saving. `--force` overwrites an existing template and snapshots the previous version first.

**Pipeline:** read clipboard (HTML > RTF > plain text fallback) -> clean Office HTML -> extract variables -> save to the clipli template store.

**Strategies:** `heuristic` (fast regex: dates, currency, %, emails, numbers), `agent` (stdio protocol or direct external command for smarter extraction), `manual` (save as-is, edit by hand later).

**`--raw`** skips HTML cleaning — preserves original Office markup.

**External agent command mode:**

```bash
clipli capture --name slide_snippet \
  --templatize \
  --strategy agent \
  --agent-command my-agent \
  --agent-timeout 60 \
  --json
```

When `--agent-command` is set, `clipli` launches the command without a shell, writes one JSON request to stdin, reads one JSON response from stdout, captures stderr/exit status on failure, and validates the returned template before saving.

Expected agent response:

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

Validation rejects invalid Jinja, invalid variable names, structural mismatches, scripts, iframes, event handlers, and `javascript:` URLs.

## 6. Render and Paste Templates

Fill a saved template with data, write the rendered HTML + plain-text fallback to the clipboard.

```bash
# Inline JSON data
clipli paste quarterly_report -D '{"title":"Q2 Report","revenue":"$5.2M"}'

# Data from a file
clipli paste quarterly_report --data-file quarter_data.json

# Data from stdin
echo '{"title":"Q3"}' | clipli paste quarterly_report --stdin

# Preview in browser without copying to clipboard
clipli paste quarterly_report -D '{"title":"Q2"}' --dry-run
clipli paste quarterly_report -D '{"title":"Q2"}' --open  # opens in browser AND copies

# Plain text control
clipli paste quarterly_report -D '{}' --plain-text none   # HTML only, no plain-text fallback
clipli paste quarterly_report -D '{}' --plain-text auto   # uses config default
```

**Built-in table templates** (no capture needed):

```bash
# Pipe TableInput JSON to a built-in template
echo '{
  "headers": [{"value":"Name","style":{"bold":true}},{"value":"Score","style":{}}],
  "rows": [[{"value":"Alice","style":{}},{"value":"95","style":{}}]],
  "style": {"header_bg":"#007873","header_fg":"#FFFFFF"}
}' | clipli paste --from-table --dry-run

# Choose template: table_default, table_striped, table_excel, slide_default
echo '...' | clipli paste --from-table -t table_excel
```

**Batch render without touching the clipboard:**

```bash
# rows.json may be an array of objects or newline-delimited JSON
clipli render quarterly_report --data-file rows.json --output-dir ./out
clipli render quarterly_report --data-file rows.json --format plain
clipli render quarterly_report --data-file rows.json --json
```

## 7. Template Lifecycle

**Find templates:**

```bash
clipli list                         # all templates, human-readable
clipli list --json                  # JSON array of TemplateMeta
clipli list --tag finance -v        # filter by tag, show variables
clipli search "revenue"             # full-text search across name, description, tags, HTML content
clipli search "quarterly" --tag finance --json
```

**Examine a template:**

```bash
clipli show quarterly_report            # human summary: name, vars, tags, dates
clipli show quarterly_report --html     # raw template HTML to stdout
clipli show quarterly_report --schema   # variable schema as JSON
clipli show quarterly_report --meta     # full metadata as JSON
clipli show quarterly_report --open     # render with defaults, open in browser
```

**Edit manually** (opens `$EDITOR`, auto-snapshots before editing):

```bash
clipli edit quarterly_report                # opens template.html.j2 in $EDITOR
clipli edit quarterly_report --auto-schema  # detect new {{ vars }} and add to schema.json
```

**Version history** (auto-created on force-save, edit, restore, import):

```bash
clipli versions quarterly_report            # list snapshots: ID, change type, timestamp
clipli versions quarterly_report --json
clipli show quarterly_report --version 20260326T120000Z    # view old version
clipli restore quarterly_report --version 20260326T120000Z # revert (snapshots current first)
```

Max 20 versions per template. Snapshots are full copies stored under `<template>/versions/<id>/`.

**Validate before use:**

```bash
clipli lint quarterly_report            # warnings + errors
clipli lint quarterly_report --strict   # treat warnings as errors (exit code 1)
clipli lint quarterly_report --json     # machine-readable report
```

Checks: unbalanced `{{ }}`/`{% %}`, invalid identifiers, duplicate schema vars, template vars not in schema, schema vars not in template.

**Share between machines:**

```bash
clipli export quarterly_report -o report.clipli     # ZIP bundle with manifest
clipli import report.clipli                          # import to store
clipli import report.clipli --force --name new_name  # overwrite, rename
```

**Delete:**

```bash
clipli delete quarterly_report              # prompts for confirmation
clipli delete quarterly_report --force      # skip confirmation
clipli delete quarterly_report --keep-versions  # remove live template, preserve version history
```

## Excel Format Reference

The `clipli excel` command generates Excel-native HTML (Office XML namespaces, `mso-*` properties, `mso-pattern`, `ProgId=Excel.Sheet`). Users never need to think about this — the command handles it.

For hand-crafted Excel HTML via `clipli write --type html`, see [references/excel_format.md](references/excel_format.md).

## Config

Optional config file, typically `~/Library/Application Support/clipli/config.toml` on macOS. CLI flags always override these values:

```toml
[defaults]
font = "Aptos Display"      # default font for clipli excel (--font overrides)
font_size_pt = 12.0          # default size for clipli excel (--font-size overrides)
plain_text_strategy = "tab-delimited"  # paste --plain-text auto uses this

[clean]
keep_classes = false          # read -c and capture HTML cleaning
target_app = "generic"        # excel | powerpoint | google_sheets | generic

[templatize]
default_strategy = "heuristic"  # capture --strategy default when flag omitted

[agent]
command = "my-agent"             # optional external command for strategy=agent
args = []                         # optional command arguments
timeout_secs = 30
```

## Error Handling

Without `--json`: errors go to stderr as plain text, exit code 1.

With `--json`, errors go to stdout as `{"ok":false,"error":"...","code":"STORE_NOT_FOUND"}`. JSON mode is available on the automation-oriented commands, including `inspect`, `capture`, `paste`, `list`, `show`, `delete`, `versions`, `lint`, `search`, `excel`, `excel-edit`, `render`, `convert`, and `doctor`.

Error code prefixes: `PB_` (pasteboard), `STORE_` (template store), `RENDER_` (template engine), `CLEAN_` (HTML cleaner), `TEMPLATIZE_` (variable extraction), `RTF_` (conversion).
