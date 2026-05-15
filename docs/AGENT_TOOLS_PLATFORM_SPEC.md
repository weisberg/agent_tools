# Agent Tools Platform Specification

> Status: living spec, v0.1 (initial draft).
> Audience: AI coding agents, human maintainers, and technical decision-makers
> who need to understand `agent_tools` as a single platform rather than a
> collection of unrelated CLIs.
> Scope: this document defines the *contract* every `*li` tool in this repo
> should converge toward. It does not redefine any tool's existing
> behavior; existing tool specs (`tools/<name>/...`) remain authoritative for
> their specific surfaces.

This spec distinguishes carefully between four levels of certainty:

| Marker | Meaning |
|---|---|
| **Implemented** | Behavior already shipped in at least one tool, with tests. |
| **Partial** | Behavior shipped in some tools but not yet uniform across the platform. |
| **Planned** | Behavior identified in a tool spec or roadmap but not yet implemented. |
| **Recommended** | Platform standard proposed by this spec; not yet enforced. |

When in doubt, prefer "Recommended" over claiming a behavior is universal.

---

## 1. Purpose

`agent_tools` exists because AI agents are powerful general reasoners but
unreliable when forced to manipulate human-oriented business artifacts directly.
LLMs can write a `.docx` byte string, edit a Markdown table by line number, or
hand-craft an Excel formula — but they do these things slowly, expensively, and
with a meaningful failure rate. The artifacts then have to be repaired by a
human, and the agent's earlier reasoning is wasted.

The platform answers that with a different stance:

- **AI agents need deterministic tool surfaces, not brittle manual editing.**
  Every artifact operation — "ensure this section exists", "update this row by
  key", "add this slide from this layout" — should be a single command with
  validated inputs, structured outputs, and a defined failure mode.
- **Human artifacts need machine-addressable command layers.** Word documents,
  spreadsheets, decks, knowledge bases, Jira issues, Notion pages, clipboards,
  and shells were not designed to be addressed programmatically by an outside
  agent. The `*li` tools impose a stable address space (stable IDs, A1 ranges,
  slide/shape paths, vault IDs, ticket keys) on top of those artifacts.
- **CLIs are the durable base layer.** They are low-latency, scriptable,
  inspectable, composable, easy for an agent to call, easy for a human to
  rehearse, and easy to embed in CI. MCP servers, Python APIs, and HTTP
  bridges can be generated *from* the CLI surface. The CLI contract is the
  source of truth.
- **Skills teach agents *when* to call the tools.** The CLI defines the *what*
  and the *how*; skills (see `docs/skills.md`) define the *when* and bind
  agent intent to deterministic command sequences.

The platform is, in shape, a Unix toolbox for agents working in real business
artifacts: many small, sharp tools, one shared contract, predictable
composition.

---

## 2. Platform Thesis

`agent_tools` is:

- **Agent-native, not merely human-friendly.** Default output is structured.
  Default error responses are machine-recoverable. Caller detection
  (`TOOLI_CALLER`) lets a tool adapt to humans, agents, or CI without forking
  command surfaces.
- **Artifact-aware, not just file-manipulating.** Tools understand the
  structure of the artifacts they touch (sections, runs, slides, ranges,
  vault records, issues, blocks). Plain text is a fallback, never the
  primary interface.
- **JSON-first, not prose-first.** Non-interactive callers get JSON;
  interactive humans get rich text. Diagnostics never contaminate stdout in
  JSON mode.
- **Plan-first for mutation.** Mutating commands either default to a dry-run
  plan (e.g. `mdli`, `notionli`, `framerli`) or expose `--dry-run` as a
  first-class flag. An agent can always rehearse a destructive operation
  before committing to it.
- **Composable like Unix tools, but safer and more typed.** Stdin and stdout
  are NDJSON or JSON envelopes; piping `vaultli search` into `xli batch` is
  a normal workflow, not a hack.
- **Built for skills, scripts, CI, and autonomous agent loops.** Every tool
  should be usable by any one of those callers without translation.

> **Operating thesis.** Every tool should turn an ambiguous human artifact
> operation into an explicit, validated, inspectable command. If the agent
> cannot describe what will change before it changes, the tool is not yet
> agent-ready.

---

## 3. Design Principles

These are the principles every `*li` tool should follow. Existing tools
already match many of them; the gaps are this platform's harmonization
backlog.

1. **Explicit inputs.** No magic environment scraping for required values.
   Required arguments are required; optional ones have documented defaults.
2. **Structured outputs.** A tool's stdout in JSON mode is parseable without
   regex by any agent that knows the envelope. Never mix prose and data on
   stdout.
3. **Stable selectors.** Where the artifact has stable identifiers (a Jira
   key, an mdli stable ID, a Notion page ID, an A1 range, a named table, a
   vault ID), prefer them. Path/title selectors are bootstrap-only.
4. **Idempotency by default.** Re-running an `ensure`, `upsert`, or `apply`
   command against an unchanged artifact must produce zero diff.
   `--idempotency-key` is the platform-recommended escape hatch for retry
   safety on systems that lack inherent idempotency.
5. **Dry-run before mutation.** Mutating commands either default to a plan
   (preferred) or expose `--dry-run` (acceptable). Either way, the agent can
   inspect the planned change without touching the artifact.
6. **Atomic writes where the artifact allows it.** Build a shadow artifact
   and rename into place; never partially overwrite a file an agent or
   human is reading. `xli` and `docli` formalize this; `mdli` follows the
   same model with `--write` plus preimage hashes.
7. **Fingerprint / preimage protection for concurrent edits.** Mutating
   commands should accept a preimage hash and refuse the write if the
   on-disk bytes have changed since the agent inspected them.
   (`mdli --preimage-hash`, `xli` fingerprint compare-and-swap.)
