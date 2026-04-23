# XLI — Excel CLI for Agile Agentic Analytics

**Version:** 0.2.0-draft
**Date:** 2026-03-15
**Status:** Final Proposed Specification
**Author:** Brian Weisberg

---

## 1. Problem Statement

### 1.1 The Current State

Today, when an AI agent needs to create or edit an Excel workbook, the workflow looks like this:

1. The agent reads a SKILL.md file containing openpyxl/pandas code patterns
2. It generates a bespoke Python script from scratch every time
3. It runs the script, hopes the output is correct
4. It runs a separate `recalc.py` script for formula evaluation
5. It checks for errors, regenerates the script if something broke, and loops

This means **every Excel operation is a code generation task.** The agent burns output tokens writing Python boilerplate, debugging openpyxl cell references, and reasoning about column-letter-to-number conversions. A simple "add a SUM formula to B10" becomes a 30-line script. A formatting pass becomes 80 lines. A report with charts, conditional formatting, and cross-sheet references becomes hundreds of lines of generated code — each one a fresh opportunity for off-by-one errors, wrong column indices, and broken cell references.

### 1.2 Why a CLI, Not an MCP Server or API

The current trend in agentic tooling is toward CLIs rather than MCP servers or APIs:

- **No lifecycle management.** A CLI is a subprocess call. No server to start, no connection to maintain, no heartbeat to monitor. The agent calls it, gets JSON back, moves on.
- **Universal transport.** Every agent framework — Claude Code, Cursor, Copilot, Open Interpreter — can call a subprocess. Not every agent framework speaks MCP.
- **Composable by default.** `xli read report.xlsx A1:D20 | jq '.rows[] | select(.revenue > 1000000)'` works out of the box. MCP tools can't be piped.
- **Schema-discoverable.** `xli schema` emits a JSON schema of all commands and their arguments. An agent can bootstrap its own understanding of the tool without reading documentation.
- **Stateless.** Each invocation is independent. No session state to corrupt, no connection pool to exhaust, no server crash to recover from.
- **Cacheable.** Identical inputs produce identical outputs. Agents can cache results.

### 1.3 Why Rust — Atomic Workbook Commits

The Python-based approach to Excel editing has a fundamental architectural problem: interpreter startup cost. Python takes 200–500ms just to boot, import openpyxl, and parse arguments — before any work happens. This forces agents to batch operations into monolithic scripts, which means more generated code, more failure modes, and harder debugging when something breaks in the middle.

Rust eliminates this entirely. A statically-linked Rust binary cold-starts in **under 2ms.** That makes a radically different architecture possible: **every edit is a fully atomic workbook commit.** Each `xli` invocation acquires a lock, reads the file, performs exactly one operation, writes to a temp file in the same directory, validates the output, `sync_all()`s, and atomically renames it into place. If anything fails at any step, the original file is untouched.

```bash
xli write report.xlsx "B5" --value 300000       # ~12ms: lock → read → patch → sync → rename
xli write report.xlsx "B6" --value 250000       # ~12ms
xli write report.xlsx "B7" --formula "=SUM(B5:B6)"  # ~12ms
xli format report.xlsx "B5:B7" --number-format '$#,##0'  # ~12ms
```

Four atomic commits, ~48ms total. The equivalent Python workflow — generate a script, boot the interpreter, import openpyxl, execute, save — takes 800ms+ and produces a single-point-of-failure script that the agent has to debug if anything goes wrong.

This atomicity changes the failure model. If `xli write` fails on B7, B5 and B6 are already committed. The agent retries one operation, not an entire script. Error messages point to exactly one cell, not somewhere in a 50-line generated program.

The single static binary also means zero-dependency deployment. No Python, no pip, no virtual environment. Copy the binary, run it.

### 1.4 Relationship to Existing Tools

| Tool | Role | Relationship to XLI |
|------|------|---------------------|
| **sheetcraft** | Declarative report renderer. Takes a Pydantic spec, produces a finished workbook. | XLI is the imperative complement. Sheetcraft renders from specs; XLI edits interactively. An agent uses sheetcraft when it has a complete report definition, XLI when it needs to make surgical edits or build incrementally. |
| **xlsx skill** | Knowledge for Claude Code's computer-use environment. Contains openpyxl patterns and recalc.py. | XLI replaces the need to generate openpyxl scripts. The skill becomes a thin SKILL.md that teaches agents to use `xli` commands instead of writing Python. |
| **mdx** | Section-addressable markdown editor for agents under output token constraints. | XLI borrows mdx's core insight — make the document *addressable* through the CLI — but for cells, ranges, sheets, and named regions instead of markdown sections. |

---

## 2. Core Identity

**XLI is a transactional OOXML compiler** — a single Rust binary that is non-interactive, JSON-first, deterministic, and file-transactional.

It supports **read/inspect/import** for Excel-family formats through a pure-Rust read path, and **write/edit/export** for `.xlsx`, `.xlsm`, `.csv`, and `.tsv` through a native Rust write/patch path.

**Macro policy:** Macro preservation and macro attachment (via existing `vbaProject.bin`) are in scope. Macro authoring — generating VBA from scratch — is out of scope.

---

## 3. Design Principles

### 3.1 Atomic Workbook Commits, Not Batched Scripts

Every mutating `xli` command executes as a single transaction: acquire lock → fingerprint → stage temp file → patch → validate → `sync_all()` → atomic rename. The original file is never touched until the replacement is complete and verified. See Section 6 for the full transaction model.

When an agent needs to perform many operations, it has three paths:

- **Sequential atomic commits** — simple, debuggable, each operation is independently retryable. Each commit is ~12ms. Preferred for ≤20 operations.
- **`xli batch OPS.ndjson`** — stream many micro-ops into one atomic commit. Preferred when the operations are logically related and must succeed or fail together.
- **`xli apply PLAN.yaml`** — spec-driven batch from a knowledge base template. Same atomic commit semantics as `batch`, but the operations are defined declaratively.

### 3.2 JSON-First Output

Every command emits structured JSON by default with rich transaction metadata. No colored terminal output unless explicitly requested with `--human`. The agent is the primary consumer.

### 3.3 Cell-Addressable Operations

Workbooks are addressed using Excel-native notation: `B10`, `Summary!A1:D20`, named ranges, table names. No row/column indices. No zero-indexing confusion.

### 3.4 Formulas Over Hardcodes

XLI enforces the same principle as the Anthropic xlsx skill: Excel formulas are always preferred over computed values. XLI will never replace a derived formula with a computed scalar. The `write` command distinguishes between data and formulas explicitly, and `validate` flags cells where a hardcoded number should probably be a formula.

### 3.5 Token-Efficient Responses

Every command returns the minimum viable JSON. Read operations support `--limit`, `--offset` to avoid dumping 10,000 rows into the context window. Errors include structured `fix` suggestions the agent can act on without reasoning from scratch.

### 3.6 Compare-and-Swap Safety

All mutating commands accept `--expect-fingerprint <sha256>`. If the workbook's fingerprint doesn't match, the command refuses to proceed. This closes the "two agents edited the same model and one silently clobbered the other" trapdoor. `xli inspect` returns the current fingerprint.

