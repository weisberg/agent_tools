# clipli Development Plan

**Spec:** CLIPLI_SPEC.md v1.0.0-spec
**Created:** 2026-03-25

---

## Overview

This plan breaks the spec's 5 phases into concrete, sequentially-buildable tasks. Each task produces a compilable, testable increment. Tasks within a phase can sometimes be parallelized but are listed in recommended order.

---

## Phase 1: Foundation (MVP)

**Goal:** A binary that can read and write the macOS pasteboard, with the data model in place.

### 1.1 Project Scaffold

- [ ] `cargo init --name clipli` in this directory
- [ ] Set up `Cargo.toml` with all Phase 1 dependencies:
  - `clap` (derive), `serde` + `serde_json`, `chrono`, `thiserror`, `dirs`
  - `objc2`, `objc2-foundation`, `objc2-app-kit`
- [ ] Add dev-dependencies: `assert_cmd`, `predicates`, `tempfile`, `insta`
- [ ] Create module files: `main.rs`, `model.rs`, `pb.rs` (empty stubs for `clean.rs`, `render.rs`, `templatize.rs`, `store.rs`)
- [ ] Verify `cargo build` succeeds on macOS

**Acceptance:** `cargo build` produces a binary. `cargo test` runs (0 tests).

### 1.2 Data Model (`model.rs`)

- [ ] Implement `PbType` enum with `uti()` method and serde derives
- [ ] Implement `PbSnapshot`, `PbTypeEntry`
- [ ] Implement `TemplateMeta`, `TemplateVariable`, `VarType`
- [ ] Implement `TableInput`, `Cell`, `CellStyle`, `Align`, `BorderStyle`, `TableStyle`
- [ ] Add `From<&str>` for `PbType` (UTI string to enum)
- [ ] Unit tests: round-trip serde for each type (serialize → deserialize, assert equality)

**Acceptance:** `cargo test model` passes. All types serialize/deserialize correctly.

### 1.3 Pasteboard FFI (`pb.rs`)

- [ ] Implement `read_all() -> Result<PbSnapshot, PbError>` using `objc2-app-kit`
  - Read `NSPasteboard.generalPasteboard`
  - Iterate available types, map UTI strings to `PbType`
  - Read data bytes for each type
  - Capture timestamp and attempt source app detection
- [ ] Implement `write(entries: &[(PbType, &[u8])]) -> Result<(), PbError>`
  - `clearContents()` then `setData:forType:` for each entry
- [ ] Implement `read_type(pb_type: PbType) -> Result<Vec<u8>, PbError>` convenience
- [ ] Implement `source_app() -> Option<String>` (best-effort via NSWorkspace)
- [ ] Define `PbError` enum with thiserror
- [ ] Write tests marked `#[ignore]` (require GUI session):
  - Write plain text → read back → assert equal
  - Write HTML → read back → assert equal
  - Read empty pasteboard → assert `PbError::Empty` or graceful empty snapshot

**Acceptance:** `cargo test -- --ignored` passes when run in a macOS GUI session. FFI calls don't segfault.

### 1.4 CLI Entrypoint + `inspect` Command

- [ ] Set up clap derive CLI in `main.rs` with top-level `Cli` struct
- [ ] Define subcommand enum (all commands stubbed, only `inspect` implemented)
- [ ] Implement `inspect`:
  - Calls `pb::read_all()`
  - Default output: human-readable listing of types + sizes + source app
  - `--json`: JSON output per spec
- [ ] Error handling: catch `PbError`, print human-readable or JSON error based on `--json`

**Acceptance:** `clipli inspect` shows clipboard contents. `clipli inspect --json` outputs valid JSON.

### 1.5 `read` and `write` Commands

- [ ] Implement `clipli read`:
  - `--type` flag (html, rtf, plain, png, pdf) defaulting to html
  - Output to stdout (text types) or `--output` file (binary types)
  - `--clean` flag (stubbed — just passes through for now, wired in Phase 2)
