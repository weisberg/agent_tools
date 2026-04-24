# xli-companion

Python companion for [xli](https://github.com/weisberg/xli) — heavyweight validation, reconciliation, and reporting for Excel workbooks.

## Overview

`xli` is the fast, transactional, deterministic core for workbook inspection, editing, formatting, and atomic commits. The Python companion handles the workloads where Python's ecosystem is materially stronger: deep validation, data reconciliation, OOXML artifact auditing, and rich report generation.

The symbiotic relationship:

- **xli** owns fast workbook operations, atomic commits, fingerprint comparison, and deterministic mutations.
- **xli-companion** owns heavyweight reasoning — dataframe-first validation, cross-source reconciliation, artifact inspection, and acceptance reporting.
- Python recommends changes; `xli` applies them atomically.

## Installation

Core dependencies only:

```bash
pip install -e .
```

With development tools (pytest, ruff, mypy):

```bash
pip install -e ".[dev]"
```

Everything (pandas, pandera, xlwings, dev tools):

```bash
pip install -e ".[all]"
```

Optional extras can also be installed individually:

| Extra        | Packages              | Use case                          |
|--------------|-----------------------|-----------------------------------|
| `pandas`     | pandas, pandera       | DataFrame validation with pandera |
| `engine`     | xlwings               | Real Excel engine verification    |
| `commercial` | (reserved)            | Commercial SDK adapters           |
| `dev`        | pytest, ruff, mypy    | Development and CI                |

Requires Python 3.11+.

## Quick Start

```bash
# 1. Create or edit a workbook with xli
xli inspect model.xlsx
xli batch model.xlsx ops.ndjson

# 2. Run the companion for validation
xli-companion model.xlsx \
  --required-sheets Summary Data \
  --key-columns id \
  --out findings.json

# 3. View the findings
cat findings.json
```

The companion outputs a structured JSON envelope:

```json
{
  "status": "ok",
  "workbook": "model.xlsx",
  "validated_fingerprint": "sha256:...",
  "summary": {
    "checks_run": 8,
    "errors": 0,
    "warnings": 2
  },
  "findings": [...],
  "fix_plan": [...]
}
```

You can also feed in `xli doctor` output for deeper analysis:

```bash
xli doctor model.xlsx > doctor.json
xli-companion model.xlsx --doctor doctor.json --out findings.json
```

Or reconcile workbook data against an external source:

```bash
xli-companion model.xlsx \
  --source facts.parquet \
  --out reconciliation.json
```

## Architecture

The companion cooperates with `xli` through four modes:

### Mode 1: xli-led editing with Python final checks

The default. `xli` performs edits and fast lint; the companion runs deeper validation afterward.

```bash
xli batch model.xlsx ops.ndjson
xli doctor model.xlsx > doctor.json
xli-companion model.xlsx --doctor doctor.json --out report.json
xli batch model.xlsx fixes.ndjson --expect-fingerprint <sha256>
```

### Mode 2: Python-led analysis with xli remediation

The companion inspects the workbook and source data, produces findings, and emits a deterministic fix plan that `xli` applies atomically.

```bash
xli-companion model.xlsx --source facts.parquet --fix-plan fixes.ndjson --out report.json
xli batch model.xlsx fixes.ndjson --expect-fingerprint <sha256>
```

### Mode 3: Python creates a new workbook, xli finishes it

For brand-new report generation, Python produces the initial workbook and `xli` handles finishing, formatting, and validation.

```bash
python -c "
from xli_companion.generate import generate_from_parquet
generate_from_parquet('facts.parquet', 'draft.xlsx')
"
xli format draft.xlsx 'Summary!B:B' --number-format '\$#,##0'
xli doctor draft.xlsx
```

### Mode 4: Engine-driven verification

When workbook behavior must be checked against a real spreadsheet engine, the companion supports optional engine adapters (e.g., xlwings for macOS/Windows).

## Available Checks

### Structural

Validates workbook structure against expectations.

- **Required sheets** — verifies that specified sheet names exist in the workbook.
- **Named ranges** — checks that expected defined names are present.

```bash
xli-companion model.xlsx --required-sheets Summary Data --expected-names fiscal_year region
```

### Data Quality

Per-sheet analysis of cell data loaded into Polars DataFrames.

- **Null rates** — flags columns where the null/blank rate exceeds a threshold (default 50%).
- **Type consistency** — detects columns with mixed data types across rows.
- **Duplicate keys** — checks specified columns for duplicate values.

```bash
xli-companion model.xlsx --key-columns id --null-threshold 0.3
```

### OOXML Artifacts

Deep inspection of the underlying OOXML package using openpyxl, zipfile, and lxml:

- Content type validation
- Chart and drawing relationship integrity
- Shared String Table (SST) consistency

### Formula Analysis

Static analysis of workbook formulas:

- Volatile function detection (INDIRECT, OFFSET, NOW, etc.)
- Error value identification (#REF!, #N/A, #VALUE!, etc.)
- Hardcoded "magic number" detection in formula expressions

### Reconciliation

Cross-source validation when `--source` is provided:

- **Total reconciliation** — compares aggregated totals between workbook and source.
- **Row count matching** — verifies record counts align.
- **Schema comparison** — checks that column names and types are consistent.
- **Value-level diffs** — identifies specific cell-level discrepancies.

## Report Generation

The companion can render findings as Markdown or HTML reports via Jinja2 templates.

```python
from xli_companion.reporting import render_markdown, render_html, write_report

# Render to string
md = render_markdown(result)
html = render_html(result)

# Write to file
write_report(result, Path("report.md"))
write_report(result, Path("report.html"), fmt="html")
```

Reports include the workbook fingerprint, platform info, summary counts, and detailed findings.

## Workbook Generation

For Mode 3 workflows, the `generate` module creates new workbooks from data using XlsxWriter:

```python
from pathlib import Path
from xli_companion.generate import (
    generate_from_dataframe,
    generate_from_csv,
    generate_from_parquet,
    generate_multi_sheet,
)

# Single sheet from a Parquet file
generate_from_parquet(Path("data.parquet"), Path("output.xlsx"))

# Single sheet from CSV
generate_from_csv(Path("data.csv"), Path("output.xlsx"))

# Multiple sheets from DataFrames
import polars as pl
generate_multi_sheet(
    {"Revenue": df_revenue, "Costs": df_costs},
    Path("report.xlsx"),
)
```

After generation, hand the workbook to `xli` for formatting, validation, and atomic post-processing.

## Engine Adapters

The `engines` package provides an abstract `SpreadsheetEngine` interface and optional adapters for real spreadsheet applications.

Currently available:

- **xlwings** — requires `pip install -e ".[engine]"` and a local Excel installation.

Adapters are opt-in. They support operations like opening a workbook, reading cell values after recalculation, and closing the engine cleanly:

```python
from xli_companion.engines.xlwings_adapter import XlwingsEngine

if XlwingsEngine.available():
    with XlwingsEngine() as engine:
        engine.open(Path("model.xlsx"))
        engine.recalculate()
        value = engine.read_cell("Summary", "B5")
```

Custom adapters can be built by subclassing `xli_companion.engines.base.SpreadsheetEngine`.

## Development

```bash
# From the companion directory
cd tools/xli/xli-companion

# Run tests with the documented uv environment
uv run --extra dev pytest

# Optional editable install workflow
git clone <repo-url>
cd xli-companion
pip install -e ".[dev]"

# Run tests after pip install
pytest

# Lint
ruff check src/ tests/

# Type check
mypy src/
```

Configuration lives in `pyproject.toml`:

- **ruff**: line-length 100, target Python 3.11
- **mypy**: strict mode enabled
- **pytest**: local `testpaths = ["tests"]` and `pythonpath = ["src"]`, so
  `uv run --extra dev pytest` uses this package instead of the repository-root
  pytest configuration

## Design Rationale

The full design document — covering the option survey, platform analysis, fidelity trade-offs, and phased implementation plan — is at [`PYTHON_COMPANION_TO_XLI.md`](../PYTHON_COMPANION_TO_XLI.md) in the repository root.
