# Python Companion to XLI

**Status:** comprehensive design draft

**Last updated:** 2026-03-15

## Executive Summary

`xli` should remain the primary workbook tool. It is the fast, transactional, deterministic surface for inspection, targeted edits, formatting, sheet management, linting, recalculation, validation, and atomic commit safety.

Python should complement `xli`, not replace it.

The Python companion exists for the workloads where Python is materially stronger:

- heavy validation and reconciliation
- rich tabular processing
- OOXML artifact auditing
- final acceptance checks
- specialized report generation
- optional integration with native spreadsheet engines or commercial libraries

The core operating model is:

1. Build and repair workbooks with `xli`.
2. Run `xli doctor` for the fast Rust-native quality gate.
3. Run the Python companion for deeper and slower checks.
4. If the Python companion finds deterministic fixes, express them as `xli` operations and let `xli` apply them atomically.

There is one important exception: Python can be the better producer for a brand-new workbook generated from scratch, especially when the task is mostly data rendering or report assembly. In that case, Python produces the initial workbook and `xli` becomes the finisher, validator, and transactional post-processor.

## Design Goal

Build a single Python-side strategy that is:

- multiplatform
- reliable on macOS
- complementary to `xli`
- practical with the current Python ecosystem
- explicit about where fidelity risks exist

This document treats "works perfectly on mac" as two separate requirements:

1. The Python stack itself must run cleanly on macOS.
2. The overall architecture must avoid depending on Windows-only spreadsheet tooling.

That does not mean every Python library can perfectly round-trip every arbitrary Excel workbook. In practice, the open-source ecosystem still has real limits there. The design should acknowledge those limits and place `xli` at the mutation boundary whenever possible.

## Core Thesis

`xli` and Python should be symbiotic in the following way:

- `xli` owns fast workbook operations.
- Python owns heavyweight reasoning.
- `xli` owns the transaction boundary.
- Python owns rich analysis and ecosystem leverage.
- `xli` is the default path.
- Python is the escalation path.

That gives us a two-speed system:

- hot loop: `xli`
- cold loop: Python

## Hard Requirements

The companion design should satisfy these constraints:

- Must be multiplatform.
- Must work well on macOS.
- Must not require Windows COM.
- Must not assume Excel is installed unless the user explicitly opts into that mode.
- Must not force Python interpreter startup for every simple workbook edit.
- Must preserve `xli` as the preferred mutation tool for existing workbooks.
- Must fit into CI and local workflows.

## Non-Goals

The Python companion should not be:

- the default editing interface for workbooks
- a replacement for `xli write`, `xli format`, `xli sheet`, or `xli batch`
- a license to generate bespoke openpyxl scripts for every task
- a Windows-only automation story
- the authoritative concurrency and compare-and-swap layer

## Why Python Still Matters

Python remains valuable because its ecosystem is much broader than the current Rust spreadsheet ecosystem in a few specific areas:

- dataframe manipulation
- statistical analysis
- data contracts and validation
- fuzzy matching and anomaly detection
- report generation
- integration with SQL engines
- optional integration with real spreadsheet applications
- optional commercial-grade spreadsheet SDKs

If the question is "set cell B5 to 100," Python is overkill.

If the question is "compare six workbook tabs to three parquet inputs, validate the schema, flag suspicious formulas, inspect OOXML relationships for missing charts, and generate an HTML acceptance report," Python is the pragmatic tool.

## Symbiotic Responsibilities

### `xli` should own

- inspect, read, write, format, and sheet operations
- atomic workbook commits
- fingerprint comparison
- deterministic mutation commands
- fast structural checks
- fast lint and validation
- recalculation orchestration
- batch application of machine-generated fixes

### Python should own

- heavy data reconciliation
- dataframe-first validation
- template-specific business rules
- report-grade summaries
- deep OOXML inspection
- specialized generation workflows
- optional engine-driven verification

### Shared boundary

Python can recommend changes, but `xli` should usually apply them.

That boundary is the key design decision. It prevents the Python side from becoming an ad hoc mutation surface that bypasses `xli`'s lock, fingerprint, and atomic commit model.

## Modes of Cooperation

### Mode 1: `xli`-led editing with Python final checks

