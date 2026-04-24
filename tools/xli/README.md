# xli

`xli` is a Rust CLI for deterministic, JSON-first Excel workbook operations. It is the fast core in this directory; `xli-companion` is the Python sidecar for heavier validation, reconciliation, and report generation.

## Current Status

Implemented today:

- `inspect`, `read`, `write`, `format`, `sheet`, `batch`, `apply`, `create`, `lint`, `recalc`, `validate`, `doctor`, `template`, and `schema`
- Atomic commits, dry runs, and fingerprint compare-and-swap on mutating workbook commands
- CSV, Markdown table, and JSON workbook creation
- Built-in template discovery and preview through `xli template`
- Template application through `xli apply`, backed by the same atomic path as `xli batch`

Still planned or partial:

- Native OOXML mutation for all edit paths. The current mutation path uses `umya-spreadsheet` and reports that warning in JSON envelopes.
- First-class `profile`, `diff`, `chart`, `table`, `repair`, and `ooxml` command families.
- Rich report-table ergonomics beyond the initial number-format aliases and basic table template.

## Build

From this directory:

```bash
cargo build
```

Run the debug binary:

```bash
cargo run -p xli-cli -- inspect example.xlsx
```

Run the installed binary after `cargo install --path xli/xli-cli`:

```bash
xli inspect example.xlsx
```

## Examples

Create a workbook:

```bash
cargo run -p xli-cli -- create /tmp/demo.xlsx --sheets Summary,Data
```

Inspect workbook structure:

```bash
cargo run -p xli-cli -- inspect /tmp/demo.xlsx
```

Write a value and a formula:

```bash
cargo run -p xli-cli -- write /tmp/demo.xlsx "Summary!A1" --value '"Revenue"'
cargo run -p xli-cli -- write /tmp/demo.xlsx "Summary!B2" --formula "=SUM(Data!B:B)"
```

Read a cell or range:

```bash
cargo run -p xli-cli -- read /tmp/demo.xlsx "Summary!A1"
cargo run -p xli-cli -- read /tmp/demo.xlsx "Summary!A1:B10" --headers --limit 20
```

Format a range. Number formats accept literal Excel formats or aliases such as `currency`, `percent_int`, and `datetime_iso`:

```bash
cargo run -p xli-cli -- format /tmp/demo.xlsx "Summary!B:B" --number-format currency
```

Run a batch:

```bash
printf '%s\n' \
  '{"op":"write","address":"Summary!A1","value":"Revenue"}' \
  '{"op":"format","range":"Summary!A1:B1","bold":true,"fill":"4472C4","font_color":"FFFFFF"}' |
  cargo run -p xli-cli -- batch /tmp/demo.xlsx --stdin
```

Preview and apply a built-in template:

```bash
cargo run -p xli-cli -- template list
cargo run -p xli-cli -- template preview basic-table-format --param range=Summary!A1:B10
cargo run -p xli-cli -- apply /tmp/demo.xlsx basic-table-format --param range=Summary!A1:B10
```

Run quality checks:

```bash
cargo run -p xli-cli -- lint /tmp/demo.xlsx
cargo run -p xli-cli -- doctor /tmp/demo.xlsx --skip-recalc
```

Discover schemas:

```bash
cargo run -p xli-cli -- schema
cargo run -p xli-cli -- schema --command create
cargo run -p xli-cli -- schema --result FormatOutput
```

## Tests

Rust workspace:

```bash
cargo test
```

Python companion:

```bash
cd xli-companion
uv run --extra dev pytest
```

## xli vs xli-companion

Use `xli` when you need fast, addressable workbook operations: inspect, read, write, format, batch, create, and schema discovery.

Use `xli-companion` when you need Python's ecosystem: dataframe validation, reconciliation against source data, OOXML artifact audits, report rendering, or optional real-engine checks.

## Spec Parity Matrix

| Spec command | Current status | Notes |
| --- | --- | --- |
| `inspect` | Implemented | JSON envelope with workbook metadata and fingerprints |
| `read` | Implemented | Cells, ranges, tables, pagination, formulas, Markdown output |
| `write` | Implemented | Values and formulas through atomic commit path |
| `format` | Implemented | Range formatting, number-format aliases, column widths |
| `sheet` | Implemented | Add, remove, rename, copy, reorder, hide, unhide |
| `batch` | Implemented | NDJSON micro-ops in one atomic commit |
| `apply` | Implemented minimal | Built-in template expansion into batch ops |
| `template` | Implemented minimal | List, preview, and validate built-in templates |
| `create` | Implemented | Blank, CSV, Markdown table, and JSON inputs |
| `lint` | Implemented MVP | Fast structural/formula checks |
| `recalc` | Implemented | LibreOffice subprocess path |
| `validate` | Implemented MVP | Post-edit workbook validation checks |
| `doctor` | Implemented | Runs quality pipeline, optionally skipping recalc |
| `schema` | Implemented | Command, result, full, and OpenAPI-style schema output |
| `profile` | Deferred | Not exposed as a CLI command yet |
| `diff` | Deferred | Not exposed as a CLI command yet |
| `chart` | Deferred | Not exposed as a CLI command yet |
| `table` | Partial/deferred | `read --table` exists; first-class table creation/edit command is planned |
| `repair` | Deferred | Not exposed as a CLI command yet |
| `ooxml unpack/pack/diff/grep` | Deferred | Low-level package helpers exist; CLI family is not exposed |

## Known Limitations

- Mutating commands currently emit the `umya-spreadsheet` fallback warning. Treat that as a signal to verify artifact-sensitive workbooks that contain charts, drawings, macros, data validation, or complex tables.
- `xli-spec.md` is broader than the current MVP. The parity matrix above is the operational source of truth for what the CLI exposes today.
- Formula recalculation depends on LibreOffice being available on the machine.
