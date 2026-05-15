---
name: xli
description: |
  Use xli for deterministic, JSON-first Excel workbook operations:
  inspect, read, write, format, validate, template, batch edits, sheet
  management, and workbook creation from CSV/Markdown/JSON. Trigger on
  `.xlsx` automation, report generation, workbook validation, table import,
  formula edits, or atomic spreadsheet mutations.
---

# xli

`xli` is a Rust CLI for deterministic, agent-safe Excel workbook operations.
Use it when the artifact is a real `.xlsx` file on disk and the agent needs to
modify or inspect it without driving Excel.

The full operator-facing reference is [`README.md`](./README.md). The platform
contract is in
[`docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md).

## When to use xli

- Build an Excel report from prepared CSV, NDJSON, JSON, or Markdown rows.
- Inspect an unknown workbook before deciding what to do.
- Update a single cell, formula, range, table, or sheet.
- Apply a batch of micro-ops in one atomic transaction.
- Format ranges with number formats, fills, fonts, and widths.
- Validate or lint a workbook before handing it to a stakeholder.
- Apply a built-in template such as `basic-table-format`.

## When not to use xli

- Driving a running Excel instance; `xli` operates on files.
- Rendering chart artifacts as images; use `vizli`.
- Heavy dataframe validation or reconciliation; use `xli-companion`.
- Manipulating decks, Word docs, or Markdown; use `deckli`, `docli`, or `mdli`.

## Agent Contract

- stdout in non-TTY mode is the JSON envelope; diagnostics go to stderr.
- Prefer `xli ... --json` or commands that naturally emit JSON.
- Mutating commands write atomically and may support compare-and-swap
  fingerprints; refuse stale writes when possible.
- Treat workbook edits as stateful and potentially destructive.
- Inspect workbook structure before writing and use precise sheet/range names.
- Address content with `Sheet!A1`, `Sheet!A1:B10`, named tables, or named ranges.
- Avoid sheet index addressing because sheet renames break it.
- Always check `warnings` on workbooks containing charts, drawings, macros, data
  validation, or complex tables.
- Use `batch` for multi-step edits so changes apply through one atomic path.
- Supported writes, formatting, sheet rename/hide/unhide/reorder, write-only
  batches, mixed batches, and template application use the artifact-preserving
  OOXML patch path. Sheet add/remove/copy and unsupported formatting fields
  still emit the fallback warning.

## Safe Default Workflow

1. Inspect:

   ```bash
   xli inspect workbook.xlsx --json
   xli schema --command read
   ```

2. Read enough context:

   ```bash
   xli read workbook.xlsx "Summary!A1:D20" --headers --limit 20
   ```

3. Plan or batch edits:

   ```bash
   printf '%s\n' \
     '{"op":"write","address":"Summary!A1","value":"Revenue"}' \
     '{"op":"format","range":"Summary!A1:D1","bold":true}' |
     xli batch workbook.xlsx --stdin
   ```

4. Verify:

   ```bash
   xli lint workbook.xlsx
   xli doctor workbook.xlsx --skip-recalc
   xli validate workbook.xlsx
   ```

## Command Map

| Need | Command |
|---|---|
| Discover sheets, dimensions, names, formulas | `inspect` |
| Read one cell or range | `read FILE "Sheet!A1:B20"` |
| Write values or formulas | `write FILE "Sheet!A1" --value ...` / `--formula ...` |
| Format a range | `format FILE "Sheet!A:A" --number-format currency` |
| Create a workbook | `create FILE --sheets Summary,Data` |
| Import CSV/Markdown/JSON into workbook form | `create --from-csv ...` |
| Apply several edits atomically | `batch FILE --stdin` |
| Discover and apply templates | `template list`, `template preview`, `apply` |
| Check output quality | `lint`, `validate`, `doctor` |
| Discover machine schemas | `schema`, `schema --command create` |

## Common Recipes

Create a workbook:

```bash
xli create /tmp/report.xlsx --sheets Summary,Data
```

Create a formatted CSV-backed report:

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

Write a label and formula:

```bash
xli write /tmp/report.xlsx "Summary!A1" --value '"Revenue"'
xli write /tmp/report.xlsx "Summary!B2" --formula "=SUM(Data!B:B)"
```

Format numeric columns:

```bash
xli format /tmp/report.xlsx "Summary!B:B" --number-format currency
xli format /tmp/report.xlsx "Summary!C:C" --number-format percent_int
```

Apply a built-in template:

```bash
xli template list
xli template preview basic-table-format --param range=Summary!A1:D20
xli apply /tmp/report.xlsx basic-table-format --param range=Summary!A1:D20
```

## Failure Recovery

| Symptom | What to do |
|---|---|
| `umya-spreadsheet fallback` warning on a chart-bearing workbook | Re-inspect; consider whether artifact-sensitive features survived. Fall back to manual review for compliance-critical files. |
| Fingerprint mismatch on write | Re-inspect to refresh the fingerprint, then retry. Another writer touched the file. |
| `recalc` fails | LibreOffice is required; check `xli doctor`. Skip recalc if formula evaluation is not needed. |
| Unknown sheet error | Run `xli inspect` and address by exact sheet name. |
| Range out of bounds | Run `xli inspect --range` to confirm dimensions before writing. |

## Companion: xli-companion

Reach for `xli-companion` when the task is heavier than addressable mutation:
dataframe validation, reconciliation against source data, OOXML artifact audits,
optional real-engine checks, or report rendering. It complements `xli`; it does
not replace it.