This should be the default.

```bash
xli inspect model.xlsx
xli batch model.xlsx ops.ndjson
xli doctor model.xlsx > doctor.json
python python_companion.py --workbook model.xlsx --doctor doctor.json --out report.json
xli batch model.xlsx fixes.ndjson --expect-fingerprint <sha256>
```

Best for:

- existing workbook maintenance
- agentic iterative editing
- spreadsheet repair loops
- CI validation after workbook changes

### Mode 2: Python-led analysis with `xli` remediation

Python inspects the workbook and source data, produces findings, and turns deterministic repairs into `xli` operations.

Best for:

- domain-specific validations
- dataset reconciliation
- anomaly detection
- acceptance audits

### Mode 3: Python creates a new workbook, `xli` finishes it

This is the main exception to the "Python should not write workbooks" rule.

If the workbook is being generated from scratch and the task is mostly:

- dataframe export
- report tab construction
- chart-heavy authoring from clean inputs
- templated rendering

then Python can be the producer and `xli` can be the finisher:

```bash
python report_builder.py --input fact.parquet --output draft.xlsx
xli inspect draft.xlsx
xli format draft.xlsx "Summary!B:B" --number-format '$#,##0'
xli doctor draft.xlsx
python python_companion.py --workbook draft.xlsx --out acceptance.json
```

Best for:

- brand-new report generation
- data-heavy report rendering
- scratch exports where full round-trip preservation is not needed

### Mode 4: Engine-driven verification

When workbook behavior must be checked against a real spreadsheet engine, the Python side can optionally call:

- Excel via `xlwings`
- LibreOffice via UNO or `pyoo`
- a commercial engine such as Aspose.Cells

This should be an explicit escalation path, not the default.

## Taxonomy of Python Options

The Python landscape is easier to reason about by category than by package name.

### Category A: existing-workbook file-format editors

These libraries can open and edit workbook files directly.

- `openpyxl`
- Aspose.Cells
- wrapper ecosystems such as `pyexcel` over plugins

### Category B: new-workbook writers

These are optimized for generating new workbooks from scratch rather than preserving old ones.

- `XlsxWriter`
- `PyExcelerate`
- pandas and Polars when writing through `XlsxWriter`

### Category C: data extraction and dataframe tooling

These are best for reading workbook data into analytics-friendly structures.

- `pandas`
- `polars`
- `python-calamine`
- `pyxlsb`
- `xlrd` for legacy `.xls`

### Category D: native spreadsheet engine automation

These operate through a real office application or office runtime.

- `xlwings`
- `pywin32` on Windows
- UNO-based tooling such as `pyoo`

### Category E: support utilities

These do not edit workbooks directly but matter in real pipelines.

- `msoffcrypto-tool`
- `zipfile`
- `lxml`
- `duckdb`
- `pandera`
- `pydantic`
- `jinja2`

## Detailed Option Survey

The survey below focuses on how each option fits a companion model for `xli`.

### `openpyxl`

**Role:** open-source default for reading and editing existing `.xlsx` and `.xlsm` files.

**Strengths:**

- broad feature surface for workbook objects
- styles, comments, tables, defined names, charts, chartsheets
- pure Python and widely deployed
- works on macOS, Linux, and Windows
- can preserve VBA payloads with `keep_vba`

**Weaknesses:**

- not a perfect round-trip engine for arbitrary Excel artifacts
- official docs warn that not all items are read and that shapes may be lost
- slower and heavier than specialized readers for large analytical extraction
- mutation model is not transactional by default

**Best fit with `xli`:**

- limited existing-workbook introspection
- deeper artifact inspection after `xli doctor`
- read-only acceptance checks
- carefully scoped edits only when a Python-only feature is unavoidable

**Recommendation:** keep it in the companion stack, but do not make it the primary mutation engine.

### `XlsxWriter`

**Role:** high-quality new-workbook writer.

**Strengths:**

- excellent formatting and presentation support
- strong support for charts, tables, images, comments, conditional formatting, and macros
- fast and predictable for report generation
- works cleanly on macOS and other platforms
- ideal for scratch workbook creation

**Weaknesses:**

- cannot read or modify an existing workbook
- not useful as a repair tool for arbitrary files