- [ ] Implement `clipli write`:
  - `--type` flag (html, rtf, plain) defaulting to html
  - Read from `--input` file or stdin
  - `--with-plain` flag: auto-generate plain text fallback (basic strip-tags for now)
- [ ] Integration tests:
  - `clipli write --type plain` with piped input → `clipli read --type plain` → assert match
  - `clipli read` with empty clipboard → proper error

**Acceptance:** Can round-trip text through the clipboard via CLI.

### 1.6 Test Fixtures

- [ ] Create `tests/fixtures/` directory
- [ ] Capture and save real HTML from:
  - Excel (basic table, formatted table)
  - PowerPoint (single slide, two slides)
  - Google Sheets (simple table)
- [ ] At minimum, create 3 representative fixtures to unblock Phase 2
- [ ] Document capture method in `tests/fixtures/README.md` so fixtures can be reproduced

**Acceptance:** Fixture files exist and contain real pasteboard HTML.

---

## Phase 2: Capture & Clean

**Goal:** Captured clipboard HTML is sanitized and stored as reusable templates.

### 2.1 HTML Cleaner (`clean.rs`) — Core Pipeline

- [ ] Add `lol_html` dependency
- [ ] Implement `CleanOptions` and `CleanError`
- [ ] Implement pipeline stages using `lol_html` rewriter:
  1. Encoding detection/normalization (UTF-8, UTF-16, Windows-1252)
  2. Strip `<meta>`, `<link>`, `<style>`, `<xml>`, conditional comments
  3. Strip `mso-*` CSS properties from inline styles
  4. Normalize font aliases (`+mj-lt` → `Calibri`, etc.)
  5. Collapse empty `<span>`, `<p>`, `<div>` elements
  6. Normalize colors (`rgb()` → hex, `windowtext` → `#000000`)
  7. Strip `class` attributes (unless `--keep-classes`)
  8. Strip `id` attributes (except internal link targets)
  9. Collapse whitespace in text nodes
  10. Validate well-formedness
- [ ] Implement `TargetApp` enum and per-target CSS property allowlists (from spec table)
- [ ] Helper: `normalize_css(style: &str) -> String`

**Note:** Stages 1-3 are highest priority (they handle the worst Office cruft). Stages 4-10 can be iterated on. Build the framework first, then add handlers.

**Acceptance:** `clean("fixture_html", &opts)` produces sane output for all fixtures. Insta snapshot tests pass.

### 2.2 Cleaner Tests

- [ ] Snapshot tests (insta) for each fixture: raw → cleaned
- [ ] Unit test: `mso-*` properties are stripped
- [ ] Unit test: `rgb()` → hex conversion
- [ ] Unit test: empty elements collapsed
- [ ] Unit test: `--keep-classes` preserves class attributes
- [ ] Unit test: target-app CSS filtering (e.g., `border` stripped for PowerPoint target)
- [ ] Edge case: empty input, input with no HTML tags, malformed HTML

**Acceptance:** `cargo test clean` passes. Snapshots reviewed and approved.

### 2.3 Template Store (`store.rs`)

- [ ] Implement `Store` struct with configurable root (default `~/.config/clipli/templates/`)
- [ ] Implement `Store::new()` — create directory structure if missing
- [ ] Implement `save()` — write template.html.j2 (or .html), meta.json, schema.json, original.html, raw.html
- [ ] Implement `load()` — read back all files, return `LoadedTemplate`
- [ ] Implement `list()` — scan directories, read meta.json, apply optional tag filter
- [ ] Implement `delete()` — remove template directory
- [ ] Implement `exists()` and `template_path()`
- [ ] Define `StoreError` with codes from spec (STORE_NOT_FOUND, STORE_ALREADY_EXISTS, STORE_IO_ERROR)
- [ ] All tests use `tempfile::TempDir` — never touch real `~/.config`

**Acceptance:** `cargo test store` passes. CRUD round-trip works in temp directories.

### 2.4 `capture` Command (Raw + Manual)