8. **Predictable errors with recovery suggestions.** Every error has a
   stable code, a category, a human message, machine-readable details, a
   suggested next action, and a retryability indicator. See §5.
9. **Clean stdout in JSON mode.** Diagnostics, logs, and progress go to
   stderr. JSON parsers must never see them.
10. **Human-readable mode is secondary.** Rich text output is welcome but
    must never be the only output mode for an operation an agent might call.
11. **Small composable commands over monolithic agents.** A tool exposes
    primitives an agent can reason over (`ensure`, `replace`, `upsert`,
    `assign`, `apply-plan`), not high-level chat verbs.
12. **Source files remain the source of truth.** Generated indexes,
    sidecars, audit logs, and cache files are rebuildable. `vaultli`
    formalizes this: `INDEX.jsonl` is a derived cache, the files on disk
    are canonical.
13. **Generated regions are fenced and protected.** When a tool writes into
    a human-edited artifact, it should mark the generated region (mdli
    managed blocks, docli tracked changes, xli table ranges) so future runs
    do not silently clobber human edits.
14. **Rust for fast deterministic cores; Python where ecosystem depth
    matters.** Both are first-class. Language choice is downstream of the
    contract.
15. **No hidden magic when agents need traceability.** State changes go to
    audit logs (`framerli`/`notionli` patterns). Every mutation should be
    reconstructible from the JSON envelope plus the artifact bytes.

---

## 4. Canonical Command Contract

This section defines the *recommended* shape every tool should converge
toward. Many existing tools deviate (see §4.6). Convergence is a roadmap
item, not a rewrite mandate.

### 4.1 Naming

- Binaries are named `<domain>li` (`xli`, `mdli`, `vaultli`, ...) — short,
  agent-typeable, autocompletes well, makes the family visible in shell
  history. **Implemented.**
- Subcommands group by noun, then verb: `xli sheet add`, `mdli section
  ensure`, `notionli row upsert`. **Recommended; followed by most tools.**
- Verbs are drawn from a small shared vocabulary:

  | Verb | Meaning |
  |---|---|
  | `inspect` | Read structural metadata; never mutates. |
  | `read` / `get` / `show` | Read content by selector. |
  | `list` | Enumerate items, paginated. |
  | `ensure` | Create if missing; leave alone if present (idempotent). |
  | `replace` | Replace the addressed content unconditionally. |
  | `upsert` | Insert or update by key (idempotent on key). |
  | `delete` / `remove` / `rm` | Remove an addressed item. |
  | `apply` | Execute a recipe / plan against an artifact. |
  | `plan` | Compute the change a recipe would make, without applying. |
  | `apply-plan` | Apply a previously computed plan with preimage protection. |
  | `validate` | Check artifact against a schema or invariants. |
  | `lint` | Surface style/structural warnings without failing on minor issues. |
  | `doctor` | Check the local environment for runtime prerequisites. |
  | `schema` | Emit machine-readable command/result schemas. |
  | `batch` | Apply an NDJSON stream of micro-ops in one transaction. |

### 4.2 Global Flags

The recommended global flag set, modeled on `tooli`'s injected flags
(see `docs/tooli.md`) and existing `mdli`/`xli`/`framerli` patterns:

| Flag | Purpose | Status |
|---|---|---|
| `--json` | Force JSON envelope on stdout. | Implemented across most tools. |
| `--jsonl` | Stream NDJSON for list/long-running commands. | Partial. |
| `--output {auto,json,jsonl,text,plain}` | Explicit output mode. | Implemented in `tooli`-based tools. |
| `--dry-run` | Compute and emit the plan without mutating. | Implemented in many tools; default behavior in `mdli`/`notionli`/`framerli`. |
| `--quiet` / `-q` | Suppress non-error diagnostics. | Implemented widely. |
| `--verbose` / `-v` | Increase diagnostic verbosity. | Partial. |
| `--force` | Override safety checks where supported. | Partial. |
| `--no-color` | Disable color in human mode. | Partial. |
| `--timeout <ms>` | Bound execution time. | Recommended. |
| `--idempotency-key <key>` | Safe-retry key for non-idempotent surfaces. | Implemented in `tooli`. |
| `--schema [--command X]` | Emit JSON Schema for inputs/outputs. | Implemented in `xli`; recommended for all. |
| `--help-agent` | Compact agent-oriented help. | Implemented in `tooli`-based tools. |
| `--agent-manifest` | Emit a discovery manifest (commands, capabilities, errors, examples). | Recommended, implemented in `tooli`. |
| `--response-format {concise,detailed}` | Hint at response shape. | Recommended. |

Avoid defining command-local flags that collide with these names.

### 4.3 Stdin and Stdout Conventions

- `-` as a positional file argument means stdin. **Implemented in `mdli`.**
- NDJSON is the recommended batch input format; one JSON object per line,
  each describing one micro-op. **Implemented in `xli batch` and the
  `mdli` recipe layer.**
- Tools that emit lists in JSONL mode emit one record per line and write
  pagination metadata to stderr or to a final summary record only when
  `--summary` (or equivalent) is requested. **Recommended.**
- Errors in JSON mode are emitted as a single envelope to stdout with a
  non-zero exit code; never split error metadata between stdout and stderr.
  **Implemented in `mdli`, `tooli`-based tools.**

### 4.4 Success Envelope (recommended target)

The recommended canonical envelope:

```json
{
  "ok": true,
  "result": { },
  "meta": {
    "tool": "xli.batch",
    "version": "0.4.2",
    "duration_ms": 142,
    "dry_run": false,
    "warnings": [],
    "annotations": { "readOnlyHint": false, "idempotentHint": true },
    "truncated": false,
    "next_cursor": null
  }
}
```