**Best fit with `xli`:**

- Python generates a new workbook
- `xli` inspects, finishes, validates, and performs atomic post-processing

**Recommendation:** use it whenever Python is responsible for creating a new polished workbook from scratch.

### `pandas`

**Role:** general-purpose dataframe layer for workbook data.

**Strengths:**

- huge ecosystem
- flexible Excel I/O engine support
- strong joins, reshaping, aggregation, and QA workflows
- integrates naturally with validation and reporting tools

**Weaknesses:**

- not workbook-fidelity oriented
- loses workbook semantics once data is loaded into dataframes
- write path is appropriate for exports, not preservation-sensitive editing

**Best fit with `xli`:**

- compare workbook data to external truth
- schema and reconciliation checks
- acceptance reporting
- source-to-output consistency validation

**Recommendation:** a companion default for data-heavy validation.

### `Polars`

**Role:** high-performance dataframe engine with Excel bridges.

**Strengths:**

- faster analytics workflows than pandas in many cases
- good fit for large-sheet validation and profiling
- useful for modern data pipelines
- cross-platform and macOS-friendly

**Weaknesses:**

- still not a workbook-fidelity editor
- Excel support depends on external engines rather than native workbook editing

**Best fit with `xli`:**

- large-sheet profiling
- reconciliation jobs
- anomaly checks
- data-contract enforcement

**Recommendation:** strong option if performance matters more than ecosystem breadth.

### `python-calamine`

**Role:** fast read-only workbook parser backed by the Rust `calamine` library.

**Strengths:**

- broad format coverage: `.xls`, `.xlsx`, `.xlsm`, `.xlsb`, `.ods`
- good fit for fast extraction
- complements pandas and Polars
- works on macOS and other platforms

**Weaknesses:**

- read-only
- not a workbook editor
- not a style- and artifact-rich inspection surface

**Best fit with `xli`:**

- fast analytical reads
- large validation jobs
- multiprotocol import support

**Recommendation:** excellent read-path companion for `xli`.

### `pyxlsb`

**Role:** focused `.xlsb` reader.

**Strengths:**

- useful for binary workbook ingestion when needed

**Weaknesses:**

- narrow scope
- limited semantics compared with richer readers
- not an editing solution

**Best fit with `xli`:**

- niche read support when `.xlsb` files enter the pipeline

**Recommendation:** optional compatibility layer, not part of the default architecture.

### `xlrd`

**Role:** legacy `.xls` reader.

**Strengths:**

- still relevant for old Excel formats

**Weaknesses:**

- no longer a modern Excel strategy
- not suitable as the center of a contemporary companion stack

**Best fit with `xli`:**

- legacy import edge case only

**Recommendation:** optional fallback only.

### `xlwings`

**Role:** automation through real Microsoft Excel.

**Strengths:**

- high behavior fidelity because Excel itself is executing
- useful for checking real workbook behavior
- supports macOS and Windows

**Weaknesses:**

- requires Excel installation
- not a Linux-first or CI-default story
- open-source feature set differs from PRO features
- macOS support is good but not identical to Windows, and docs note macOS limitations such as no UDF support

**Best fit with `xli`:**

- optional engine verification on developer machines
- explicit acceptance check for Excel-native behavior on macOS and Windows

**Recommendation:** use only as an opt-in escalation path.

### `pywin32`

**Role:** Windows COM automation.

**Strengths:**

- strong Windows-native Excel automation

**Weaknesses:**

- Windows-only
- fails the multiplatform and macOS requirement

**Best fit with `xli`:**

- none for the core companion design

**Recommendation:** reject as a primary architecture choice.

### UNO / `pyoo`

**Role:** automation through LibreOffice/OpenOffice.

**Strengths:**

- cross-platform office-engine automation
- can run on macOS, Linux, and Windows in principle

**Weaknesses:**

- operational overhead is higher than file-format libraries
- requires LibreOffice/OpenOffice runtime
- Python/runtime integration on macOS is more cumbersome than pure library approaches
- behavior fidelity is LibreOffice fidelity, not Excel fidelity

**Best fit with `xli`:**

- optional Linux/macOS engine verification
- headless batch recalculation or rendering workflows