- [ ] Implement `clipli capture --name <NAME>`:
  - Read HTML from pasteboard (fallback: RTF → plain text)
  - Run through `clean::clean()` (unless `--raw`)
  - Save via `store::save()`
  - Support `--force` to overwrite
  - Support `--description`, `--tags`
  - `--json` output with capture result
  - `--preview` opens cleaned HTML in browser (`open` command)
- [ ] Wire `--strategy manual` (save as .html, not .html.j2)
- [ ] Wire `--keep-classes` through to clean options
- [ ] `--templatize` flag accepted but only `manual` strategy works (heuristic/agent in Phase 4)

**Acceptance:** `clipli capture -n test_template` saves a template. `clipli list` shows it.

### 2.5 `list`, `show`, `delete` Commands

- [ ] Implement `clipli list` with `--tag`, `--json`, `--verbose` flags
- [ ] Implement `clipli show <NAME>` with `--html`, `--schema`, `--meta`, `--open` flags
- [ ] Implement `clipli delete <NAME>` with `--force` flag (confirm prompt without --force)
- [ ] Integration tests using tempdir store

**Acceptance:** Full template lifecycle: capture → list → show → delete.

---

## Phase 3: Templates & Rendering

**Goal:** Templates can be filled with data and pasted back to the clipboard with formatting preserved.

### 3.1 Template Renderer (`render.rs`)

- [ ] Add `minijinja` dependency
- [ ] Implement `Renderer::new(template_dir)`:
  - Load built-in templates via `include_str!`
  - Scan store directory for user templates
- [ ] Implement `Renderer::render(name, data) -> Result<RenderedOutput, RenderError>`
- [ ] Implement `html_to_plain_text()`:
  - Tables → tab-delimited
  - `<br>`/`<p>` → newlines
  - `<li>` → `- ` prefix
  - Strip all tags, decode entities
- [ ] Define `RenderError` with codes from spec

**Acceptance:** Can render built-in table template with sample data. Output is valid HTML.

### 3.2 Custom Filters

- [ ] `currency` filter — format number as `$X,XXX` (locale-aware stretch goal)
- [ ] `pct` filter — format float as `X.X%` with configurable decimal places
- [ ] `date_fmt` filter — parse date string, reformat with strftime pattern
- [ ] `number_fmt` filter — comma-separated thousands
- [ ] `default_font` filter — fallback font name
- [ ] Unit tests for each filter with edge cases (negative numbers, zero, large values, invalid input)

**Acceptance:** `{{ 4200000 | currency }}` renders `$4,200,000`. All filter tests pass.

### 3.3 Built-in Templates

- [ ] Create `templates/` directory
- [ ] Implement `_base.html.j2` per spec
- [ ] Implement `table_default.html.j2` per spec
- [ ] Implement `table_striped.html.j2` per spec
- [ ] Implement `slide_default.html.j2` (basic slide layout — single content block)
- [ ] Render tests: each template with sample data → insta snapshots
- [ ] Verify rendered HTML pastes correctly into Excel/Sheets (manual validation)

**Acceptance:** Built-in templates render correctly. Snapshot tests pass.

### 3.4 `paste` Command

- [ ] Implement `clipli paste <NAME>`:
  - Load template from store
  - Merge data: `--data` (inline JSON) > `--data-file` > stdin
  - Render via `Renderer`
  - Write HTML + plain text to pasteboard
  - `--dry-run` prints to stdout instead
  - `--plain-text` strategy: auto, tab-delimited, none
  - `--open` opens in browser
- [ ] Error handling: missing template, missing variables, invalid JSON data
- [ ] Integration test: save a template, paste with data, read back from clipboard

**Acceptance:** `clipli paste my_template -D '{"title":"Q4"}'` puts formatted HTML on clipboard.

### 3.5 `paste --from-table`

- [ ] Implement `--from-table` mode:
  - Read `TableInput` JSON from stdin
  - Select built-in template via `--template` flag (default: `table_default`)
  - Render and write to pasteboard
- [ ] Test: pipe TableInput JSON → paste → verify clipboard HTML has correct rows/cells

**Acceptance:** Can generate a formatted table from JSON and paste it.