Implementations may add a `schema` field at the top level (as `mdli` does
with `"schema": "mdli/output/v1"`) to version the envelope. Such a field
is recommended for any tool whose envelope shape may evolve.

### 4.5 Error Envelope (recommended target)

```json
{
  "ok": false,
  "error": {
    "code": "E3001",
    "category": "state",
    "message": "No section matched selector 'cashplus.analytics'.",
    "suggestion": {
      "action": "retry_with_modified_input",
      "fix": "Run `mdli id list report.md` to see available IDs.",
      "example": "mdli section get report.md --id cashplus.okr"
    },
    "is_retryable": true,
    "details": { "selector": "cashplus.analytics" }
  },
  "meta": {
    "tool": "mdli.section.get",
    "version": "0.5.1",
    "duration_ms": 9,
    "dry_run": false,
    "warnings": []
  }
}
```

The error code may be either a numeric `E1xxx`–`E5xxx` family code (the
`tooli` convention; see `docs/tooli.md`) or a stable string code prefixed
`E_` (the `mdli`/`framerli`/`notionli` convention). Both are acceptable
today; see §5 for the harmonization recommendation.

### 4.6 Existing Deviations (recorded, not corrected here)

This is the current envelope reality, not the target. Harmonization is a
roadmap item; rewriting envelopes mid-flight breaks downstream skills, so
this spec records deviations rather than declaring a flag day.

| Tool | Deviation from §4.4 / §4.5 |
|---|---|
| `mdli` | Wraps with top-level `schema: "mdli/output/v1"`; uses `result` + string error codes (`E_AMBIGUOUS_SELECTOR`). Mutating commands return an `ops`/`preimage_hash`/`postimage_hash` block. |
| `xli` | JSON-first envelope per `tools/xli/README.md`; emits `umya-spreadsheet` warnings inline on mutating commands. |
| `framerli` | Uses `data` instead of `result`; error fields are `hint` + `retryable` instead of `suggestion` + `is_retryable`. |
| `docli` (spec) | Uses `data` + top-level `command` and `elapsed_ms`; error code uses `INVALID_TARGET`-style strings without `E_` prefix. |
| `clipli` | Documents a minimal `{ok, error, code}` failure envelope; broader fields exist in practice. |
| `deckli` (spec) | Uses `success` instead of `ok` and a per-command `command` field; error includes `suggestion` as a string. |
| `notionli` | Implements MVP envelope with operation receipts and dry-run-by-default; converging toward §4.4. |
| `jirali` | Stable JSON envelope with exit codes 0–8 (Jira-shaped, not `E1xxx`). |

The recommendation is to add a top-level `schema` field to every tool's
envelope so that envelope versions can evolve independently per tool while
agents can detect the shape without sniffing.

---

## 5. Error Taxonomy

The platform recommends a two-layer error taxonomy: a **shared family
prefix** (so error handlers can branch generically) and an **artifact-
specific code** (so agents can recover precisely).

### 5.1 Shared Families

Modeled on `tooli`'s `E1xxx`–`E5xxx` families (see `docs/tooli.md`):

| Family | Meaning | Typical exit code |
|---|---|---:|
| `E1xxx` | Input / user error: bad flags, malformed JSON, schema violation. | 2 |
| `E2xxx` | Auth / permission: missing token, denied scope, expired credential. | 30 |
| `E3xxx` | State: not found, conflict, ambiguous selector, stale preimage. | 10 |
| `E4xxx` | Runtime / external dependency: network failure, missing binary, target app unavailable. | 70 |
| `E5xxx` | Internal / tool bug: unreachable branch, panic-recovered failure. | 70 |

Existing string-coded tools (`mdli`, `framerli`, `notionli`) should
publish a mapping from their string code to a numeric family so generic
agent handlers can branch on `category`. **Recommended.**

### 5.2 Artifact-Specific Codes

These names appear across tool specs and should converge to the same
semantics wherever they apply:

| Code (recommended) | Meaning | Today |
|---|---|---|
| `E_AMBIGUOUS_SELECTOR` | Selector matched more than one structure. | `mdli` |
| `E_SELECTOR_NOT_FOUND` | Selector matched zero structures. | `mdli` |
| `E_DUPLICATE_ID` | Stable ID present more than once. | `mdli` |
| `E_STALE_PREIMAGE` | Input bytes changed since the plan was computed. | `mdli` |
| `E_BLOCK_LOCKED` | Generated region marked locked, edit refused. | `mdli` |
| `E_BLOCK_MODIFIED` | Generated region content edited outside the tool. | `mdli` |
| `E_VALIDATION_*` | Validation findings against a declared schema. | `mdli` |
| `E_AUTH_MISSING` | No credentials configured. | `framerli`, `notionli` |
| `E_NOT_IMPLEMENTED` | Command surface exists but bridge/backend not yet wired. | `framerli` |
| `E_RATE_LIMITED` | Upstream rate limit hit; retry after `details.retry_after`. | Recommended. |
| `E_PARTIAL_FIDELITY` | Operation succeeded but artifact lost some fidelity (e.g. `umya-spreadsheet` fallback). | Recommended; surfaced today as warnings in `xli`. |
| `E_BRIDGE_DISCONNECTED` | Live-app bridge (deckli sidecar, framerli node) not connected. | Recommended. |
| `E_SCHEMA_MISMATCH` | Recipe / job / plan version not understood. | `mdli` |

Each error must carry: `code`, `category`, `message`, `details`,
`suggestion`, `is_retryable`. Exit codes follow the family table above
unless the artifact has its own established convention (e.g. `jirali`'s
0–8 codes).