**Recommendation:** keep as an optional engine adapter, not the default companion base.

### Aspose.Cells for Python via .NET

**Role:** commercial high-fidelity spreadsheet SDK.

**Strengths:**

- broad feature coverage
- cross-platform support including macOS
- much stronger candidate for fidelity-sensitive workflows than most open-source file-format editors
- useful when workbook complexity exceeds what `openpyxl` can safely cover

**Weaknesses:**

- commercial licensing
- .NET runtime dependency
- more operational complexity than pure-Python libraries

**Best fit with `xli`:**

- premium companion mode for fidelity-sensitive pipelines
- advanced validation and manipulation when open-source limits become a blocker

**Recommendation:** strongest commercial option if the open-source stack proves insufficient.

### `PyExcelerate`

**Role:** very fast writer for cell-heavy new workbook generation.

**Strengths:**

- good for bulk write throughput

**Weaknesses:**

- not a comprehensive workbook editor
- narrower feature set than `XlsxWriter`

**Best fit with `xli`:**

- bulk export specialist when styling and fidelity needs are limited

**Recommendation:** niche option; `XlsxWriter` is the more general new-workbook choice.

### `pyexcel`

**Role:** convenience API over multiple spreadsheet backends.

**Strengths:**

- can simplify basic import/export use cases

**Weaknesses:**

- abstraction layer can obscure backend-specific limits
- not a superior choice for fidelity-sensitive workflows

**Best fit with `xli`:**

- lightweight utility scenarios only

**Recommendation:** not part of the core recommended stack.

### `msoffcrypto-tool`

**Role:** decrypting encrypted Office files before analysis.

**Strengths:**

- useful in real enterprise pipelines
- orthogonal to editing and validation

**Weaknesses:**

- not an editor

**Best fit with `xli`:**

- preprocessing step before handing the workbook to `xli` or the Python validator

**Recommendation:** optional but valuable support tool.

## Platform and macOS Analysis

The platform requirement narrows the recommended architecture substantially.

### Safe cross-platform baseline

These are the safest default choices for macOS plus multiplatform support:

- `openpyxl`
- `XlsxWriter`
- `pandas`
- `polars`
- `python-calamine`
- `msoffcrypto-tool`
- `duckdb`
- `pandera`
- `pydantic`
- `zipfile`
- `lxml`

### Mac-capable but not universal default choices

- `xlwings`
- LibreOffice UNO / `pyoo`
- Aspose.Cells for Python via .NET

These can be useful, but they come with runtime or licensing assumptions and should be treated as optional modes.

### Reject for primary companion design

- `pywin32`

It breaks the macOS requirement immediately.

## Fidelity Reality Check

If "works perfectly on mac" means "the Python library itself installs and runs on macOS," that is achievable.

If it means "every arbitrary Excel workbook can be opened, modified, and saved with perfect artifact preservation using open-source Python libraries," that is not a defensible claim today.

Based on official documentation:

- `openpyxl` is powerful but not a perfect round-trip engine for all Excel artifacts.
- `XlsxWriter` is excellent for new files but does not modify existing ones.
- `xlwings` can access real Excel behavior on macOS, but only when Excel is installed and only on supported local environments.
- UNO can automate LibreOffice across platforms, but that is not the same as Excel-native fidelity.
- Aspose.Cells is the strongest cross-platform Python option if high fidelity is worth the commercial dependency.

This is exactly why `xli` should remain the core mutation surface.

## Recommended Architecture

### Open-source default stack

This should be the baseline recommendation.

**Mutation and fast checks**

- `xli`

**Fast reads and extraction**

- `python-calamine`
- `polars` or `pandas`

**Workbook artifact inspection**

- `openpyxl`
- `zipfile`
- `lxml`

**Validation and reconciliation**

- `pandera`
- `pydantic`
- `duckdb`
- `numpy`
- `scipy` when needed

**Reporting**

- `jinja2`
- `markdown`
- `rich`

This stack is cross-platform, works on macOS, and complements `xli` cleanly.

### Open-source plus engine escalation

Use the open-source default stack, plus:

- `xlwings` for macOS/Windows Excel-engine verification
- UNO or `pyoo` for LibreOffice-based checks where appropriate