---

## 4. Command Surface

### 4.1 Overview

```
xli <command> [file] [arguments] [flags]

INSPECTION & ANALYSIS
  inspect     Workbook structure, metadata, fingerprint, and sheet inventory
  read        Read cell values, ranges, tables, or named ranges
  profile     Statistical profile of sheet data (types, distributions, nulls)
  diff        Compare two workbooks structurally and by value

MUTATION
  write       Write values or formulas to cells/ranges
  format      Apply formatting to cells/ranges
  sheet       Manage sheets (add, remove, rename, copy, reorder)
  chart       Create or modify charts

BATCH & SPECS
  apply       Apply a YAML plan to a workbook (atomic commit)
  batch       Stream ndjson micro-ops into one atomic commit

CREATION & IMPORT
  create      Create a new workbook from scratch or from a spec
  table       Import CSV/TSV into a workbook as a named table

QUALITY
  lint        Check formula correctness, style violations, structural issues
  validate    Post-recalc error scan (formula errors, rule violations)
  recalc      Recalculate all formulas via LibreOffice
  doctor      Run lint + recalc + validate in one pass
  repair      Auto-fix common issues (formula prefixes, number formats, etc.)

OOXML
  ooxml unpack   Extract workbook to directory for inspection
  ooxml pack     Repackage directory into workbook
  ooxml diff     Structural diff of two workbooks at the XML level
  ooxml grep     Search workbook XML contents by pattern

META
  schema      Emit JSON schema for all commands, plans, and result envelopes
  template    List, preview, or validate knowledge base templates

GLOBAL FLAGS (all commands)
  --human                    Human-readable output instead of JSON
  --expect-fingerprint SHA   Refuse if workbook fingerprint doesn't match (mutating commands)
  --dry-run                  Report what would change without writing (mutating commands)
  --atomic=true|false        Atomic commit mode (default: true, mutating commands)
```

### 4.2 Command Reference

#### `xli inspect`

Returns workbook structure without reading all cell data. Uses calamine's fast-path reader for maximum speed on large files. Returns the workbook fingerprint that other commands can use for compare-and-swap.

```bash
xli inspect report.xlsx
```

```json
{
  "status": "ok",
  "command": "inspect",
  "fingerprint": "sha256:a3f8c1d9e7b2...",
  "file": "report.xlsx",
  "size_bytes": 245760,
  "sheets": [
    {
      "name": "Summary",
      "dimensions": "A1:G45",
      "rows": 45,
      "cols": 7,
      "has_charts": false,
      "has_tables": true,
      "tables": ["summary_metrics"],
      "named_ranges": ["assumptions", "kpi_targets"],
      "formula_count": 28,
      "merged_regions": ["A1:G1", "A3:C3"]
    },
    {
      "name": "Raw Data",
      "dimensions": "A1:BL5000",
      "rows": 5000,
      "cols": 64,
      "has_charts": false,
      "has_tables": true,
      "tables": ["email_sends", "conversions"],
      "named_ranges": [],
      "formula_count": 0,
      "merged_regions": []
    }
  ],
  "defined_names": {
    "assumptions": "Summary!$B$5:$B$12",
    "kpi_targets": "Summary!$D$5:$D$12"
  },
  "has_macros": false,
  "styles_summary": {
    "fonts_used": ["Arial", "Calibri"],
    "has_conditional_formatting": true,
    "color_coded_inputs": true
  }
}
```

#### `xli read`

Reads cell values from a workbook. For value-only reads on large files, uses calamine's streaming reader for speed. For reads that also need formula text or style metadata, uses the full OOXML parser.

```bash
# Single cell — returns value and metadata
xli read report.xlsx "Summary!B10"

# Range — returns grid of values
xli read report.xlsx "Summary!A1:D20"

# Pagination for large ranges
xli read report.xlsx "Raw Data!A1:BL5000" --limit 50 --offset 0

# Named range
xli read report.xlsx --named assumptions

# Table by name
xli read report.xlsx --table email_sends --limit 20

# Formulas mode — show formulas instead of computed values
xli read report.xlsx "Summary!A1:D20" --formulas

# Both values and formulas
xli read report.xlsx "Summary!A1:D20" --with-formulas
```

**Single cell response:**
```json
{
  "status": "ok",
  "address": "Summary!B10",
  "value": 1250000,
  "formula": "=SUM(B5:B9)",
  "type": "number",
  "format": "$#,##0",
  "font": {"bold": false, "color": "000000"},
  "fill": null
}
```

**Range response:**
```json
{
  "status": "ok",
  "range": "Summary!A1:D5",
  "headers": ["Metric", "Q1", "Q2", "Q3"],
  "rows": [
    {"Metric": "Revenue", "Q1": 1200000, "Q2": 1350000, "Q3": 1500000},
    {"Metric": "Spend", "Q1": 400000, "Q2": 420000, "Q3": 450000},
    {"Metric": "ROI", "Q1": 3.0, "Q2": 3.21, "Q3": 3.33},
    {"Metric": "Conversion Rate", "Q1": 0.042, "Q2": 0.045, "Q3": 0.048}
  ],
  "total_rows": 4,
  "truncated": false
}
```

#### `xli write`

Writes values or formulas to cells. Each invocation is an atomic workbook commit. Distinguishes between data writes and formula writes explicitly.

```bash
# Single cell value
xli write report.xlsx "Summary!B10" --value 1250000

# Single cell formula
xli write report.xlsx "Summary!B10" --formula "=SUM(B5:B9)"

# With compare-and-swap — refuse if file changed since inspect
xli write report.xlsx "B10" --formula "=SUM(B5:B9)" --expect-fingerprint "sha256:a3f8c1d9e7b2..."

# Bulk write from JSON stdin — single atomic commit for the batch
echo '{"cells": [
  {"address": "B5", "value": 300000},
  {"address": "B6", "value": 250000},
  {"address": "B7", "formula": "=SUM(B5:B6)"}
]}' | xli write report.xlsx --sheet Summary --stdin

# Write from CSV
xli write report.xlsx --sheet "Raw Data" --from data.csv --start A1
```

**Response:**
```json
{
  "status": "ok",
  "command": "write",
  "commit_mode": "atomic",
  "fingerprint_before": "sha256:a3f8c1d9e7b2...",
  "fingerprint_after": "sha256:f1e2d3c4b5a6...",
  "written": 3,
  "cells": ["Summary!B5", "Summary!B6", "Summary!B7"],
  "formulas_written": 1,
  "needs_recalc": true,
  "warnings": []
}
```

#### `xli format`

Applies formatting to cells or ranges. Each invocation is an atomic commit.