---

## 6. Mutation Safety Model

This is the most important section. Every tool that mutates a real
artifact should follow the same lifecycle.

### 6.1 Lifecycle

```
inspect  →  plan  →  dry-run  →  apply  →  validate  →  diff/report
```

- **inspect** reads structural metadata so the agent knows what exists.
- **plan** computes the intended change. For declarative recipes this is
  an explicit command (`mdli plan`); for one-shot edits the plan is the
  default JSON output of the mutating command.
- **dry-run** confirms the plan against the current artifact bytes
  without writing.
- **apply** writes atomically.
- **validate** checks the post-state against a schema or invariants.
- **diff/report** explains semantically what changed. (`mdli diff` is the
  reference implementation; see `tools/mdli/README.md`.)

### 6.2 Mandatory Behaviors for Mutating Commands

Mutating commands must:

1. **Default to safe.** Either default to a dry-run plan (preferred for
   high-risk artifacts: `mdli`, `notionli`, `framerli`) or require an
   explicit `--write` / `--apply` / `--yes` flag. Never mutate by default
   without an explicit affirmative input.
2. **Emit a structured plan.** The plan describes ops, addresses, and
   preimage/postimage hashes where applicable. Agents can store, diff,
   and re-apply plans.
3. **Use atomic writes.** Build a shadow artifact in the same filesystem;
   fsync; rename. Never modify the source archive in place. The `docli`
   spec formalizes three durability modes (`fast`/`durable`/`paranoid`);
   `xli` uses atomic commits + fingerprint compare-and-swap.
4. **Refuse on stale preimage.** If `--preimage-hash` is supplied and
   the on-disk bytes do not match, return `E_STALE_PREIMAGE` and exit
   non-zero.
5. **Carry preimage and postimage hashes** in the result envelope so
   the agent can compose subsequent operations safely.
6. **Never silently overwrite human edits in generated regions.** The
   `mdli` managed-block model with checksums is the reference: a
   generated block whose checksum no longer matches its content is an
   `E_BLOCK_MODIFIED` error, not a silent overwrite.
7. **Record warnings when fidelity may be lost.** When an operation uses
   a partial implementation path (e.g. `xli`'s `umya-spreadsheet`
   fallback for chart-bearing workbooks), the envelope must carry a
   warning the agent can surface.
8. **Use conflict sidecars for unrecoverable conflicts.** When the tool
   cannot resolve a three-way conflict, write a JSON sidecar (e.g.
   `<file>.mdli.conflict`) containing recorded base, on-disk, and
   incoming content; exit non-zero; leave the source file untouched.
9. **Append to an audit log when configured.** `framerli`'s
   `state/audit.ndjson` and `notionli`'s receipt log are the reference
   patterns. Mutations should be reconstructible from the audit log
   alone.
10. **Allow CI to gate on validation and diff summaries.** `mdli
    validate` and `mdli diff --summary` are the reference: structured
    counts suitable for CI threshold checks.

### 6.3 Reference Tools

- `tools/mdli/README.md` — preimage hashes, managed blocks with
  checksums, three-way conflict sidecars, semantic diff, recipe
  plan/apply.
- `tools/xli/README.md` — atomic commits, fingerprint compare-and-swap,
  partial-fidelity warnings on the `umya-spreadsheet` fallback path.
- `tools/docli/docli-spec.md` §2 — explicit `fast`/`durable`/`paranoid`
  durability modes and the shadow-package write pipeline.
- `tools/framerli/README.md` — dry-run-by-default mutations with audit
  logging.

---

## 7. Artifact Addressing and Selectors

Selectors are how an agent points at a thing inside an artifact. The
platform principle:

> **If a human can rename or reorder it, agents should not rely solely on
> its visible name or ordinal position.**

Recommended addressing model per artifact family:

| Artifact | Stable selector | Bootstrap-only selector | Notes |
|---|---|---|---|
| Markdown sections (`mdli`) | Hidden ID marker (`mdli:id v=1 id=...`) | `--path "H1 > H2"` | Path matches are an error if ambiguous. |
| Markdown tables (`mdli`) | Named table marker + key column | Position within section | Row addressing requires `--key`. |
| Managed blocks (`mdli`) | Block ID with checksum | — | No fallback; blocks must be addressed by ID. |
| Excel cells (`xli`) | A1 / `Sheet!A1:B10` | Named ranges, named tables | Address by named table when one exists. |
| Excel sheets (`xli`) | Sheet name | Sheet index | Renames break index addressing; named addressing survives. |
| Decks (`deckli`) | Slide ID, shape ID, placeholder type (`title`, `body`) | Slide index, shape index | Placeholder type beats positional index when present. |
| DOCX (`docli`) | Heading path + offset, bookmark, paragraph ID | Paragraph index | The spec formalizes a selector enum with `body`/`header.default`/`footer.first` story scopes. |
| Knowledge vault (`vaultli`) | Vault `id` | File path | `INDEX.jsonl` resolves IDs to files; never address by position. |
| Jira (`jirali`) | Issue key (`ENG-123`) | JQL query | JQL is composition, not identity. |
| Notion (`notionli`) | Page/block/data-source UUID | Title search | Aliases (`alias set tasks ...`) make UUIDs ergonomic. |
| Framer (`framerli`) | Project URL + collection slug | — | Project URL is the canonical handle. |
| Clipboard (`clipli`) | UTI (`public.html`) + named template ID | Position in clipboard history | Templates are versioned by name. |

Tools that introduce stable IDs into existing artifacts (the `mdli id
assign` pattern) should provide a one-shot bootstrap command so legacy
files can be lifted into the stable-selector model.

---

## 8. Tool Family Taxonomy

