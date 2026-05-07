# xli Development Plan

> Status: living plan, v0.1 (initial draft, 2026-05-05).
> Authoritative spec: [`xli-spec.md`](./xli-spec.md).
> Operator quickstart: [`README.md`](./README.md).
> Agent guide (just landed in the platform-spec branch): [`SKILL.md`](./SKILL.md).
> Platform contract: [`../../docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md).
> Cross-tool error registry: [`../../docs/error-registry.md`](../../docs/error-registry.md).

This file plans the next phase of `xli` work. The core CLI is real and
working; the gap between today and a "comprehensively developed" tool
sits in three places: blocked tests, envelope/error harmonization, and
finishing the native OOXML mutation path that lets the
`umya-spreadsheet` fallback warning go away for chart-bearing
workbooks.

---

## 1. Current State (audited 2026-05-05)

### 1.1 Workspace shape

Cargo workspace under `tools/xli/Cargo.toml` with 9 member crates:

| Crate | Role |
|---|---|
| `xli-cli` | clap dispatch, output formatting, exit codes |
| `xli-core` | Addressing, envelope, error taxonomy, ops, style |
| `xli-fs` | Commit pipeline, fingerprint compare-and-swap, file locking, staging |
| `xli-ooxml` | Native OOXML editor, package, shared strings |
| `xli-new` | Workbook creation (blank, CSV, Markdown, JSON) |
| `xli-read` | `inspect`, `read` |
| `xli-calc` | LibreOffice subprocess for recalc |
| `xli-kb` | Template KB / built-in templates |
| `xli-schema` | JSON-Schema discovery |

`cargo check --workspace` passes cleanly.

### 1.2 Test status — **blocked**

`cargo test --workspace` fails to compile. Stray Finder duplicates
have been picked up by Cargo as test files and crate names with
spaces, which Cargo rejects:

```
error: invalid character ' ' in crate name: `atomic_safety 2`
error: invalid character ' ' in crate name: `envelope_contract 2`
error: invalid character ' ' in crate name: `csv_import 2`
```

Twelve files with " 2" / " 3" suffixes exist in tree, eight in
`xli-cli/tests/` and four in `xli-{kb,new,read}/src/`. Until they are
removed (or excluded), the test suite cannot run. This is the **first
blocker** and the plan's prerequisite step.

### 1.3 CLI surface today

`xli --help` exposes 14 subcommands (matches the README parity matrix):

```
inspect   read     write    format
sheet     batch    apply    create
lint      recalc   validate doctor
template  schema
```

End-to-end probes against a freshly created `/tmp/demo.xlsx` confirm
all of these emit a stable JSON envelope:

| Command | Result |
|---|---|
| `create` | Creates an `.xlsx`; emits `output.sheets_created`. |
| `inspect` | Returns `fingerprint` (sha256), per-sheet metadata (rows, cols, formula_count, tables, named_ranges, merged_regions, is_chart_sheet), `defined_names`, `has_macros`. |
| `write` | Atomic commit; emits `commit_mode: "atomic"`, `fingerprint_before`/`fingerprint_after`, `stats.elapsed_ms`. **Emits the `umya-spreadsheet fallback` warning today.** |
| `read` | Returns address, value, value_type; supports cells, ranges, tables, pagination, formulas, Markdown. |
| `format` | Range formatting; number-format aliases (`currency`, `percent_int`, `datetime_iso`, …). |
| `sheet` | Add/remove/rename/copy/reorder/hide/unhide actions. |
| `batch` | NDJSON micro-ops in one atomic commit. |
| `apply` | Built-in template expansion into batch ops. |
| `lint`, `validate`, `doctor` | Quality pipeline; doctor wraps lint + validate (+ optional recalc). |
| `template list/preview/validate` | Built-in template metadata; one template ships today (`basic-table-format`). |
| `schema` | Full JSON Schema with `commands`, `results`, and a 17-variant `XliError` enum. |
| `recalc` | LibreOffice subprocess. |

### 1.4 Envelope and error shape today

```json
{
  "status": "ok",
  "command": "write",
  "input": { ... },
  "output": { ... },
  "commit_mode": "atomic",
  "fingerprint_before": "sha256:...",
  "fingerprint_after": "sha256:...",
  "needs_recalc": false,
  "stats": { "elapsed_ms": 1786, "file_size_before": 5334, "file_size_after": 6036 },
  "warnings": ["Used umya-spreadsheet fallback for workbook mutation. ..."],
  "errors": [],
  "suggested_repairs": []
}
```

Errors:

```json
{
  "status": "error",
  "command": "write",
  "errors": [
    { "code": "FINGERPRINT_MISMATCH", "expected": "sha256:...", "actual": "sha256:..." }
  ],
  "suggested_repairs": [
    { "action": "...", "suggestion": "...", "valid_range": null }
  ]
}
```

Significant deviations from the platform spec
([`AGENT_TOOLS_PLATFORM_SPEC.md` §4.4 / §4.5](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md)):

- `status: "ok" | "error" | "issues_found"` instead of boolean `ok`.
- `errors` is an array; spec recommends a single `error` object.
- No `meta` block; `command` / `commit_mode` / `needs_recalc` /
  `stats` / `fingerprint_before` / `fingerprint_after` are top-level.
- Error variants use upper-snake codes without an `E_` prefix
  (`FINGERPRINT_MISMATCH`, `CELL_REF_OUT_OF_BOUNDS`, …) and no
  `category` field.
- The README's published envelope sketch is closer to the platform
  shape than the actual output. **The README is wrong.** The Platform
  Conformance section in the spec needs to record this faithfully.

The deviation is *richer* than the platform spec target: `xli` carries
real audit data (fingerprints, stats, repair suggestions). Migration
should preserve those, not delete them — fold them into `meta`.

### 1.5 The OOXML fallback

Every mutating write currently emits:

> "Used umya-spreadsheet fallback for workbook mutation. Some
> workbook artifacts may have been modified."

This is the central engineering work the spec calls out. `xli-ooxml/`
contains `editor.rs`, `package.rs`, `shared_strings.rs` — the
scaffolding for a native edit path that preserves charts, drawings,
macros, data validation, and complex tables. Whether that path is
*wired into the commit pipeline yet* needs verification in Track A.

A single-cell write also takes ~1.7s, dominated by the fallback's
full-workbook re-serialization. Native OOXML edits should be
sub-100ms.

### 1.6 Spec parity gap (from README)

Implemented today: `inspect`, `read`, `write`, `format`, `sheet`,
`batch`, `apply` (minimal), `template` (minimal), `create`, `lint`
(MVP), `recalc`, `validate` (MVP), `doctor`, `schema`.

Deferred: `profile`, `diff`, `chart`, `table` (first-class command
family), `repair`, `ooxml unpack/pack/diff/grep`.

### 1.7 Maturity placement

Best-effort against the platform spec ladder (§15):

> **Level 6** today (atomic mutation + fingerprint, fixture tests
> exist but are blocked, SKILL.md just landed, cross-tool integration
> not yet exercised). **Target: level 8** by the end of this plan.

### 1.8 Repo cruft to clear

Listed for visibility; deletion requires explicit maintainer
approval (Track F).

```
tools/xli/xli/xli-cli/tests/atomic_safety 2.rs
tools/xli/xli/xli-cli/tests/csv_import 2.rs
tools/xli/xli/xli-cli/tests/data_types 2.rs
tools/xli/xli/xli-cli/tests/envelope_contract 2.rs
tools/xli/xli/xli-cli/tests/error_handling 2.rs
tools/xli/xli/xli-cli/tests/quality_pipeline 2.rs
tools/xli/xli/xli-cli/tests/range_reads 2.rs
tools/xli/xli/xli-cli/tests/sheet_operations 2.rs
tools/xli/xli/xli-kb/src/lib 2.rs
tools/xli/xli/xli-new/src/create 2.rs
tools/xli/xli/xli-read/src/lib 2.rs
tools/xli/xli/xli-read/src/read 2.rs
```

Most are byte-identical to their canonical sibling
(`atomic_safety.rs` and `atomic_safety 2.rs` are both 243 lines).
**`xli-new/src/create.rs` (854 lines) and `create 2.rs` (665 lines)
diverge** — the canonical file is the longer one; the duplicate is an
older snapshot. Track F will diff each before deletion to confirm.

---

## 2. Goals

`xli` should reach **platform maturity level 8** — versioned envelope,
complete and registered error taxonomy, native OOXML mutation as the
default path, durable docs, the deferred command families
(`profile`, `diff`, `chart`, `table`, `repair`, `ooxml`) all exposed,
and proven cross-tool integration with `vaultli`, `mdli`, and `vizli`.

By the end of this plan an agent should be able to:

1. Inspect any `.xlsx` and trust the structural map and fingerprint.
2. Mutate cells, ranges, formulas, and styles atomically with
   compare-and-swap **and zero `umya-spreadsheet fallback` warnings on
   chart/macro/drawing-bearing workbooks**.
3. Compose batch ops in one transaction with all-or-nothing semantics.
4. Build report tables with full column-formatting ergonomics from
   CSV / NDJSON / JSON.
5. Diff two workbooks semantically (cell, range, table, sheet level).
6. Run a workbook through a quality pipeline (`doctor`) that gates CI.
7. Compose into the analytics workflow:
   `vaultli search → xli create+format → vizli render → mdli report`.

---

## 3. Out of Scope (explicitly)

- Reimplementing what already works. Track A documents reality before
  any code change.
- Replacing `calamine` for reads or `rust_xlsxwriter` for greenfield
  generation — both are working and fit their niches.
- Cross-platform install, signed binaries, Homebrew formula. Defer
  until contract is firm.
- An MCP shim. CLI is canonical; MCP follows from envelope harmonization.
- The Python companion (`xli-companion`). Keep it parked; the Rust
  core is the primary surface. Companion work happens after the core
  is at level 8.

---

## 4. Tracks

Seven tracks, A–G. Track F (cleanup) is **blocking** because the test
suite cannot run without it. Recommended sequence: **F → A → B → C →
D → E → G**, where G is independent.

### Track F — Cleanup (5 minutes once approved) — **PREREQUISITE**

Awaits explicit maintainer approval. Required before any other track
can be honestly verified because tests cannot run today.

**Deliverables.**

1. Diff each `* 2.rs` against its canonical sibling (the 11 listed in
   §1.8 plus any newly-discovered duplicates).
2. Confirm each is either byte-identical or a genuinely older
   snapshot of the canonical file.
3. `git rm` the duplicates in one commit with a clear message.
4. Verify `cargo test --workspace` actually compiles after removal.

**Risk:** the divergent `xli-new/src/create.rs` vs `create 2.rs`
(854 vs 665 lines) — confirm the longer one is canonical and the
shorter one isn't carrying behavior the canonical file doesn't have.

### Track A — Audit & Harden (1–2 sessions)

**Goal.** With tests unblocked, build a parity matrix of every
command, every error variant, and every spec promise vs. actual
behavior.

**Deliverables.**

1. `tools/xli/PARITY_MATRIX.md` — every command from `xli-spec.md`
   on one axis; current behavior, integration-test coverage, gaps,
   known bugs on the other.
2. End-to-end probes for every subcommand against:
   - blank workbook (`create --sheets ...`)
   - chart-bearing workbook (build a fixture; this is where
     `umya-spreadsheet` fallback bites hardest)
   - table-bearing workbook
   - merged-cell / named-range / data-validation workbook
   - macro-bearing workbook (`.xlsm`)
3. Document every `XliError` variant: which commands emit it, under
   what conditions, what the recovery path is.
4. Bug fixes for any obvious failures surfaced by the probes
   (separate commits per bug).
5. Golden envelope outputs added to `xli-cli/tests/` so shape
   regressions become test failures.

**Acceptance.**

- Every subcommand has at least one verified happy-path probe.
- Parity matrix has zero "?" rows.
- `cargo test --workspace` is green and includes the new probes.

### Track B — Platform Conformance (1 session)

**Goal.** Migrate the envelope and error shape onto the platform
contract while preserving `xli`'s richer fields. Pre-v1 churn is
cheapest now.

**Deliverables.**

1. **Envelope migration.** Old:

   ```json
   { "status": "ok", "command": "...", "input": {...}, "output": {...},
     "commit_mode": "atomic", "fingerprint_before": "...",
     "fingerprint_after": "...", "needs_recalc": false,
     "stats": {...}, "warnings": [], "errors": [], "suggested_repairs": [] }
   ```

   New:

   ```json
   {
     "ok": true,
     "command": "write",
     "result": { /* old `output` */ },
     "meta": {
       "tool": "xli.write",
       "version": "0.1.0",
       "duration_ms": 1786,
       "dry_run": false,
       "commit_mode": "atomic",
       "fingerprint_before": "sha256:...",
       "fingerprint_after": "sha256:...",
       "needs_recalc": false,
       "stats": { "file_size_before": 5334, "file_size_after": 6036 },
       "warnings": [...],
       "input": { /* old `input`, kept for audit */ },
       "suggested_repairs": [...]
     }
   }
   ```

   `status: "issues_found"` collapses into `ok: true` with non-empty
   `meta.warnings`; `status: "error"` collapses into `ok: false` plus
   `error: { ... }`.

2. **Error rename.** Single `error` object instead of an `errors`
   array. Add `E_` prefix and `category` to every variant:

   | Today | After |
   |---|---|
   | `FILE_NOT_FOUND` | `E_FILE_NOT_FOUND` (input) |
   | `CLI_PARSE_ERROR` | `E_CLI_PARSE` (input) |
   | `LOCK_CONFLICT` | `E_LOCK_CONFLICT` (state) |
   | `SHEET_NOT_FOUND` | `E_SELECTOR_NOT_FOUND` (state, with `details.kind="sheet"`) |
   | `CELL_REF_OUT_OF_BOUNDS` | `E_SELECTOR_OUT_OF_BOUNDS` (state) |
   | `INVALID_CELL_ADDRESS` | `E_INVALID_SELECTOR` (input) |
   | `FORMULA_PARSE_ERROR` | `E_FORMULA_PARSE` (input) |
   | `FINGERPRINT_MISMATCH` | `E_STALE_PREIMAGE` (state) |
   | `TEMPLATE_NOT_FOUND` | `E_TEMPLATE_NOT_FOUND` (state) |
   | `TEMPLATE_PARAM_MISSING` | `E_TEMPLATE_PARAM_MISSING` (input) |
   | `TEMPLATE_PARAM_INVALID` | `E_TEMPLATE_PARAM_INVALID` (input) |
   | `RECALC_TIMEOUT` | `E_TIMEOUT` (runtime) |
   | `RECALC_FAILED` | `E_RECALC_FAILED` (runtime) |
   | `WRITE_CONFLICT` | `E_CONFLICT` (state) |
   | `SPEC_VALIDATION_ERROR` | `E_SCHEMA_MISMATCH` (input) |
   | `BATCH_PARTIAL_FAILURE` | `E_BATCH_PARTIAL` (state) |
   | `OOXML_CORRUPT` | `E_ARTIFACT_CORRUPT` (state) |

   Plus add `E_PARTIAL_FIDELITY` (runtime, `is_retryable: false`) so
   the `umya-spreadsheet` fallback warning becomes a structured code
   instead of free-text. (The warning array still carries the human
   message.)

3. **`docs/error-registry.md`** updates: replace the "today emitted as
   warnings by `xli`" stub with the full per-variant table.

4. **Suggestion object.** Map existing `suggested_repairs` entries
   into the platform's `error.suggestion` form
   (`{action, fix, example}`), one per error.

5. **Global flags.** Audit the clap dispatcher and add (or align):
   `--dry-run`, `--quiet`, `--verbose`, `--no-color`,
   `--idempotency-key`, `--help-agent`, `--agent-manifest`. Many
   exist via clap defaults; this is gap-fill.

6. **`--agent-manifest`.** Emit a single JSON discovery document
   matching the platform spec §11 shape. `xli-schema` already has
   most of the data — the manifest just composes commands +
   capabilities + errors + examples.

7. **README envelope correction.** The published envelope sketch is
   wrong; replace it with the post-Track-B form and a pointer to the
   spec.

8. **`xli-spec.md` Platform Conformance** section: add it (currently
   missing — only the README has the brief deviation note).

**Acceptance.**

- Every JSON output uses the new envelope.
- Every error has `code`, `category`, `message`, `details`,
  `suggestion`, `is_retryable`.
- `xli schema` reflects the new shapes (auto, since it's
  `schemars`-derived).
- All Track A probes still pass.

### Track C — Native OOXML Mutation Path (3–4 sessions)

**Goal.** Make the `umya-spreadsheet fallback` warning rare instead of
universal. This is the spec's biggest engineering investment and
`xli`'s key correctness story for chart/macro/drawing-bearing
workbooks.

**Deliverables.**

1. **Audit `xli-ooxml/`.** Confirm what `editor.rs`, `package.rs`,
   `shared_strings.rs` already implement vs. what `xli-fs/commit.rs`
   actually wires up. Probably wired only for narrow cases today.

2. **Native edit paths**, one per supported op type:
   - Cell value writes (string, number, boolean, date, error).
   - Cell formula writes.
   - Range formatting (fill, font color, bold/italic, number format,
     alignment, column width).
   - Sheet add/remove/rename/copy/reorder/hide/unhide.
   - Defined-name add/update/remove.

3. **Selector through `xli-fs/commit.rs`:**
   - Try native OOXML path.
   - On unsupported feature (chart geometry edit, macro module, …),
     fall through to `umya-spreadsheet` and emit
     `E_PARTIAL_FIDELITY` with `details.unsupported: [...]`.
   - On native path success, emit no fallback warning.

4. **Shared strings handling.** `shared_strings.rs` is a known
   correctness hot spot. Cover:
   - inline string vs shared-string round trip
   - shared-string dedup on append
   - shared-string GC on remove (or a documented "we don't GC" choice)

5. **Performance target.** Single-cell writes drop from ~1.7s
   (fallback) to <100ms (native) on typical workbooks. Add a
   `tests/perf.rs` golden test that asserts envelope `duration_ms`
   bounds.

6. **Fidelity test corpus.** Ship fixtures under `tools/xli/tests/fixtures/`:
   - `chart_bearing.xlsx` — pie + bar chart sheet.
   - `macro_bearing.xlsm` — VBA module.
   - `drawing_bearing.xlsx` — embedded image + shape.
   - `validation_bearing.xlsx` — data validation rules.
   - `complex_table.xlsx` — `Table1` over a range with totals row.
   - `merged_named.xlsx` — merged regions + named ranges + defined names.

   For each: assert that a native edit on an unrelated cell does
   **not** disturb the artifact (unzip and diff the OOXML parts).

**Acceptance.**

- Native path covers cell writes, formula writes, range formatting,
  and sheet ops.
- The fallback warning becomes the exception, not the rule, for the
  fixture corpus.
- A `tests/native_ooxml_fidelity.rs` integration suite covers each
  fixture and asserts no unrelated parts changed (byte diff on the
  unzipped OOXML).
- Single-cell write performance budget met.

### Track D — Deferred Command Families (3–4 sessions)

**Goal.** Close the spec parity gap: `profile`, `diff`, `chart`,
`table`, `repair`, `ooxml`. Pick the most-valuable ones first; ship
the others when justified.

**Sequence.**

1. **`xli table`** (1 session) — first-class create/edit/list of
   Excel Tables. Today `read --table` works; promote tables to a
   write-side first-class concept. Backed by `xli-ooxml/` to avoid
   the fallback.

2. **`xli diff`** (1 session) — semantic diff between two workbooks.
   Cell-level, range-level, sheet-level, and structural deltas
   (sheet added/removed/renamed, named range changes, table column
   changes). NDJSON findings plus `summary` block (parity with
   `mdli diff`).

3. **`xli chart`** (1 session) — read chart metadata, add basic
   charts (bar, line, pie) with named-range data sources. Native
   OOXML path required; this is the second hardest job after Track
   C.

4. **`xli repair`** (½ session) — run `validate`, then auto-fix the
   issues `xli` knows how to repair (hex-color normalization,
   whitespace, redundant defined names, etc.). Idempotent.

5. **`xli ooxml unpack/pack/diff/grep`** (½ session) — low-level
   package helpers. Most of the work is already inside `xli-ooxml/`;
   this just exposes a CLI surface so agents can debug.

6. **`xli profile`** (½ session) — workbook profiling (size, formula
   complexity, cross-sheet dependencies, per-sheet hot spots). Pure
   read; cheap.

**Acceptance.**

- Each new command has at least one happy-path integration test.
- README parity matrix marks each as "Implemented" with notes.

### Track E — Report Table Ergonomics (1–2 sessions)

**Goal.** Make `xli` the canonical analytics-report surface. Today
`create --from-csv` already does column selection, renames, hidden
columns, currency aliases, and totals row — push it further.

**Deliverables.**

1. **NDJSON-rows input.** `xli create --from-rows /tmp/rows.ndjson`
   for direct agent piping (parity with `mdli`).
2. **Per-column rich format.** `--col Revenue:currency:right:bold` —
   document the grammar, support fill/font color/min-width.
3. **Frozen panes / split.** `--freeze 1` keeps headers visible.
4. **Conditional formatting helpers.** `--heat Revenue` / `--bar
   Revenue` for inline data bars and color scales (Excel-compatible).
5. **Auto-fit column widths.** `--autofit`.
6. **Total/grand-total rows.** Already exists; add `--subtotal-by
   Region` for grouped totals.
7. **Built-in templates.** Promote `basic-table-format` to a small
   library: `striped-table`, `kpi-card`, `summary-block`,
   `comparison-table`. Each documented in `xli-kb`.

**Acceptance.**

- A single `xli create --from-rows ... --col ... --col ... --autofit
  --freeze 1` produces a publication-grade report with no follow-up
  formatting.
- New templates are listed by `xli template list` and previewable.

### Track G — SKILL.md, Cross-Tool Loop, and Roadmap (1 session)

**Goal.** Make `xli` composable inside agent workflows and update
platform-level docs.

**Deliverables.**

1. **`tools/xli/SKILL.md` audit.** A SKILL.md was just written in the
   platform-spec branch. After Tracks A–E land, audit it for accuracy
   (envelope shape, command set, error codes, performance
   expectations). Replace stale sections.

2. **Cross-tool recipe.** A `tools/xli/RECIPES.md` documenting:
   - `vaultli show queries/X.sql → run query → NDJSON → xli create`
   - `xli read --table T --format markdown → mdli table replace`
   - `xli inspect → vizli render --data ...` (chart from workbook
     data)

3. **Update `docs/skills.md`** to list `xli` in the Active skills
   inventory.

4. **Update `docs/tool-roadmap.md`** maturity table: `xli` from 6 to
   8, after all acceptance criteria are met.

5. **Update the platform spec**'s tool-family taxonomy
   (`AGENT_TOOLS_PLATFORM_SPEC.md` §8) with the new maturity number
   and remove `xli` from the `E_PARTIAL_FIDELITY` "today emitted as
   warnings" caveat.

**Acceptance.**

- `xli` appears in the Active skills inventory.
- The cross-tool recipe runs end-to-end on a clean checkout.
- An external agent reading just the SKILL.md can complete one full
  loop without falling back to `--help`.

---

## 5. Sequencing & Risk

| Step | Track | Why this order | Risk |
|---|---|---|---|
| 1 | **F** (BLOCKING) | Tests cannot run until the duplicates are removed; nothing else can be verified honestly. | None once approved. |
| 2 | A | We need a parity matrix before we change the envelope or refactor the commit pipeline. | Low. Investigation + small fixes. |
| 3 | B | Cheapest moment to migrate the envelope. Doing C/D on the old envelope means doing it twice. | Low–Medium. Mechanical; the schema-derived JSON Schema absorbs most churn automatically. |
| 4 | C | The biggest correctness payoff and the platform's reference for partial-fidelity handling. Builds on B's `E_PARTIAL_FIDELITY` plumbing. | High. OOXML is unforgiving; mitigated by the fixture corpus and reopen-and-validate gate. |
| 5 | D | Surface completeness. Some pieces (`chart`) require Track C; others (`profile`, `diff`) don't and could move earlier if Track C stalls. | Medium. `chart` is the hardest. |
| 6 | E | Polish and ergonomics. Worth doing once the surface is broad. | Low. |
| 7 | G | Final docs and roadmap. | Low. |

If schedule pressure forces a cut: **F + A + B + C** alone deliver
the highest-value chunk — blocked tests unblocked, behavior
documented, contract on the platform, and the OOXML fallback
mostly retired. D, E, G are increments on top.

---

## 6. Acceptance for the Whole Plan

`xli` is "comprehensively developed" when:

- [ ] Test suite compiles and passes (`cargo test --workspace`).
- [ ] Parity matrix exists; every spec command has a row.
- [ ] Envelope and error shape match the platform contract; codes
      registered in `docs/error-registry.md`.
- [ ] Native OOXML mutation path is the default for cell writes,
      formula writes, range formatting, and sheet ops; fallback is
      structured (`E_PARTIAL_FIDELITY`) and rare.
- [ ] `chart`, `table`, `diff`, `profile`, `repair`, `ooxml` command
      families are present (at least at MVP).
- [ ] Report-table ergonomics cover NDJSON input, per-column rich
      format, frozen panes, autofit, and a small built-in template
      library.
- [ ] SKILL.md is accurate post-migration.
- [ ] One cross-tool recipe (`vaultli + xli + vizli + mdli`) runs
      end-to-end.
- [ ] `docs/tool-roadmap.md` and the platform spec list `xli` at
      level 8.

---

## 7. What This Plan Is Not

- A spec replacement. `xli-spec.md` remains the design document.
- A schedule. Sessions are rough effort estimates.
- An invitation to skip Track F. Without cleanup, every other
  track's "tests pass" claim is unverifiable.
- A claim that every spec command graduates to level 8 in this
  round. `merge`, advanced `convert`, advanced `chart` editing may
  remain at MVP after this plan.

---

## 8. Open Questions

1. **Default-to-plan vs. `--dry-run`?** Today every mutating command
   accepts `--dry-run` and defaults to writing. The platform spec
   recommends "default to plan" for high-risk artifacts; `mdli` and
   `notionli` follow that. **Recommendation: keep `--dry-run`** —
   workbooks with fingerprint compare-and-swap are lower-risk than
   Markdown documents with managed-block checksums, and the friction
   of default-to-plan would hurt the analytics workflow. Confirm.

2. **Where does the audit log live?** `xli` does not yet keep one.
   Recommendation: optional `~/.local/share/xli/audit.ndjson` with
   `--audit-path` override, populated only when
   `XLI_AUDIT_ENABLED=1`. Track G work, not Track C.

3. **Should `xli` adopt `kb://` URIs for templates?** `xli-kb`
   currently uses internal names (`basic-table-format`). The
   platform's open question (§16, item 9) about `vaultli` as the
   knowledge substrate applies here. Recommendation: defer until
   `vaultli`/`docli`/`vizli` agree on the URI scheme.

4. **Numeric vs. string error families.** This plan uses string
   `E_*` codes (matching `mdli`/`framerli`/`notionli`/`docli`-plan)
   plus `category`. Confirm `xli` does **not** need numeric
   `E1xxx`–`E5xxx` codes.

5. **`xli-companion`.** Out of scope for this plan. Confirm it stays
   parked.

6. **`umya-spreadsheet` future.** Once the native path covers >90% of
   real-world workbooks, do we drop the dependency entirely? Or keep
   it as the fallback story for unfamiliar OOXML shapes? Recommendation:
   keep as fallback, escalate `E_PARTIAL_FIDELITY` to an opt-in
   strict-mode hard error (`--strict`).

7. **Locking model.** `xli-fs/lock.rs` exists. Document the lock
   semantics (advisory? per-file? cross-process?) in the parity
   matrix in Track A; harmonize with `mdli`/`docli`'s lifecycle in
   Track B if appropriate.

---

## 9. Pointers

- Platform contract: [`../../docs/AGENT_TOOLS_PLATFORM_SPEC.md`](../../docs/AGENT_TOOLS_PLATFORM_SPEC.md)
- Error registry: [`../../docs/error-registry.md`](../../docs/error-registry.md)
- Skills guide: [`../../docs/skills.md`](../../docs/skills.md)
- Reference tool for the mutation safety model: [`../mdli/README.md`](../mdli/README.md)
- Reference tool for spec → MVP discipline: [`../docli/PLAN.md`](../docli/PLAN.md)
- This tool's spec: [`./xli-spec.md`](./xli-spec.md)
- This tool's README parity matrix: [`./README.md`](./README.md)
- This tool's skill: [`./SKILL.md`](./SKILL.md)