```bash
# Bold a header row
xli format report.xlsx "Summary!A1:G1" --bold --font-size 12 --fill "4472C4" --font-color "FFFFFF"

# Number format
xli format report.xlsx "Summary!B5:D12" --number-format '$#,##0;($#,##0);"-"'

# Percentage format
xli format report.xlsx "Summary!B15:D15" --number-format '0.0%'

# Column width
xli format report.xlsx --sheet Summary --col-width "A:25,B:15,C:15,D:15"

# Conditional formatting
xli format report.xlsx "Summary!B5:D12" \
  --conditional "value>1000000:fill=C6EFCE,font_color=006100" \
  --conditional "value<0:fill=FFC7CE,font_color=9C0006"

# Financial model color coding (input cells = blue text)
xli format report.xlsx "Summary!B5:B12" --preset input-cell

# Apply a format spec from the knowledge base
xli format report.xlsx --spec templates/financial-formatting.yaml
```

#### `xli sheet`

Manages sheets within a workbook.

```bash
xli sheet report.xlsx list
xli sheet report.xlsx add "Sensitivity Analysis" --after "Summary"
xli sheet report.xlsx rename "Sheet1" "Executive Summary"
xli sheet report.xlsx copy "Summary" "Summary (Backup)"
xli sheet report.xlsx delete "Sheet3"
xli sheet report.xlsx reorder "Executive Summary,Data,Analysis,Charts"
xli sheet report.xlsx hide "Raw Data"
xli sheet report.xlsx unhide "Raw Data"
```

#### `xli chart`

Creates or modifies charts. Charts are defined via a YAML spec because chart configuration is inherently complex and multi-parameter.

```bash
# Create chart from spec
xli chart report.xlsx --spec templates/bar-chart.yaml --sheet Summary --anchor "F2"

# Create chart inline (simple cases)
xli chart report.xlsx create \
  --type bar \
  --data "Summary!A4:D8" \
  --title "Quarterly Revenue by Segment" \
  --sheet Summary \
  --anchor "F2" \
  --size "720x480"

# List charts in workbook
xli chart report.xlsx list

# Remove a chart
xli chart report.xlsx remove --sheet Summary --index 0
```

#### `xli batch`

Streams micro-ops from ndjson into one atomic commit. This is the bridge between "20 sequential atomic commits" (fine at ~240ms) and "80 related edits that must succeed or fail together" (needs batch semantics).

```bash
# From file
xli batch report.xlsx --ops edits.ndjson

# From stdin — agent streams ops as it computes them
cat <<'EOF' | xli batch report.xlsx --stdin
{"op": "write", "address": "Summary!B5", "value": 300000}
{"op": "write", "address": "Summary!B6", "value": 250000}
{"op": "write", "address": "Summary!B7", "formula": "=SUM(B5:B6)"}
{"op": "format", "range": "Summary!B5:B7", "number_format": "$#,##0"}
{"op": "format", "range": "Summary!A1:G1", "bold": true, "fill": "4472C4", "font_color": "FFFFFF"}
EOF

# Dry-run
xli batch report.xlsx --ops edits.ndjson --dry-run
```

**Response:**
```json
{
  "status": "ok",
  "command": "batch",
  "commit_mode": "atomic",
  "fingerprint_before": "sha256:a3f8c1d9...",
  "fingerprint_after": "sha256:f1e2d3c4...",
  "ops_executed": 5,
  "ops_failed": 0,
  "stats": {
    "cells_written": 3,
    "cells_formatted": 10,
    "formulas_written": 1
  },
  "needs_recalc": true,
  "warnings": []
}
```

#### `xli apply`

The power command. Applies a YAML plan from the knowledge base as a single atomic commit. Uses Tera (Jinja2-compatible) for parameter interpolation.

```bash
xli apply report.xlsx --spec templates/cuped-results-table.yaml \
  --vars '{"metric": "conversion_rate", "pre_period": "2025-Q3"}'

xli apply report.xlsx --spec templates/quarterly-report-format.yaml --dry-run
```

See Section 8 for the full plan spec schema and a worked CUPED example.

#### `xli create`

Creates a new workbook. Uses `rust_xlsxwriter` for maximum fidelity with the Excel file format.

```bash
# Empty workbook
xli create report.xlsx

# From template
xli create report.xlsx --template templates/experiment-report.yaml

# From CSV (import + auto-format)
xli create report.xlsx --from data.csv --auto-format

# From multiple CSVs (one sheet each)
xli create report.xlsx --from "sends.csv:Email Sends" --from "conversions.csv:Conversions"
```

#### `xli lint`

Pre-recalc structural and formula correctness check. This is a fast, Rust-native scan — no LibreOffice required.

```bash
xli lint report.xlsx
xli lint report.xlsx --rules templates/financial-model-rules.yaml
xli lint report.xlsx --sheet Summary
```

**What lint checks:**
- Missing `_xlfn.` prefixes on modern Excel functions
- Missing `_xlpm.` parameter prefixes on LAMBDA formulas
- Defined-name formula correctness
- Spill/dynamic-array misuse
- Locale separator issues (semicolons vs. commas)
- Cross-sheet reference validity
- Table/autofilter range conflicts
- Hardcoded numbers adjacent to formula cells
- Input cells not using blue font (financial model convention)
- Formula consistency within rows/columns
- Blank headers, type inconsistencies in columns
- Year values formatted as numbers instead of text
- Negative numbers not using parentheses

**Response:**
```json
{
  "status": "issues_found",
  "command": "lint",
  "issues": [
    {
      "id": "missing-xlfn-prefix",
      "address": "Summary!C10",
      "formula": "=CONCAT(A10, B10)",
      "severity": "error",
      "message": "CONCAT requires _xlfn. prefix for compatibility.",
      "fix": "=_xlfn.CONCAT(A10, B10)",
      "auto_repairable": true
    },
    {
      "id": "hardcoded-in-formula-zone",
      "address": "Summary!B8",
      "value": 0.05,
      "severity": "warning",
      "message": "Hardcoded number (0.05) in a region where adjacent cells use formulas. Consider referencing an assumption cell.",
      "auto_repairable": false
    }
  ],
  "summary": {
    "errors": 1,
    "warnings": 1,
    "info": 0,
    "auto_repairable": 1
  }
}
```

#### `xli repair`

Auto-fixes issues flagged by `lint` that have deterministic repairs.

```bash
xli repair report.xlsx                      # Fix all auto-repairable issues
xli repair report.xlsx --only missing-xlfn-prefix  # Fix only specific issue type
xli repair report.xlsx --dry-run            # Preview fixes without applying
```

#### `xli recalc`

Invokes LibreOffice as a subprocess to evaluate all formulas and write computed values back into the file. Returns structured JSON with error locations.

```bash
xli recalc report.xlsx
xli recalc report.xlsx --timeout 60
```

**Note:** This is the one command where runtime cost is dominated by LibreOffice, not XLI. Typical execution: 1–5 seconds. XLI handles macro setup, subprocess invocation, and post-recalc error scanning internally.

#### `xli validate`

Post-recalc error scan. Checks for formula evaluation errors that only appear after recalculation, and optionally applies validation rules from the knowledge base.

```bash
xli validate report.xlsx
xli validate report.xlsx --rules templates/financial-model-rules.yaml
```