This should be opt-in, not default.

### Commercial high-fidelity stack

Use the open-source default stack, but replace or augment certain workbook operations with:

- Aspose.Cells for Python via .NET

This is the best candidate if strict fidelity requirements emerge and the budget supports it.

## Recommended Symbiotic Workflows

### Workflow A: iterative workbook editing

```bash
xli inspect model.xlsx
xli batch model.xlsx ops.ndjson
xli doctor model.xlsx > doctor.json
python python_companion.py --workbook model.xlsx --doctor doctor.json --out companion.json
xli batch model.xlsx companion-fixes.ndjson --expect-fingerprint <sha256>
```

Use when:

- a workbook already exists
- many small edits are expected
- atomic safety matters

### Workflow B: source-to-workbook reconciliation

```bash
xli inspect model.xlsx > inspect.json
python python_companion.py \
  --workbook model.xlsx \
  --inspect inspect.json \
  --source facts.parquet \
  --out acceptance.json
```

Use when:

- workbook numbers must match external truth
- complex data reconciliation is needed

### Workflow C: scratch report generation

```bash
python build_report.py --input facts.parquet --output draft.xlsx
xli inspect draft.xlsx
xli doctor draft.xlsx
python python_companion.py --workbook draft.xlsx --out final-audit.json
```

Use when:

- the workbook is brand-new
- Python is easier for the initial render
- `xli` still needs to validate and finish the output

### Workflow D: engine verification on macOS

```bash
xli doctor model.xlsx
python python_companion.py --workbook model.xlsx --engine xlwings --out engine-audit.json
```

Use when:

- workbook behavior must be checked in actual Excel on macOS
- the local machine has Excel installed

## What the Python Companion Should Actually Do

The first production version should be narrow and useful.

### Minimum useful scope

1. Read `xli inspect` and `xli doctor` outputs.
2. Open the workbook through one or more Python readers.
3. Run domain-specific and artifact-specific checks.
4. Emit structured findings.
5. Emit an optional deterministic fix plan in an `xli`-friendly form.

### Suggested checks

- required sheet presence
- named range integrity
- source-vs-workbook totals reconciliation
- duplicate key checks in extracted tables
- null-rate and type coercion issues
- suspicious formula-pattern checks
- missing expected chart or drawing relationships
- template-specific layout conformance
- regression diff against a golden workbook

### Suggested outputs

- machine-readable JSON summary
- markdown or HTML acceptance report
- optional `xli batch` fix file

## Output Contract

The companion should emit JSON, not ad hoc prose. A useful envelope is:

```json
{
  "status": "ok",
  "workbook": "model.xlsx",
  "validated_fingerprint": "sha256:...",
  "platform": {
    "python": "3.13",
    "os": "macOS"
  },
  "summary": {
    "checks_run": 24,
    "errors": 1,
    "warnings": 4
  },
  "findings": [
    {
      "code": "MISSING_CHART_RELATIONSHIP",
      "severity": "error",
      "sheet": "Summary",
      "message": "Expected chart drawing is missing."
    }
  ],
  "fix_plan": [
    {
      "kind": "xli-batch-op",
      "op": "sheet.add",
      "name": "Checks"
    }
  ],
  "artifacts": {
    "markdown_report": "acceptance.md"
  }
}
```

### Contract rules

- Always include the workbook fingerprint that was validated.
- Never apply fixes silently.
- Emit deterministic fix plans only when confidence is high.
- Treat nondeterministic or advisory findings as warnings that require human or agent review.

## Decision Matrix

| Scenario | Best choice | Why |
|---|---|---|
| Existing workbook, small surgical edits | `xli` | Fast, atomic, addressable |
| Existing workbook, deep artifact inspection | `openpyxl` + `zipfile` + `lxml` | Rich introspection without making Python the transaction layer |
| Large-sheet data validation | `python-calamine` + `polars` or `pandas` | Fast extraction and strong analytics tooling |
| Brand-new polished workbook generation | `XlsxWriter` | Best scratch writer |
| Bulk write-heavy export with simple formatting | `PyExcelerate` | Throughput specialist |
| macOS or Windows check against real Excel behavior | `xlwings` | Uses Excel itself |
| Cross-platform office-engine automation | UNO / `pyoo` | Runtime-heavy but portable |
| Highest Python-side fidelity with budget | Aspose.Cells | Broadest serious cross-platform SDK |
| Legacy `.xls` import only | `xlrd` | Narrow legacy fallback |
| `.xlsb` ingestion | `python-calamine` or `pyxlsb` | Read compatibility |