This is the consolidated view of every tool family in the repo today.
Maturity uses the ladder defined in §15.

| Tool | Domain | Source-of-truth model | Agent workflow | Maturity (§15) | Status |
|---|---|---|---|---:|---|
| `mdli` | Markdown AST: sections, tables, frontmatter, managed blocks | File on disk; managed regions fenced with checksums | Inspect → plan → apply → validate → diff | 7 | Implemented; PRD Phases 1–8 shipped. See `tools/mdli/README.md`. |
| `xli` | Excel `.xlsx` workbooks | File on disk; fingerprint compare-and-swap | Inspect → write/format/batch → validate → recalc | 6 | Implemented MVP; partial fidelity on the `umya-spreadsheet` fallback path. See `tools/xli/README.md`. |
| `vaultli` | File-based knowledge vault | Files on disk; `INDEX.jsonl` is a derived cache | Init → ingest/scaffold → index → validate → search | 6 | Implemented (Rust + Python parity). See `tools/vaultli/README.md`. |
| `clipli` | macOS clipboard (HTML/RTF/SVG/PNG/PDF), reusable templates, history | Pasteboard + on-disk template/history stores with versioning/privacy controls | Inspect/watch → capture/history → templatize/render → paste/restore | 5 | Implemented for macOS; spec at `tools/clipli/CLIPLI_SPEC.md`. |
| `jirali` | Atlassian Jira | Live REST/GraphQL + local deterministic state for rehearsal | Auth → JQL/issue/sprint/comment ops with structured exit codes | 5 | Implemented for issue/comment core; broader surfaces deterministic-stub. |
| `notionli` | Notion workspace | Live API + local profile state, dry-run-by-default writes | Search → resolve → row/page/db ops with `--apply` to commit | 4 | MVP implemented; expanding. See `tools/notionli/README.md`. |
| `framerli` | Framer Server API (sites/CMS/publish) | Live API via Node bridge + local audit log | Auth → inspect → plan → publish with approval gates | 4 | Implemented core slice with mock bridge; broader surface returns `E_NOT_IMPLEMENTED`. |
| `deckli` | PowerPoint via Office.js bridge | Live document inside running PowerPoint, addressed via WebSocket sidecar | Inspect masters/theme → batch edits → render → verify | 1 (spec); proto in `addin/`, `bridge/` | Spec at `tools/deckli/DECKLI_SPECS.md`; live-bridge architecture distinct from file-mutation tools. |
| `docli` | Word `.docx` | File on disk; shadow-package atomic commit pipeline | Inspect → narrow edit verbs → review → finalize → diff | 0–1 (spec) | Spec at `tools/docli/docli-spec.md`; workspace crates scaffolded. |
| `vizli` | Parameterized visualization templates → SVG/PNG/HTML/PDF | Sidecar `.md` files describe templates; render is deterministic | Search sidecars → resolve params → render → verify | 1–2 (Python, in development) | Spec at `tools/vizli/VIZLI_README.md` and `tools/vizli/PLAN.md`. |
| `bashli` | Shell workflow execution | JSON TaskSpec → structured StepResult; bash is read-only substrate | Compose TaskSpec → execute → consume structured output | 0–1 (spec) | Spec at `tools/bashli/bashli-spec-final.md`; greenfield. |
| `barli` | macOS menu bar plugin host | Local YAML config + Python plugins; hot-reloads | Drop plugin → configure menu → click | 3 (working app, no test suite) | See `tools/barli/README.md`. Companion to the platform, not an artifact tool. |
| `pdfli` | PDF inspection/extraction/conversion/repair | Planned | Planned | 0 | Placeholder. See `docs/tool-roadmap.md`. |
| `gitli` | GitHub issues, labels, wiki, PRs, repo workflows | Planned | Planned | 0 | Placeholder. See `docs/tool-roadmap.md`. |

Cross-tool relationships:

- `vaultli` is the recommended **knowledge substrate**. Every other tool
  can pull queries, templates, runbooks, and prompts from a vault rather
  than embedding them.
- `mdli` is the recommended **report surface**. Tools that produce
  textual artifacts should emit Markdown that `mdli` can manage
  (sections, named tables, managed blocks).
- `xli` and `vizli` are the recommended **analytics surface**. Numeric
  analysis lives in workbooks; visualizations are deterministic templates.
- `clipli` is the recommended **handoff surface** to GUI apps that lack
  CLIs (Excel, PowerPoint, browsers) on macOS.
- `deckli` is the live-document complement to `docli` and `xli`: it
  manipulates a running app rather than a file on disk.

---

## 9. Cross-Tool Workflow Patterns

These are the canonical compositions the platform is built to support.
Each is a recommended pattern, not a hard-coded pipeline.

### 9.1 Knowledge → Artifact

```text
vaultli search "Q3 revenue runbook"
  → mdli context vault://docs/runbook --max-tokens 2000
    → docli edit insert / mdli section ensure
      → xli create --from-csv / vizli render
        → mdli validate report.md --schema report.schema.yml
```

The agent never hand-writes a report; it composes from indexed
knowledge, drops content into addressable artifacts, and validates the
result.

### 9.2 Analytics Report Generation

```text
vaultli show queries/retention.sql
  → run query → NDJSON rows
    → xli create report.xlsx --from-csv ... + xli format
      → vizli render charts/line_timeseries --data rows.csv
        → mdli table replace report.md --name retention --from-rows rows.ndjson
          → mdli apply report.md --recipe report.yml
            → mdli validate + mdli diff against last week
```

This is the workflow `mdli` was specifically designed for; see
`tools/mdli/README.md` "Recipe / Apply Flow".

### 9.3 Clipboard-Mediated Workflow