**Response:**
```json
{
  "status": "issues_found",
  "command": "validate",
  "formula_errors": {
    "total": 2,
    "details": [
      {
        "address": "Summary!D15",
        "error": "#DIV/0!",
        "formula": "=C15/C14",
        "fix": "Wrap in IFERROR or check that C14 is non-zero: =IFERROR(C15/C14, 0)"
      },
      {
        "address": "Summary!B20",
        "error": "#REF!",
        "formula": "=SUM(#REF!)",
        "fix": "Cell reference is broken — likely a deleted row or column. Reconstruct the range."
      }
    ]
  },
  "rule_violations": [],
  "summary": {
    "total_formulas": 42,
    "errors": 2,
    "warnings": 0,
    "info": 0
  }
}
```

#### `xli doctor`

Runs `lint` → `recalc` → `validate` in one pass. The full quality pipeline as a single command.

```bash
xli doctor report.xlsx
xli doctor report.xlsx --rules templates/financial-model-rules.yaml --auto-repair
```

#### `xli diff`

Compares two workbooks. Uses calamine for fast parallel reads. Supports both value-level and structural XML-level comparison.

```bash
xli diff report_v1.xlsx report_v2.xlsx
xli diff report_v1.xlsx report_v2.xlsx --sheet Summary --values-only
```

**Response:**
```json
{
  "status": "ok",
  "sheets_added": [],
  "sheets_removed": [],
  "sheets_modified": ["Summary"],
  "cell_changes": [
    {
      "address": "Summary!B10",
      "old": {"value": 1200000, "formula": "=SUM(B5:B9)"},
      "new": {"value": 1250000, "formula": "=SUM(B5:B9)"}
    }
  ],
  "formatting_changes": 3,
  "total_changes": 4
}
```

#### `xli profile`

Statistical profile of sheet data. Gives the agent a quick understanding of data shape, types, and quality without reading every row.

```bash
xli profile report.xlsx --sheet "Raw Data"
xli profile report.xlsx --table email_sends
```

**Response:**
```json
{
  "status": "ok",
  "sheet": "Raw Data",
  "rows": 5000,
  "cols": 64,
  "columns": [
    {
      "header": "household_id",
      "col": "A",
      "type": "string",
      "unique": 4823,
      "nulls": 0,
      "sample": ["HH-001234", "HH-005678", "HH-009012"]
    },
    {
      "header": "revenue",
      "col": "B",
      "type": "number",
      "min": 0,
      "max": 12500000,
      "mean": 487320.5,
      "median": 325000,
      "nulls": 12,
      "zeros": 45
    }
  ]
}
```

#### `xli ooxml`

Direct OOXML inspection and manipulation. Essential for debugging when something goes wrong with a workbook.

```bash
# Extract workbook to directory
xli ooxml unpack report.xlsx ./unpacked/

# Repackage directory into workbook
xli ooxml pack ./unpacked/ report_rebuilt.xlsx

# Structural diff at the XML level
xli ooxml diff report_v1.xlsx report_v2.xlsx

# Search workbook XML contents
xli ooxml grep report.xlsx "SUM("
xli ooxml grep report.xlsx --part "xl/worksheets/sheet1.xml" "conditional"
```

#### `xli schema`

Emits JSON schema for all commands, plan specs, and result envelopes.

```bash
xli schema                  # Full schema for all commands
xli schema write            # Schema for just the write command
xli schema plan             # Schema for YAML plan files accepted by `apply`
xli schema result           # Schema for result envelopes returned by all commands
xli schema batch-op         # Schema for ndjson ops accepted by `batch`
xli schema --openapi        # OpenAPI-compatible schema (for MCP bridge)
```

#### `xli template`

Manages knowledge base templates.

```bash
xli template list
xli template preview financial-summary
xli template apply financial-summary --to report.xlsx
xli template validate my-custom-template.yaml
```

---

## 5. Response Envelope

Every command returns a structured JSON envelope with transaction metadata. This gives the agent rich signal for its next decision without follow-up calls.

### 5.1 Standard Envelope Fields

```json
{
  "status": "ok | error | issues_found",
  "command": "write",
  "input": {
    "file": "report.xlsx",
    "address": "Summary!B10",
    "value": 1250000
  },
  "output": {
    "written": 1,
    "cells": ["Summary!B10"]
  },
  "commit_mode": "atomic",
  "fingerprint_before": "sha256:a3f8c1d9e7b2...",
  "fingerprint_after": "sha256:f1e2d3c4b5a6...",
  "needs_recalc": true,
  "stats": {
    "elapsed_ms": 12,
    "file_size_before": 245760,
    "file_size_after": 245824
  },
  "warnings": [],
  "errors": [],
  "suggested_repairs": []
}
```

### 5.2 Error Envelope

```json
{
  "status": "error",
  "command": "write",
  "code": "CELL_REF_OUT_OF_BOUNDS",
  "message": "Cell address Z100 is outside the sheet dimensions (A1:G45)",
  "context": {
    "sheet": "Summary",
    "requested": "Z100",
    "dimensions": "A1:G45"
  },
  "fix": {
    "action": "retry_with_modified_input",
    "suggestion": "The last column is G (column 7). Use a column between A and G.",
    "valid_range": "A1:G45"
  },
  "fingerprint_before": "sha256:a3f8c1d9e7b2...",
  "fingerprint_after": null
}
```

### 5.3 Error Codes

| Code | Meaning |
|------|---------|
| `FILE_NOT_FOUND` | Workbook file doesn't exist at given path |
| `SHEET_NOT_FOUND` | Named sheet doesn't exist in workbook |
| `CELL_REF_OUT_OF_BOUNDS` | Address is outside sheet dimensions |
| `INVALID_CELL_ADDRESS` | Malformed cell reference (e.g., "ZZZ0") |
| `FORMULA_PARSE_ERROR` | Formula string is malformed |
| `FINGERPRINT_MISMATCH` | `--expect-fingerprint` didn't match; file was modified externally |
| `TEMPLATE_NOT_FOUND` | Referenced template doesn't exist |
| `TEMPLATE_PARAM_MISSING` | Required template parameter not provided |
| `TEMPLATE_PARAM_INVALID` | Parameter value doesn't match schema |
| `RECALC_TIMEOUT` | LibreOffice recalculation timed out |
| `RECALC_FAILED` | LibreOffice recalculation failed |
| `WRITE_CONFLICT` | Cell is in a merged region or protected sheet |
| `SPEC_VALIDATION_ERROR` | YAML spec file has structural errors |
| `BATCH_PARTIAL_FAILURE` | Some ops in a batch failed; no commit (atomic mode) |
| `OOXML_CORRUPT` | Workbook ZIP or XML structure is invalid |

---

## 6. Transaction Model

This is the architectural core. Every mutating command (`write`, `format`, `sheet`, `chart`, `apply`, `batch`, `repair`) follows this sequence:

```
1. ACQUIRE LOCK
   - Exclusive file lock on target workbook via `fs4` crate
   - If lock fails: return LOCK_CONFLICT error with retry guidance

2. FINGERPRINT
   - Compute SHA-256 of the target file
   - If --expect-fingerprint was provided and doesn't match:
     return FINGERPRINT_MISMATCH error with current fingerprint
   - Store as fingerprint_before in response envelope

3. STAGE TEMP FILE
   - Create temp file in the SAME directory as the target
   - Same directory ensures atomic rename is possible (same mount point)
   - Use `tempfile::NamedTempFile` for automatic cleanup on failure

4. PATCH
   - Read source workbook
   - Apply the requested mutation(s)
   - Write result to the temp file
   - Stream-copy unchanged OOXML parts; rewrite only touched parts

5. PRE-COMMIT VALIDATION
   - Run fast structural checks on the temp file:
     • OOXML package structure is valid
     • No orphaned relationships
     • Cell references in written formulas are within sheet dimensions
     • Conditional formatting ranges are valid
   - If validation fails: delete temp file, return error

6. SYNC
   - `sync_all()` the temp file to ensure bytes are on disk
   - This catches I/O errors that a plain close can hide

7. ATOMIC RENAME
   - `std::fs::rename()` the temp file over the target
   - On the same mount point, this is atomic on Linux and macOS
   - On Windows, use `ReplaceFile` for similar guarantees

8. RELEASE LOCK
   - Release the exclusive file lock

9. RESPOND
   - Compute fingerprint_after from the new file
   - Return full response envelope with both fingerprints
```

If any step fails, earlier steps are unwound: the temp file is cleaned up, the lock is released, and the original file remains untouched.

---

## 7. Implementation Architecture

### 7.1 Cargo Workspace

XLI is organized as a Cargo workspace with focused crates for compilation speed, test isolation, and potential reuse.

```
xli/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── xli-cli/            # Command parsing, output formatting, entry point
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/
│   │       │   ├── inspect.rs
│   │       │   ├── read.rs
│   │       │   ├── write.rs
│   │       │   ├── format.rs
│   │       │   ├── sheet.rs
│   │       │   ├── chart.rs
│   │       │   ├── batch.rs
│   │       │   ├── apply.rs
│   │       │   ├── create.rs
│   │       │   ├── lint.rs
│   │       │   ├── validate.rs
│   │       │   ├── recalc.rs
│   │       │   ├── doctor.rs
│   │       │   ├── repair.rs
│   │       │   ├── diff.rs
│   │       │   ├── profile.rs
│   │       │   ├── ooxml.rs
│   │       │   ├── schema.rs
│   │       │   └── template.rs
│   │       └── output.rs   # JSON/human output formatting
│   │
│   ├── xli-core/           # Plans, ops, result envelopes, shared types
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── addressing.rs   # Cell address parsing (A1 ↔ row/col)
│   │       ├── plan.rs         # YAML plan schema (serde + Tera)
│   │       ├── ops.rs          # Batch op definitions
│   │       ├── envelope.rs     # Response envelope types
│   │       ├── style.rs        # Style/format specifications
│   │       └── error.rs        # Structured error types with fix suggestions
│   │
│   ├── xli-fs/             # File transactions: locks, fingerprints, atomic commit
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lock.rs         # Exclusive file locking (fs4)
│   │       ├── fingerprint.rs  # SHA-256 workbook fingerprinting
│   │       ├── staging.rs      # Temp file creation in same directory
│   │       └── commit.rs       # sync_all + atomic rename
│   │
│   ├── xli-read/           # Read-only workbook inspection/import
│   │   ├── Cargo.toml      # Depends on: calamine
│   │   └── src/
│   │       ├── inspect.rs
│   │       ├── read.rs
│   │       ├── profile.rs
│   │       └── diff.rs
│   │
│   ├── xli-ooxml/          # OOXML package patching, validation, diffing
│   │   ├── Cargo.toml      # Depends on: zip, quick-xml
│   │   └── src/
│   │       ├── package.rs      # ZIP archive read/write
│   │       ├── patch.rs        # Streaming XML transform engine
│   │       ├── shared_strings.rs
│   │       ├── styles.rs
│   │       ├── sheet_data.rs   # Cell value/formula patching
│   │       ├── relationships.rs
│   │       ├── defined_names.rs
│   │       ├── tables.rs
│   │       ├── validation_rules.rs
│   │       ├── comments.rs
│   │       ├── charts.rs
│   │       ├── conditional.rs
│   │       ├── macros.rs       # VBA part pass-through (opaque bytes)
│   │       ├── unpack.rs
│   │       ├── pack.rs
│   │       └── grep.rs
│   │
│   ├── xli-new/            # New workbook generation
│   │   ├── Cargo.toml      # Depends on: rust_xlsxwriter
│   │   └── src/
│   │       └── create.rs
│   │
│   ├── xli-calc/           # Recalculation backends
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── libreoffice.rs  # v1 authoritative backend
│   │       └── preflight.rs    # Fast Rust-native checks (future)
│   │
│   ├── xli-kb/             # Knowledge base: templates, rules, examples
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── loader.rs       # Template discovery and loading
│   │       ├── catalog.rs      # Catalog index
│   │       └── validate.rs     # Template spec validation
│   │
│   └── xli-schema/         # Typed plan/result schemas, JSON Schema generation
│       ├── Cargo.toml      # Depends on: schemars
│       └── src/
│           ├── plan_schema.rs
│           ├── result_schema.rs
│           └── batch_op_schema.rs
│
└── knowledge/              # Knowledge base content (data, not code)
    └── xli/
        ├── catalog.yaml
        ├── templates/
        ├── rules/
        ├── examples/
        └── references/
```

### 7.2 Engine Routing

XLI uses the best tool for each job:

| Operation | Engine | Crate | Rationale |
|-----------|--------|-------|-----------|
| Read values, inspect, profile, diff | Fast reader | `calamine` | 5.5M+ downloads, ~2.5x faster than alternatives, streaming support, mature |
| Mutate existing workbook | OOXML patch | `zip` + `quick-xml` | Stream-copy unchanged parts, rewrite only touched XML. No roundtrip corruption risk. |
| Create new workbook | Writer | `rust_xlsxwriter` | Maximum Excel fidelity, charts, conditional formatting, tables, sparklines, macros |
| Recalculate formulas | LibreOffice | subprocess | Only option for authoritative formula evaluation |
| Phase 1 mutation fallback | Roundtrip | `umya-spreadsheet` | Temporary fallback for operations the OOXML patch engine doesn't cover yet |

**The OOXML patch engine is the target architecture.** It opens the ZIP, streams unchanged entries through, rewrites only the affected XML parts using `quick-xml`'s high-performance pull parser/writer, and writes a new ZIP. This avoids loading the entire workbook into a DOM, avoids the roundtrip corruption risks of general-purpose read-write libraries, and is naturally fast for surgical edits.

**Phase 1 reality:** The OOXML patch engine's initial coverage will be: cell values/formulas, shared strings, styles/number formats, defined names, sheet add/rename/delete, workbook metadata, and macro part pass-through. For operations outside that coverage (charts, conditional formatting, data validation, comments), Phase 1 falls back to `umya-spreadsheet` with explicit warnings in the response. As the patch engine gains coverage in Phase 2 and 3, operations graduate off umya one by one until it can be removed.

### 7.3 Key Implementation Details