### 3.6 `edit` Command

- [ ] Implement `clipli edit <NAME>`:
  - Open template file in `$EDITOR`
  - On save: validate Jinja2 syntax (attempt parse with minijinja)
  - Detect new `{{ variables }}` not in schema
  - `--auto-schema`: auto-add detected variables
  - Without `--auto-schema`: print warning listing new variables
  - Update `updated_at` in meta.json

**Acceptance:** Can edit a template, add a variable, and see it detected.

### 3.7 Configuration

- [ ] Add `toml` dependency
- [ ] Implement config loading from `~/.config/clipli/config.toml`
- [ ] Support all config keys from spec section 7
- [ ] Defaults when no config file exists
- [ ] CLI flags override config values
- [ ] Create default config on first run (or document that it's optional)

**Acceptance:** Config values affect behavior (e.g., `default_strategy`, `keep_classes`).

---

## Phase 4: Templatization

**Goal:** Clipboard content can be automatically analyzed and converted into templates with named variables.

### 4.1 Heuristic Templatizer (`templatize.rs`)

- [ ] Add `regex` dependency
- [ ] Implement multi-pass scanner on HTML text content:
  - Pass 1: Dates (`\b\d{1,2}[/-]\d{1,2}[/-]\d{2,4}\b`, month names, ISO) → `date_N`
  - Pass 2: Currency (`\$[\d,]+\.?\d*`, `€`, `£`) → `currency_N`
  - Pass 3: Percentages (`\d+\.?\d*%`) → `pct_N`
  - Pass 4: Emails → `email_N`
  - Pass 5: Large numbers (`\b\d{1,3}(,\d{3})+\b`) → `number_N`
  - Pass 6: Quarters (`Q[1-4]\s*\d{4}`) → `quarter_N`
  - Pass 7: Remaining `<td>`/`<th>` text content > 2 chars → `field_N`
- [ ] Each pass outputs `TemplateVariable` with inferred `VarType` and original value as `default_value`
- [ ] Replace matched text in HTML with `{{ var_name }}` (careful not to break HTML tags/attributes)
- [ ] Return modified HTML + variable list

**Key risk:** Replacing text inside HTML without breaking structure. Use a proper HTML parser to identify text nodes, not raw string replacement.

**Acceptance:** Heuristic templatizer on fixture HTML produces valid Jinja2 templates. Re-rendering with default values reproduces original content.

### 4.2 Templatizer Tests

- [ ] Test each detection pass in isolation with targeted inputs
- [ ] Test combined passes on fixture HTML
- [ ] Round-trip test: templatize → render with defaults → compare to original (should be identical)
- [ ] Edge cases: overlapping patterns (e.g., `$1,234` matches both currency and large number — currency should win), values inside HTML attributes, empty tables

**Acceptance:** All templatizer tests pass. Round-trip fidelity verified.

### 4.3 Wire Heuristic Strategy into `capture`

- [ ] `clipli capture --templatize --strategy heuristic`:
  - After cleaning, run through `templatize::heuristic()`
  - Save as `.html.j2` with `schema.json`
  - `--json` output includes detected variables
- [ ] Integration test: capture fixture → verify template + schema saved correctly

**Acceptance:** `clipli capture -n report --templatize` produces a template with variables.

### 4.4 Agent Strategy Protocol

- [ ] Implement `--strategy agent` flow:
  - After cleaning, emit JSON payload to stdout (per spec agent protocol)
  - Read JSON response from stdin
  - Validate response: valid Jinja2, valid identifiers, HTML structure preserved
  - Save validated template + variables
- [ ] Define validation checks:
  - Parse template with minijinja (syntax check)
  - All variable names are valid Python/Jinja identifiers
  - Template HTML has same top-level structure as input (same number of `<table>`, `<tr>`, etc.)
- [ ] Error handling: timeout, invalid JSON, failed validation → clear error messages

**Acceptance:** Can pipe agent protocol JSON through an external command and save the result.

### 4.5 `convert` Command

- [ ] Implement `clipli convert --from <FMT> --to <FMT>`:
  - `html` → `j2`: run heuristic or agent templatizer on input HTML
  - `j2` → `html`: render with `--data`
  - `rtf` → `html`: basic RTF to HTML conversion (may need additional dependency or simple custom parser)
  - `html` → `plain`: strip tags with table-aware tab-delimited handling
- [ ] Support `--input`/`--output` (default stdin/stdout)
- [ ] Tests for each conversion path

**Acceptance:** All 4 conversion paths work. `convert` is composable in pipelines.

---

## Phase 5: Polish

**Goal:** Production-quality CLI experience, packaging, and distribution.

### 5.1 Error Output & UX

- [ ] Colored stderr output for human-readable errors (consider `anstream` or `owo-colors`)
- [ ] Actionable suggestions in error messages (e.g., "Template not found. Run `clipli list` to see available templates.")
- [ ] Consistent JSON error envelope: `{"error": "message", "code": "ERROR_CODE"}` across all commands
- [ ] Progress indicators for long operations (e.g., agent strategy waiting for response)

### 5.2 Shell Completions

- [ ] Add `clap_complete` dependency
- [ ] Generate completions for bash, zsh, fish
- [ ] Subcommand to emit completions: `clipli completions --shell zsh`
- [ ] Template name completion (dynamic, reads from store)

### 5.3 Help & Documentation

- [ ] Polish `--help` text for every subcommand (examples, clear descriptions)
- [ ] `clipli --version` shows version + build info
- [ ] Write README.md with quick-start, examples, agent integration patterns

### 5.4 CI/CD

- [ ] GitHub Actions workflow: build + test on macOS (ARM + Intel)
- [ ] Separate job for `--ignored` pasteboard tests (needs GUI session — may need workaround)
- [ ] Clippy + rustfmt checks
- [ ] Release workflow: build universal binary, create GitHub release

### 5.5 Packaging

- [ ] Homebrew formula
- [ ] `cargo install` support (publish to crates.io or document install-from-git)
- [ ] Universal macOS binary (fat binary for arm64 + x86_64)

---

## Dependency Graph

```
Phase 1: model → pb → CLI scaffold → inspect → read/write → fixtures
Phase 2: clean → store → capture → list/show/delete
Phase 3: render → filters → templates → paste → paste --from-table → edit → config
Phase 4: templatize heuristic → wire into capture → agent protocol → convert
Phase 5: UX → completions → docs → CI → packaging
```

Each phase builds on the previous. Within a phase, the order above reflects real dependencies — later tasks need earlier ones to compile/function.

---

## Key Technical Risks

| Risk | Mitigation |
|------|------------|
| **objc2 API instability** | Pin exact versions. The objc2 ecosystem is pre-1.0 — expect breaking changes. Isolate all FFI in `pb.rs` behind a stable internal API. |
| **HTML cleaning fidelity** | Invest heavily in fixture-based snapshot tests. Real Office HTML is wildly inconsistent across versions. Collect fixtures from multiple Office versions early. |
| **lol_html limitations** | lol_html is a streaming rewriter — it can't do tree operations (e.g., "remove this element if all children are empty"). Some clean stages may need a second pass or a tree-based library like `scraper` for specific checks. |
| **Pasteboard format fidelity** | The HTML that Excel/PPT puts on the clipboard is not the same as what they accept on paste. Test the full round-trip (paste output back into the source app) early and often. |
| **Templatizer text replacement** | Replacing text in HTML without breaking tags is fragile. Must operate on text nodes only, not raw string search-replace. Use lol_html's text content handlers or a DOM parser. |
| **Agent protocol design** | The stdin/stdout JSON protocol must be validated thoroughly. Agents may return malformed or subtly wrong templates. Build strong validation in 4.4 before trusting agent output. |

---

## Definition of Done (per phase)

- All `cargo test` pass (excluding `#[ignore]` pasteboard tests)
- No `cargo clippy` warnings
- `cargo fmt` clean
- Implemented commands work end-to-end from the CLI
- Snapshot tests reviewed and approved for any HTML output changes