```text
clipli inspect → identify rich format (HTML, SVG)
  → clipli capture --templatize
    → agent fills variables
      → clipli paste / clipli excel data.csv --copy-as svg
        → user pastes into Excel/PowerPoint
```

This is the bridge between deterministic tools and GUI apps the
platform does not (yet) drive directly.

### 9.4 Live Deck Workflow

```text
deckli inspect --masters → discover layouts
  → deckli inspect --theme → discover colors
    → deckli batch ops.json → apply edits in one round trip
      → deckli render --slide N → base64 PNG
        → vision verification
          → corrective deckli batch
```

The vision-feedback loop is what makes a live-document bridge worth
the architectural complexity of the sidecar.

### 9.5 Markdown as Control Plane

`mdli` recipes, validation schemas, and templates can be checked into
git and used as the deterministic control plane for what a report
*should* contain. CI runs `mdli apply` and `mdli validate` on every PR.

### 9.6 Agent Skill Workflow

```text
skill frontmatter triggers → SKILL.md instructions
  → preflight check (doctor) → deterministic CLI calls
    → quality gate (validate/lint) → done
```

Skills should not contain prose that the CLI could enforce. If a step
matters, write it as a script or a `validate` command, not as a
paragraph in `SKILL.md`.

---

## 10. Skills and Progressive Disclosure

Skills (see `docs/skills.md`) are how agents learn *when* to use these
tools. The platform recommendations:

- **One skill per mature tool, eventually.** Today: `mdli/SKILL.md`,
  `vaultli/SKILL.md`, `clipli/clipli/SKILL.md`, `deckli/SKILL.md`,
  `jirali/SKILL.md`. Recommended additions: `xli`, `notionli`,
  `framerli` once their command surfaces stabilize.
- **`SKILL.md` stays concise.** Long references go into `references/`
  alongside the skill. Critical determinism goes into scripts, not
  prose. The guidance in `docs/skills.md` is authoritative.
- **Skills should include preflight checks, workflows, failure
  recovery, and quality gates.** The `mdli` SKILL is the reference: it
  documents exit codes, error codes, recommended workflows, and
  recovery patterns.
- **Trigger design matters.** Descriptions should combine domain
  vocabulary (what the tool does) with user phrasing (what the user
  says). See `docs/skills.md` for testing approach.
- **Skills should not duplicate CLI behavior.** If the `apply` command
  validates and writes atomically, the skill should call `apply`, not
  re-implement validation in prose.
- **Cross-tool skills are valuable.** A "generate quarterly report"
  skill could legitimately span `vaultli`, `xli`, `vizli`, and `mdli`.
  Such skills are the right place for the *composition* glue that no
  individual tool owns.

---

## 11. Schema, Manifest, and Discovery Standards

Agents need to discover capabilities without prior knowledge. The
platform's discovery surface, in order of precedence:

1. **`--schema`** emits machine-readable command and result schemas.
   Recommended forms (already in `xli`):
   - `xli schema` — full schema
   - `xli schema --command create` — single-command schema
   - `xli schema --result FormatOutput` — single-result schema
2. **`--agent-manifest`** emits a discovery manifest containing every
   command, its capabilities, error codes, examples, annotations
   (`readOnlyHint`, `idempotentHint`, `destructiveHint`,
   `openWorldHint`), and pagination hints. **Implemented in `tooli`;
   recommended for all tools.**
3. **`--help-agent`** is a compact, low-token, agent-oriented
   alternative to `--help`. **Implemented in `tooli`; recommended.**
4. **Output annotations** in the envelope's `meta.annotations` block
   tell the agent what guarantees the command makes
   (read-only, idempotent, destructive, open-world).
5. **MCP bridge** is a generated projection of the same contract.
   `tooli` apps can serve MCP via `mcp serve`; standalone Rust binaries
   can wrap themselves with `tooli serve` or a thin MCP shim. The CLI
   remains canonical; MCP is a transport.

> The CLI is the canonical interface. Generated schemas, Python APIs,
> and MCP surfaces are projections of the same contract. If a behavior
> is not on the CLI, it is not part of the platform contract.

---

## 12. Testing and Fixture Standards

Testing patterns the platform expects, in rough priority order:

| Test type | Purpose | Reference |
|---|---|---|
| Direct function tests | Confirm core logic without CLI overhead. | All Rust tools' `*_tests.rs`. |
| CLI contract tests | Confirm flags, exit codes, and envelope shape. | `mdli` `cli_contract` tests; `tooli`'s `TooliTestClient`. |
| JSON envelope tests | Lock `ok`/`error`/`meta` shape per tool. | `mdli`, `xli`. |
| Dry-run tests | Confirm no mutation occurs and the plan is faithful. | `mdli`, `notionli`. |
| Idempotency tests | Confirm a second apply produces zero diff. | `mdli` apply / apply-plan. |
| Mutation safety tests | Atomic write, fingerprint, conflict sidecar. | `mdli` three-way conflict; `xli` fingerprint. |
| Stale-preimage tests | Confirm refusal on changed input bytes. | `mdli`. |
| Malformed input tests | Confirm clear errors on bad NDJSON, bad YAML, bad selectors. | `mdli` fixture corpus. |
| Edge-case fixture corpus | Real-world artifact pathologies. | `tools/mdli/tests/fixtures/`. |
| Parity tests (Rust ↔ Python) | When a tool ships in two languages. | `vaultli` Rust/Python parity. |
| Golden output tests | Lock stable JSON shapes; lock rendered SVG/PNG. | `vizli` golden testing per `PLAN.md`. |
| Integration tests | End-to-end workflows across tools. | `jirali` mock-Jira integration. |