**Address Parser.** A dedicated `addressing.rs` in `xli-core` that converts between Excel notation and numeric (row, col) tuples. Single source of truth — no other module does column-letter math.

```rust
pub struct CellRef {
    pub sheet: Option<String>,
    pub col: String,      // "B"
    pub row: u32,         // 10
    pub col_idx: u32,     // 2
}

pub fn parse_address(ref_str: &str) -> Result<CellRef, AddressError>;
pub fn parse_range(ref_str: &str) -> Result<RangeRef, AddressError>;
pub fn col_to_letter(idx: u32) -> String;    // 1 → "A", 27 → "AA", 64 → "BL"
pub fn letter_to_col(letter: &str) -> u32;   // "A" → 1, "AA" → 27, "BL" → 64
```

**Atomic Commit.** Every mutation follows the pattern from Section 6:

```rust
pub fn atomic_commit<F>(
    path: &Path,
    expect_fingerprint: Option<&str>,
    mutate: F,
) -> Result<CommitResult>
where
    F: FnOnce(&mut WorkbookPatcher) -> Result<MutationOutput>,
{
    let lock = acquire_exclusive_lock(path)?;
    let fp_before = fingerprint(path)?;
    if let Some(expected) = expect_fingerprint {
        if fp_before != expected {
            return Err(Error::FingerprintMismatch { expected, actual: fp_before });
        }
    }
    let temp = stage_temp_file(path)?;
    let mut patcher = WorkbookPatcher::open(path, &temp)?;
    let output = mutate(&mut patcher)?;
    patcher.finalize()?;
    temp.sync_all()?;
    pre_commit_validate(&temp)?;
    atomic_rename(&temp, path)?;
    drop(lock);
    let fp_after = fingerprint(path)?;
    Ok(CommitResult { fp_before, fp_after, output })
}
```

**OOXML Patch Engine.** The patch engine does not fully unpack a workbook to disk just to change three cells. It opens the ZIP, rewrites only the affected parts, and streams unchanged entries through:

```rust
pub struct WorkbookPatcher {
    reader: ZipArchive<File>,
    writer: ZipWriter<File>,
    touched_parts: HashSet<String>,
}

impl WorkbookPatcher {
    /// Patch a specific XML part using quick-xml streaming transform
    pub fn patch_part<F>(&mut self, part_name: &str, transform: F) -> Result<()>
    where F: FnOnce(&mut XmlPatcher) -> Result<()>;

    /// Pass through an untouched part (zero-copy for unchanged entries)
    fn passthrough_part(&mut self, part_name: &str) -> Result<()>;

    /// Finalize: stream all untouched parts, close ZIP
    pub fn finalize(mut self) -> Result<()>;
}
```

**OOXML Unpack Safety.** `xli ooxml unpack` uses the `zip` crate's `enclosed_name()` method to ensure extracted paths cannot escape the destination directory.

### 7.4 Dependencies

```toml
# Workspace Cargo.toml
[workspace]
members = ["crates/*"]

[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Excel engines
calamine = { version = "0.26", features = ["dates"] }
rust_xlsxwriter = "0.79"
umya-spreadsheet = "2.3"          # Phase 1 fallback only

# OOXML patch engine
zip = "2"
quick-xml = { version = "0.36", features = ["serialize"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Schema generation
schemars = "0.8"

# Template engine (Jinja2-compatible)
tera = "1"

# Filesystem
fs4 = "0.10"                      # File locking
tempfile = "3"                    # Safe temp file creation
sha2 = "0.10"                    # Fingerprinting

# Diagnostics
tracing = "0.1"
tracing-subscriber = "0.3"

# Error handling
anyhow = "1"
thiserror = "2"

[profile.release]
lto = true
strip = true
codegen-units = 1
```

### 7.5 Build and Installation

```bash
# Build
cargo build --release
# Binary: target/release/xli (~10–15 MB statically linked)

# Install from source
cargo install --path crates/xli-cli

# Cross-compile for Linux (from macOS)
cargo build --release --target x86_64-unknown-linux-gnu

# Verify
xli schema | head -20
```

---

## 8. Knowledge Base

### 8.1 Layout

The knowledge base uses a progressive-disclosure pattern: lightweight catalog first, specific templates/rules/examples on demand. This keeps context small for agents and makes the KB auditable and versionable.

```
knowledge/xli/
├── catalog.yaml                   # Index of all templates, rules, and examples
├── templates/
│   ├── formatting/
│   │   ├── financial-model.yaml       # Color coding, number formats, font conventions
│   │   ├── executive-summary.yaml     # Header styling, spacing
│   │   └── data-table.yaml            # Auto-width, alternating rows, frozen headers
│   ├── reports/
│   │   ├── experiment-report.yaml     # Full experiment results workbook scaffold
│   │   ├── cuped-results-table.yaml   # CUPED treatment/control comparison
│   │   ├── quarterly-metrics.yaml     # Standard quarterly KPI dashboard
│   │   └── campaign-performance.yaml  # Email campaign analysis layout
│   └── charts/
│       ├── bar-chart.yaml
│       ├── line-trend.yaml
│       └── waterfall.yaml
├── rules/
│   ├── financial-model-rules.yaml     # Blue inputs, formula consistency, etc.
│   ├── data-quality-rules.yaml        # No blanks in key columns, type checks
│   └── vanguard-standards.yaml        # Org-specific standards
├── examples/
│   ├── cuped-analysis-workflow.yaml   # End-to-end CUPED workflow example
│   └── campaign-report-workflow.yaml  # Campaign report generation example
├── references/
│   ├── excel-formula-prefixes.yaml    # _xlfn./_xlpm. prefix reference
│   ├── number-format-patterns.yaml    # Common number format strings
│   └── color-standards.yaml           # Financial model color conventions
└── schemas/
    ├── plan.schema.json               # JSON Schema for plan files
    ├── batch-op.schema.json           # JSON Schema for batch ops
    └── rules.schema.json              # JSON Schema for rule files
```

### 8.2 Plan Spec Schema

Every plan (template) follows a common schema:

```yaml
# --- Metadata ---
name: string                    # Unique identifier (kebab-case)
description: string             # What this plan does
version: string                 # Semver
author: string                  # Who created it
tags: [string]                  # For discovery: ["formatting", "financial", "cuped"]
category: string                # One of: formatting, reports, charts

# --- Parameters ---
parameters:
  param_name:
    type: string | number | boolean | array
    description: string
    default: any                # Optional default
    required: boolean           # Default: false
    enum: [any]                 # Optional: restrict to specific values

# --- Operations ---
operations:                     # Ordered list of actions to execute
  - action: string              # e.g., write.cell, format.range, sheet.ensure
    # ... action-specific fields
    # All string values support {{ param_name }} Tera interpolation
```

### 8.3 Action Types

