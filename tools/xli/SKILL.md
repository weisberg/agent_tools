---
name: xli
description: |
  Use xli for deterministic, JSON-first Excel workbook operations:
  inspect, read, write, format, batch edits, sheet management, workbook
  creation from CSV/Markdown/JSON, and built-in template application.
  Trigger on: "edit this xlsx", "build a workbook", "format that range",
  "create an Excel report from this CSV", "inspect the spreadsheet",
  "validate the workbook", "atomic commit on .xlsx", or any task where
  an agent needs to manipulate `.xlsx` files without driving Excel.
---

# xli

`xli` is a Rust CLI for deterministic, agent-safe Excel workbook
operations. It is the right tool whenever the artifact is a `.xlsx`
file on disk and the agent needs to *modify* it, not just read it.

The full operator-facing reference is
[`README.md`](./README.md). The platform contract `xli` follows is in
[`docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md).

## When to use xli

- Build an Excel report from prepared CSV / NDJSON / JSON rows.
- Inspect an unknown workbook before deciding what to do with it.
- Update a single cell, formula, range, or sheet inside an existing
  workbook with atomic commit semantics.
- Apply a batch of micro-ops in one transaction.
- Format a range with number formats, fills, fonts, alignment, and
  column widths.
- Validate or lint a workbook before handing it to a stakeholder.
- Apply a built-in template (e.g. `basic-table-format`) to part of a
  workbook.

## When **not** to use xli

- You need to drive a *running* Excel instance — `xli` operates on
  files, not on the live application.
- You need to render a chart artifact (image) — use `vizli`.
- You need pandas-style validation, reconciliation, or report
  generation — use `xli-companion` (Python).
- You need to manipulate decks or Word documents — use `deckli` /
  `docli`.

## Agent Contract

- stdout in non-TTY mode is the JSON envelope; diagnostics go to stderr.
- `--json` forces JSON envelope on stdout.
- Mutating commands write atomically and may emit a warning on the
  `umya-spreadsheet` fallback path. **Always check `meta.warnings`** on
  workbooks containing charts, drawings, macros, data validation, or
  complex tables — fidelity may be reduced for those features.
- Mutating commands accept a fingerprint for compare-and-swap; refuse
  on stale input.
- Address content with `Sheet!A1` (cell), `Sheet!A1:B10` (range), or
  named tables / named ranges where they exist. Avoid sheet *index*
  addressing because sheet renames break it.
- Selector resolution order: named table → named range → A1 selector.

## Recommended Workflows

### 1. Inspect before mutating

```bash
xli inspect workbook.xlsx --json
```

Returns workbook metadata, sheet names, dimensions, and fingerprints.
Use this to confirm shape before writing.

### 2. Build a formatted report from CSV

```bash
xli create /tmp/report.xlsx \
  --from-csv data.csv \
  --title "Revenue Report" \
  --col Account \
  --col Revenue:currency:right \
  --hide InternalNotes \
  --rename Account:Customer \
  --total-row
```

### 3. Atomic single-cell write with formula

```bash
xli write workbook.xlsx "Summary!B2" --formula "=SUM(Data!B:B)"
```

### 4. Batch edits in one commit

```bash
printf '%s\n' \
  '{"op":"write","address":"Summary!A1","value":"Revenue"}' \
  '{"op":"format","range":"Summary!A1:B1","bold":true,"fill":"4472C4","font_color":"FFFFFF"}' \
  | xli batch workbook.xlsx --stdin
```

### 5. Apply a built-in template

```bash
xli template list
xli template preview basic-table-format --param range=Summary!A1:B10
xli apply workbook.xlsx basic-table-format --param range=Summary!A1:B10
```

### 6. Pre-handoff quality gate

```bash
xli lint workbook.xlsx
xli doctor workbook.xlsx --skip-recalc
xli validate workbook.xlsx
```

## Failure Recovery

| Symptom | What to do |
|---|---|
| `umya-spreadsheet fallback` warning on a chart-bearing workbook | Re-inspect; consider whether artifact-sensitive features survived. Fall back to manual review for compliance-critical files. |
| Fingerprint mismatch on write | Re-inspect to refresh the fingerprint, then retry. Another writer touched the file. |
| `recalc` fails | LibreOffice is required; check `xli doctor`. Skip recalc if the workbook does not need formula evaluation. |
| Unknown sheet error | Run `xli inspect` and address by exact sheet name (case-sensitive). |
| Range out of bounds | `xli inspect --range` to confirm the dimension before writing. |

## Companion: `xli-companion` (Python)

Reach for `xli-companion` when the task is heavier than addressable
mutation: dataframe validation, reconciliation against source data,
OOXML artifact audits, optional real-engine checks, or report
rendering. It complements `xli`; it does not replace it.

## Schema discovery

```bash
xli schema                     # full schema
xli schema --command create    # one command
xli schema --result FormatOutput
```

Use this to learn the exact shape of inputs/outputs before composing
batch ops or wiring `xli` into another tool.