**Recommended bar before a tool is "agent-ready":**

1. Three realistic agent scenarios documented and tested.
2. One failure / recovery scenario tested (what does the agent do when
   the artifact is in an unexpected state?).
3. One dry-run / apply scenario tested (if the tool mutates).
4. A schema or contract document the agent can consume.
5. A SKILL.md that triggers correctly and points at the CLI.

---

## 13. Implementation Language Strategy

The repo's apparent pattern, and the recommendation:

- **Rust** for fast, deterministic, low-latency CLI cores — especially
  artifact mutation. Rust's sub-millisecond cold start is what makes
  per-edit atomic commits practical (the `docli` thesis). Used by
  `mdli`, `xli`, `clipli`, `jirali`, `notionli`, `framerli`, `deckli`,
  `docli`, `bashli`, the `vaultli` primary implementation.
- **Python** for ecosystem-heavy validation, dataframes, rendering,
  reconciliation, and reference implementations. Used by
  `xli-companion`, `vaultli/py/core.py`, `vizli`, `barli`. Built with
  `tooli` (`docs/tooli.md`) so Python tools share envelope, error,
  schema, and MCP behavior with each other.
- **TypeScript / JavaScript** when the target platform requires it —
  Office.js add-ins (`deckli/addin/`), Node SDK bridges
  (`framerli/bridge/`, the `deckli` sidecar). Treated as sidecars,
  never as the primary command surface.
- **Sidecars are acceptable** when the target platform requires them
  (Office add-ins, vendor SDKs available only in JS). The platform
  contract is the JSON the sidecar exchanges with the Rust/Python core,
  not the language of the sidecar.

Language choice is downstream of the agent contract. If a tool exposes
the right envelope, the right errors, and the right safety model, its
implementation language is an engineering detail.

---

## 14. Documentation Standards

Documentation should land in predictable places:

| Where | What |
|---|---|
| `README.md` | Repo overview, featured tools, common envelope, status. |
| `AGENTS.md` / `CLAUDE.md` | Short instructions for agents working *in this repo*. |
| `docs/AGENT_TOOLS_PLATFORM_SPEC.md` | This file: the platform contract. |
| `docs/tooli.md` | Python CLI framework guide. |
| `docs/skills.md` | Skill authoring guide; local skills inventory. |
| `docs/tool-roadmap.md` | Tool inventory, status, planned tools. |
| `docs/RUST_CRATES_FOR_TOOLS.md` | Crate selection notes for Rust tools. |
| `tools/<tool>/README.md` | Operator-facing summary. |
| `tools/<tool>/<tool>-spec.md` (or `*_SPEC*.md`, `*_PRD*.md`) | Developer spec. |
| `tools/<tool>/SKILL.md` | Agent-facing operating guide. |
| `tools/<tool>/PLAN.md` | Implementation plan / phase milestones. |
| `tooli_feedback.md` | Agent-tool feedback, friction logs, missing capabilities. |

Each tool's documentation should distinguish at least four registers:

1. **Operator quickstart** — copy-pasteable commands.
2. **Agent workflow guide** — recommended sequences and recovery paths.
3. **Developer spec** — internals, invariants, parity matrix.
4. **Roadmap** — what is partial vs. planned.

Generated documentation should be marked as such (e.g. emit
`<!-- generated by tool X -->` markers) so humans and agents know not
to hand-edit.

---

## 15. Maturity Ladder

A common ladder so the roadmap can be discussed in shared terms.
Classifications below are conservative best-effort; correct in place
when better evidence exists.

| Level | Name | Definition |
|---:|---|---|
| 0 | Idea / spec only | A README or PRD exists; no code. |
| 1 | Prototype command | A binary exists with at least one working subcommand. |
| 2 | JSON contract implemented | All commands emit a stable JSON envelope. |
| 3 | Dry-run and validation implemented | Mutations are rehearsable; validation surfaces structural issues. |
| 4 | Atomic mutation and conflict safety implemented | Atomic writes, fingerprint protection, conflict handling. |
| 5 | Skill and agent workflow documented | A `SKILL.md` exists and has been exercised. |
| 6 | Fixture-backed contract tests | Realistic artifact pathologies are covered by tests. |
| 7 | Cross-tool integration tested | Composes correctly with at least one other `*li` tool. |
| 8 | Stable platform-ready tool | Versioned envelope, complete error registry, durable docs, real users. |

Best-effort current placement (correct in `docs/tool-roadmap.md` when
better evidence emerges):

| Tool | Estimated level |
|---|---:|
| `mdli` | 7 |
| `xli` | 6 |
| `vaultli` | 6 |
| `clipli` | 5 |
| `jirali` | 5 |
| `notionli` | 4 |
| `framerli` | 4 |
| `barli` | 3 (no tests) |
| `vizli` | 1–2 |
| `deckli` | 1 (proto) |
| `docli` | 0–1 (spec, scaffolded) |
| `bashli` | 0–1 (spec) |
| `pdfli`, `gitli` | 0 |

Tools at level 6+ should be safe defaults for new agent workflows.
Tools at level ≤2 should not be wired into autonomous loops without
human review.

---

## 16. Open Questions

Real, unresolved decisions that this spec deliberately does not
foreclose:

1. **One global envelope version, or per-tool envelope versions?**
   `mdli` uses `mdli/output/v1`; recommendation is per-tool versions
   with a shared shape so each tool can evolve independently. Confirm.
2. **Numeric `E1xxx` codes vs. string `E_*` codes?** Both exist today.
   Recommendation is to keep both but require every tool to expose a
   `category` field so generic handlers can branch.