| Action | Description |
|--------|-------------|
| `sheet.ensure` | Create sheet if it doesn't exist |
| `sheet.rename` | Rename a sheet |
| `sheet.clear` | Clear all content from a sheet |
| `write.cell` | Write a value or formula to a single cell |
| `write.range` | Write headers and/or rows to a range |
| `write.table` | Create an Excel table with a name |
| `format.range` | Apply formatting to a cell range |
| `format.columns` | Set column widths |
| `format.rows` | Set row heights |
| `format.conditional` | Apply conditional formatting rules |
| `format.merge` | Merge cells |
| `format.freeze` | Freeze panes at a given cell |
| `chart.create` | Create a chart object |
| `comment.add` | Add a cell comment |
| `named_range.define` | Define or update a named range |
| `filter.auto` | Apply auto-filter to a range |

### 8.4 Worked Example: CUPED Results Table

```yaml
name: cuped-results-table
description: >
  Creates a formatted CUPED experiment results table with
  treatment/control comparison, variance reduction metrics,
  and significance indicators.
version: "1.0"
author: "Brian Weisberg"
tags: ["experimentation", "cuped", "statistics"]
category: reports

parameters:
  metric:
    type: string
    description: "Primary metric name"
    required: true
  pre_period:
    type: string
    description: "CUPED pre-period identifier"
    required: true
  sheet:
    type: string
    default: "Results"
  start_cell:
    type: string
    default: "A1"

operations:
  - action: sheet.ensure
    name: "{{ sheet }}"

  - action: write.range
    sheet: "{{ sheet }}"
    start: "{{ start_cell }}"
    headers:
      - "Group"
      - "N"
      - "Raw Mean"
      - "CUPED-Adjusted Mean"
      - "Std Error"
      - "Lift (%)"
      - "p-value"
      - "Significant?"
    rows:
      - ["Control", null, null, null, null, null, null, null]
      - ["Treatment", null, null, null, null, null, null, null]

  - action: format.range
    range: "{{ start_cell }}:H1"
    style:
      bold: true
      fill: "4472C4"
      font_color: "FFFFFF"
      font_size: 11
      alignment: center

  - action: format.columns
    sheet: "{{ sheet }}"
    widths:
      A: 12
      B: 10
      C: 16
      D: 22
      E: 14
      F: 12
      G: 12
      H: 14

  - action: format.range
    range: "F2:F3"
    number_format: '0.0%'

  - action: format.range
    range: "G2:G3"
    number_format: '0.0000'

  - action: format.conditional
    range: "G2:G3"
    rules:
      - condition: "value < 0.05"
        fill: "C6EFCE"
        font_color: "006100"
      - condition: "value >= 0.05"
        fill: "FFC7CE"
        font_color: "9C0006"

  - action: write.cell
    address: "{{ sheet }}!H2"
    formula: '=IF(G2<0.05,"Yes","No")'

  - action: write.cell
    address: "{{ sheet }}!H3"
    formula: '=IF(G3<0.05,"Yes","No")'

  - action: write.cell
    address: "A5"
    sheet: "{{ sheet }}"
    value: "CUPED Pre-Period: {{ pre_period }}"
    style:
      italic: true
      font_color: "808080"
      font_size: 9
```

### 8.5 Validation Rule Types

Rules used by `lint`, `validate`, and `doctor`:

| Check | Description |
|-------|-------------|
| `no_formula_errors` | Cells in range have no #REF!, #DIV/0!, etc. |
| `formula_consistency` | All cells in a row/column use the same formula pattern |
| `input_cell_color` | Input cells (hardcoded values) use blue font |
| `formula_cell_color` | Formula cells use black font |
| `no_hardcodes_in_formula_zone` | Flag hardcoded numbers adjacent to formula cells |
| `number_format_match` | Cells in range use the expected number format |
| `no_blank_headers` | Header row has no empty cells |
| `column_type_consistency` | All values in a column are the same type |
| `year_as_text` | Year values formatted as text, not numbers |
| `negative_in_parentheses` | Negative numbers use (123) not -123 |
| `xlfn_prefix` | Modern functions have correct `_xlfn.` prefix |
| `xlpm_prefix` | LAMBDA parameters have correct `_xlpm.` prefix |
| `defined_name_validity` | Named range formulas are well-formed |
| `table_range_conflict` | No overlapping table/autofilter ranges |

---

## 9. Agent Skill Integration

### 9.1 Claude Code SKILL.md

```markdown
# XLI — Excel CLI

## Quick Reference

| Task | Command |
|------|---------|
| Inspect workbook | `xli inspect file.xlsx` |
| Read a range | `xli read file.xlsx "Sheet!A1:D20"` |
| Write a value | `xli write file.xlsx "B10" --value 42` |
| Write a formula | `xli write file.xlsx "B10" --formula "=SUM(B5:B9)"` |
| Format cells | `xli format file.xlsx "A1:G1" --bold --fill 4472C4` |
| Batch edits | `echo '...' \| xli batch file.xlsx --stdin` |
| Apply template | `xli apply file.xlsx --spec templates/financial-summary.yaml` |
| Full quality check | `xli doctor file.xlsx` |

## Workflow

1. **Inspect** the workbook to understand structure and get its fingerprint
2. **Read** relevant ranges to understand current state
3. **Write** data and formulas (each write is an atomic commit — safe to do one at a time)
4. **Format** cells and ranges (each format is an atomic commit)
5. **Doctor** to lint, recalculate, and validate in one pass
6. If doctor reports issues, **repair** auto-fixable problems, fix others manually

## Rules

- ALWAYS use `--formula` for calculated cells, never `--value` with a pre-computed number
- ALWAYS run `xli doctor` after finishing edits to catch formula errors
- Parse all XLI output as JSON — never use regex on XLI output
- Use `xli batch --stdin` for >20 related edits instead of individual commits
- Use `xli apply --spec` for operations defined in the knowledge base
- Use `xli read --limit N` for large sheets to avoid context window overflow
- Use `--expect-fingerprint` when multiple agents may edit the same file
- Each xli call is an atomic commit. Safe to retry any single operation on failure.
```

### 9.2 Sub-Agent Integration

In the Agile Agentic Analytics ecosystem, XLI is primarily consumed by two sub-agents:

- **athena-analyst** — Queries data from Athena, then uses XLI to write results into formatted workbooks. Workflow: `sqlservd query → xli create → xli batch (data + formulas) → xli doctor`.
- **experiment-analyst** — Runs CUPED analysis, then uses XLI to produce experiment report workbooks. Uses `xli apply --spec cuped-results-table.yaml` with parameters derived from the analysis output.

### 9.3 Permissions Model

```json
{
  "permissions": {
    "allow": [
      "Bash(xli *)",
      "Read(~/.xli/**)",
      "Read(.xli/**)",
      "Read(knowledge/xli/**)"
    ]
  }
}
```

---

## 10. Performance Budget

The atomic commit model depends on predictable, fast execution.