## Recommended Positioning Statements

These are the statements the document should defend consistently.

### Statement 1

`xli` is the operational core. Python is the specialist extension layer.

### Statement 2

The companion should be read-heavy, validation-heavy, and report-heavy.

### Statement 3

Python should usually suggest or prepare changes, while `xli` applies them atomically.

### Statement 4

For new workbook generation, Python can be a producer and `xli` can be the finisher.

### Statement 5

Open-source Python is good enough for a strong companion, but not strong enough to claim perfect arbitrary workbook round-tripping on its own.

## Phased Implementation Plan

### Milestone 1: read-only validator

Build a Python companion that:

- ingests `xli inspect` and `xli doctor`
- reads workbook data and artifact metadata
- emits structured findings

No direct workbook edits.

### Milestone 2: fix-plan generator

Add:

- `xli batch` ndjson output
- rule-based repair suggestions
- report generation

Still no direct workbook edits by Python for existing files.

### Milestone 3: scratch workbook generator path

Support a sanctioned Python-first generation workflow:

- data extraction and transformation in Python
- initial workbook generation in `XlsxWriter`
- final formatting, validation, and acceptance through `xli`

### Milestone 4: optional fidelity escalations

Add optional adapters for:

- `xlwings`
- UNO / `pyoo`
- Aspose.Cells

These should remain explicitly optional because they change operational complexity.

## Final Recommendation

The best default companion architecture is:

- `xli` for edits, safety, and fast quality gates
- `python-calamine` for fast reads
- `polars` or `pandas` for data-heavy analysis
- `openpyxl` plus `zipfile`/`lxml` for workbook artifact inspection
- `pandera`, `pydantic`, and `duckdb` for validation logic
- `XlsxWriter` only for brand-new workbook generation
- `xlwings` or UNO only as optional engine verification layers
- Aspose.Cells only if commercial fidelity requirements justify it

This satisfies the multiplatform requirement, works well on macOS, and keeps the architecture aligned with `xli` rather than competing with it.

## Source Notes

The recommendations above are based on current official documentation and project references as of 2026-03-15.

Primary references:

- [openpyxl tutorial](https://openpyxl.readthedocs.io/en/3.1/tutorial.html)
- [openpyxl optimized modes](https://openpyxl.readthedocs.io/en/3.1.2/optimized.html)
- [openpyxl API: `load_workbook`](https://openpyxl.readthedocs.io/en/3.1.1/api/openpyxl.reader.excel.html)
- [XlsxWriter documentation](https://xlsxwriter.readthedocs.io/)
- [XlsxWriter FAQ](https://xlsxwriter.readthedocs.io/faq.html)
- [XlsxWriter macros](https://xlsxwriter.readthedocs.io/working_with_macros.html)
- [pandas `ExcelFile` reference](https://pandas.pydata.org/pandas-docs/version/2.3.0/reference/api/pandas.ExcelFile.html)
- [pandas I/O guide](https://pandas.pydata.org/pandas-docs/version/2.2/user_guide/io.html)
- [Polars Excel guide](https://docs.pola.rs/user-guide/io/excel/)
- [python-calamine README](https://github.com/dimastbk/python-calamine)
- [pyxlsb on PyPI](https://pypi.org/project/pyxlsb/)
- [xlrd on PyPI](https://pypi.org/project/xlrd/)
- [xlwings installation and platform notes](https://docs.xlwings.org/en/0.32.1/installation.html)
- [pyoo on PyPI](https://pypi.org/project/pyoo/)
- [Aspose.Cells for Python via .NET](https://docs.aspose.com/cells/python-net/getting-started/)
- [pywin32 on PyPI](https://pypi.org/project/pywin32/)
- [msoffcrypto-tool on PyPI](https://pypi.org/project/msoffcrypto-tool/)