3. **Should `--dry-run` be universal, or should "default to plan" be
   the universal pattern?** `mdli`/`notionli`/`framerli` default to
   plan; `xli` accepts `--dry-run`. Decide whether the platform
   prefers explicit-write or implicit-plan as the default.
4. **Should every tool expose MCP?** `tooli`-based Python tools get
   MCP for free. Rust tools currently do not. A thin shared MCP shim
   would let any Rust tool serve MCP without per-tool effort.
5. **How standardized should schema discovery be?** `xli` has
   `schema --command X --result Y`. Should every tool implement the
   same flag shape? Probably yes.
6. **Should `plan` / `apply-plan` be universal verbs?** `mdli` and
   `notionli` use them. `xli batch` is morally the same thing under a
   different name. Harmonization candidate.
7. **How are partial-fidelity warnings standardized?** `xli` emits
   `umya-spreadsheet` fallback warnings inline; the platform should
   define a recommended `meta.warnings[]` schema.
8. **Should there be a top-level orchestrator CLI** (`agentli` /
   `tooli run`) that knows about every `*li` and can route by
   intent? Possible; not currently scoped.
9. **How does `vaultli` become the knowledge substrate for every
   other tool?** Open question whether tools should accept `vault://`
   URIs natively (the `docli` spec proposes `kb://`) or whether the
   composition stays at the skill layer.
10. **How are credentials standardized?** Today, each tool defines its
    own (`NOTION_API_KEY`, `JIRALI_API_TOKEN`, `FRAMER_API_KEY`,
    Keychain support). A shared `agent_tools` credential layer would
    reduce friction.
11. **How is audit logging standardized?** `framerli`'s
    `state/audit.ndjson` and `notionli`'s receipt log are
    near-identical conceptually. Harmonization candidate.
12. **Should the platform define a shared cancellation/timeout
    contract?** `tooli --timeout` exists; Rust tools handle this
    individually.

---

## 17. Acceptance Criteria for This Spec

This document is successful when:

- A new agent dropped into the repo can read this file plus
  `AGENTS.md` and know how every `*li` tool *should* behave, even if
  it has not yet read that tool's individual docs.
- A developer adding a new tool family (`pdfli`, `gitli`, or any
  future addition) can use this spec as the design checklist.
- A maintainer reviewing a PR can cite this spec to ask: "what
  selector model does this use?", "what is the dry-run path?",
  "where is the error-code registry?".
- The platform can have an honest conversation about which tools are
  level 7 and which are level 1, without overclaiming.
- Implementations diverge from this spec only deliberately, with a
  recorded reason in the relevant tool spec.

This document is *not* successful if it becomes a marketing brochure,
hides current deviations, or describes capabilities the repo does not
have. When that happens, prefer correcting this file over
overstating reality.

---

## Appendix A — Existing Envelope Shapes (current reality)

For agents wiring up parsers today, here are the shapes actually in
use. These are the inputs to harmonization, not the targets.

```jsonc
// mdli — every command
{
  "schema": "mdli/output/v1",
  "ok": true,
  "result": { /* command-specific */ }
}

// mdli — mutating commands return additionally:
{
  "changed": true,
  "preimage_hash": "sha256:...",
  "postimage_hash": "sha256:...",
  "ops": [ { "op": "ensure_section", "id": "...", "level": 2 } ],
  "warnings": []
}

// framerli — success
{ "ok": true, "data": {}, "meta": { "ms": 12, "profile": "default", "dry_run": false } }

// framerli — error
{ "ok": false, "error": { "code": "E_AUTH_MISSING", "message": "...", "hint": "...", "retryable": false }, "meta": {} }

// docli (spec) — success
{ "ok": true, "command": "inspect", "data": { /* ... */ }, "warnings": [], "elapsed_ms": 142 }

// deckli (spec) — success
{ "success": true, "command": "set.text", "result": { /* ... */ }, "timing_ms": 47 }

// clipli — minimal failure
{ "ok": false, "error": "message", "code": "ERROR_CODE" }

// tooli (recommended target — see docs/tooli.md) — success
{
  "ok": true,
  "result": { /* ... */ },
  "meta": {
    "tool": "app.command", "version": "1.0.0", "duration_ms": 42,
    "dry_run": false, "warnings": [], "annotations": { "readOnlyHint": true },
    "truncated": false, "next_cursor": null
  }
}
```

## Appendix B — Recommended New-Tool Checklist

Before merging a new `*li` tool family:

- [ ] Binary name follows `<domain>li` convention.
- [ ] Subcommands use the verb vocabulary in §4.1 where applicable.
- [ ] Global flags from §4.2 do not conflict with command-local flags.
- [ ] Every command emits a stable JSON envelope (success and error).
- [ ] Every error has `code`, `category`, `message`, `details`,
      `suggestion`, `is_retryable`.
- [ ] Mutating commands either default to a plan or require an
      explicit `--write`/`--apply`/`--yes` flag.
- [ ] Mutating commands write atomically.
- [ ] Mutating commands accept a preimage hash where the artifact has
      a stable byte form.
- [ ] Selectors prefer stable IDs over names/positions.
- [ ] `--schema`, `--agent-manifest`, `--help-agent` are present
      (or noted as planned).
- [ ] `tools/<tool>/README.md` exists with operator quickstart.
- [ ] `tools/<tool>/<tool>-spec.md` (or PRD) exists for developers.
- [ ] At least three realistic agent scenarios are tested.
- [ ] At least one failure-recovery scenario is tested.
- [ ] If mutating: at least one dry-run/apply scenario is tested.
- [ ] `SKILL.md` is drafted (can be terse) once the surface stabilizes.
- [ ] `docs/tool-roadmap.md` is updated with status.
- [ ] If the tool deviates from this spec, the deviation is recorded
      in the tool's spec with a reason.