| Operation | Target | Notes |
|-----------|--------|-------|
| Cold start (binary load) | < 2ms | Rust static binary, no interpreter |
| `xli inspect` (small file, <1MB) | < 10ms | calamine fast path |
| `xli inspect` (large file, ~50MB) | < 200ms | calamine streaming |
| `xli read` (single cell) | < 10ms | calamine fast path |
| `xli read` (1000-row range) | < 50ms | calamine with serde |
| `xli write` (single cell, atomic commit) | < 15ms | OOXML patch: open ZIP → patch XML → write ZIP → sync → rename |
| `xli write` (100 cells, bulk stdin) | < 30ms | Single open → batch patch → single commit |
| `xli batch` (50 ops, atomic commit) | < 40ms | Single open → execute all → single commit |
| `xli format` (range, single style) | < 15ms | OOXML patch |
| `xli apply` (20-operation plan) | < 50ms | Single open → execute all → single commit |
| `xli lint` | < 30ms | calamine scan + formula parsing |
| `xli recalc` | 1–5s | Dominated by LibreOffice subprocess |
| `xli validate` (post-recalc scan) | < 30ms | calamine scan |
| `xli doctor` | 1–6s | lint + recalc + validate |
| `xli create` (new empty workbook) | < 5ms | rust_xlsxwriter |
| `xli create` (from 10K-row CSV) | < 200ms | rust_xlsxwriter streaming |
| `xli ooxml diff` | < 100ms | Parallel ZIP reads |
| Fingerprint computation | < 5ms | SHA-256 of file bytes |

20 sequential atomic writes complete in ~300ms — less than the time it takes Python to import openpyxl once.

---

## 11. Implementation Phases

### Phase 1: Foundation (v0.1.0)

**Goal:** An agent can inspect, read, write, format, batch, recalculate, lint, validate, and create workbooks using CLI commands instead of generating Python scripts. Every mutating command is an atomic workbook commit with fingerprinting.

**Scope:**
- `inspect`, `read`, `write`, `format`, `sheet`, `batch`, `lint`, `recalc`, `validate`, `doctor`, `create`, `schema`
- Atomic commit layer: locks, fingerprinting, staging, `sync_all()`, atomic rename
- `--expect-fingerprint` on all mutating commands
- OOXML patch engine: cell values/formulas, shared strings, styles/number formats, defined names, sheet add/rename/delete, workbook metadata, macro part pass-through
- `umya-spreadsheet` fallback for operations outside OOXML patch coverage (with warnings)
- `calamine` for all read operations
- `rust_xlsxwriter` for new file creation
- Structured JSON envelopes with transaction metadata
- Structured error handling with fix suggestions
- Formula normalization checks in `lint` (`_xlfn.`, `_xlpm.`, defined names)
- Knowledge base loader (templates, rules)
- SKILL.md for Claude Code
- Cross-compilation for Linux (x86_64, aarch64) and macOS (x86_64, aarch64)
- Release binaries on GitHub Releases

**Not in scope:** `apply`, `diff`, `profile`, `chart`, `repair`, `ooxml` commands, Tera-based plan engine.

### Phase 2: Plans and Patch Coverage (v0.2.0)

**Goal:** Complex multi-step operations are declarative YAML plans. The OOXML patch engine covers the most important formatting and structural operations, reducing reliance on umya.

**Scope:**
- `apply` command with Tera-based plan engine
- `repair` command for auto-fixable lint issues
- `profile` command for data profiling
- `chart` command (basic chart creation)
- `ooxml unpack|pack|diff|grep` commands
- Dry-run mode for `apply` and `batch`
- OOXML patch coverage extended to: comments/notes, data validation, conditional formatting, tables, column widths/row heights
- Built-in template library (formatting, CUPED results, experiment report scaffolds)
- `template` command (list, preview, validate)

### Phase 3: Intelligence (v0.3.0)

**Goal:** XLI is a full-featured transactional OOXML compiler with comprehensive patch coverage, advanced quality tools, and minimal fallback to umya.

**Scope:**
- `diff` command for value-level and structural comparison
- Advanced chart modification and styling in OOXML patch engine
- Chart support in `rust_xlsxwriter` path for new files
- Validation rule authoring (custom rules in YAML)
- Template versioning and dependency management
- OOXML patch coverage extended to: drawing anchors, pivot caches, sparklines
- `umya-spreadsheet` dependency removable for workbooks within patch engine coverage
- Optional Rust-native fast-preflight formula checker (SUM, AVERAGE, IF, VLOOKUP)
- MCP schema export (`xli schema --openapi`) for bridge scenarios
- Performance optimization for large workbooks (>100K rows, calamine streaming)

---

## 12. Success Criteria

### 12.1 Quantitative

- **Token reduction:** Agent uses ≥60% fewer output tokens for Excel operations compared to generating openpyxl scripts
- **Error reduction:** Formula errors (#REF!, #DIV/0!, etc.) in agent-produced workbooks drop by ≥80% due to structured lint/validate
- **Time to first working workbook:** Agent produces a correctly formatted, validated workbook in ≤3 tool calls (inspect → apply → doctor) vs. current 8-15 calls (generate script → run → check → fix → repeat)
- **Execution speed:** 20 atomic commits complete in <300ms total
- **Zero silent corruption:** No workbook artifact (chart, conditional format, comment, macro, data validation) is silently dropped or corrupted during a roundtrip edit
- **Binary size:** < 15MB statically linked

### 12.2 Qualitative

- An agent that has never seen XLI before can use `xli schema` to bootstrap its own understanding and produce a valid workbook without reading SKILL.md
- A Vanguard analyst can author a formatting template in YAML without writing code
- The experiment-analyst sub-agent can produce a complete CUPED results workbook using only `xli` commands — no openpyxl code generation anywhere in the pipeline
- XLI deploys as a single binary — no Python, no pip, no virtual environment, no runtime dependencies
- Two agents can safely edit the same workbook without silent data loss, using fingerprint-based compare-and-swap

---

## 13. Open Questions

1. **Daemon mode?** Should XLI support a long-running daemon that keeps a workbook open via a Unix domain socket? This would push atomic writes from ~15ms to ~1ms. Current lean: no — 15ms is fast enough and statelessness is simpler. Revisit if agents routinely do 100+ sequential edits.

2. **OOXML patch engine completeness timeline.** The patch engine is the hardest piece to build. Streaming XML transforms that preserve every Excel artifact (drawing anchors, pivot caches, sparklines, slicer caches, timeline objects) is a multi-month effort. The phased approach with umya fallback is realistic for v0.1, but the graduation plan needs concrete milestones.

3. **Sheetcraft convergence?** Should sheetcraft Pydantic specs be a valid input to `xli apply`? If they can be serialized to YAML matching XLI's plan schema, convergence is natural.

4. **Template registry?** Local-first + git-distributed vs. shared registry. Current lean: git-distributed.

5. **Formula construction DSL in plans?** Should plans support `SUM(col_ref("B", row_start, row_end))` instead of raw `=SUM(B5:B9)` for row-shift resilience? Adds a DSL layer.

6. **Built-in formula evaluator?** Fast Rust-native evaluation of common functions (SUM, AVERAGE, IF, VLOOKUP) to reduce LibreOffice dependence. Current lean: Phase 3 as a preflight checker, not as a replacement for authoritative recalc.

7. **Windows atomic rename.** `std::fs::rename` on Windows is not atomic if the destination already exists. Need to use `ReplaceFile` via windows-sys for true atomic semantics, or document the limitation.
