# Tool Roadmap

This document consolidates the old root `TOOLS.md` planning checklist and the
tool-family notes that lived in `tools/TOOLS.md`.

## Agent Tool Standards

The architectural contract for every `*li` tool — envelope shape, error
taxonomy, mutation safety lifecycle, selector model, schema discovery,
testing standards, and the new-tool checklist — is defined in
[`AGENT_TOOLS_PLATFORM_SPEC.md`](AGENT_TOOLS_PLATFORM_SPEC.md). Cross-tool
error codes are catalogued in [`error-registry.md`](error-registry.md).

In short, every tool should:

- Take explicit, validated inputs.
- Emit a stable JSON envelope with a `meta` block.
- Use stable error codes with `category`, `suggestion`, `is_retryable`.
- Default to safe (dry-run plan) for mutating commands and write atomically.
- Prefer stable selectors (IDs, A1 ranges, named tables) over names/positions.
- Compose with other `*li` tools via NDJSON and stdin/stdout.

See the spec for the authoritative form, including envelope deviations
already in flight.

## Maturity Ladder

The platform spec defines an 8-level maturity ladder (§15). Best-effort
classifications are reproduced in the tool-family table below; correct in
place when better evidence exists.

## Current Tool Families

Quick view (maturity per the platform spec ladder; see each tool's section
below for detail):

| Tool | Domain | Maturity | Notes |
|---|---|---:|---|
| `mdli` | Markdown AST | 7 | PRD Phases 1–8 implemented; cross-tool integration tested via recipes |
| `xli` | Excel `.xlsx` | 6 | Atomic + fingerprint; `umya-spreadsheet` fallback warnings remain |
| `vaultli` | File-based knowledge vault | 6 | Rust + Python parity, sidecar model, `INDEX.jsonl` cache |
| `clipli` | macOS clipboard intelligence | 5 | Capture/templatize/render loop; macOS-only |
| `jirali` | Jira issues / JQL / sprints | 5 | Live + local-deterministic execution paths |
| `notionli` | Notion workspace | 4 | MVP shipped; expanding |
| `framerli` | Framer Server API | 4 | Rust core + Node bridge; mock mode for CI |
| `barli` | macOS menu bar plugin host | 3 | Hot-reload Python plugins; no test suite |
| `vizli` | Visualization templates → SVG/PNG/HTML/PDF | 1–2 | Spec stable, implementation in progress |
| `deckli` | PowerPoint via Office.js bridge | 1 | Spec + add-in/bridge proto |
| `docli` | Word `.docx` | 0–1 | Spec mature; workspace crates scaffolded |
| `bashli` | Structured shell execution | 0–1 | Spec; greenfield |
| `pdfli` | PDF tooling | 0 | Placeholder |
| `gitli` | GitHub issues, PRs, wiki | 0 | Placeholder |

### `vaultli`

File-based knowledge base management with YAML frontmatter, sidecar markdown for
non-markdown assets, and a derived `INDEX.jsonl`.

Best for making docs, queries, templates, runbooks, and skills discoverable
without introducing a database.

Start with:

- `tools/vaultli/README.md`
- `tools/vaultli/SKILL.md`

### `clipli`

macOS clipboard intelligence for capture, templated paste, Excel-style HTML/SVG/PNG
table generation, and format conversion.

Best when an agent needs to inspect the current clipboard, preserve formatting,
or preview rich output before writing it back.

Start with:

- `tools/clipli/README.md`
- `tools/clipli/clipli/SKILL.md`
- `tools/clipli/CLIPLI_SPEC.md`

### `barli`

macOS menubar automation experiments.

Current docs:

- `tools/barli/README.md`
- `tools/barli/SKILL.md`

### `deckli`

Presentation/deck tooling. Current useful docs include:

- `tools/deckli/SKILL.md`
- `tools/deckli/DECKLI_SPECS.md`
- `tools/deckli/LAYOUTS.md`
- `tools/deckli/RECIPES.md`

### `docli`

Document tooling. Current docs:

- `tools/docli/SKILL.md`
- `tools/docli/docli-spec.md`
- `tools/docli/PYTHON_COMPANION_TO_DOCLI.md`

### `xli`

Spreadsheet/workbook tooling. `xli` is now a working Rust workspace for
JSON-first Excel operations: inspect, read, write, format, sheet management,
batch edits, workbook creation/import, quality checks, schema discovery, and
minimal built-in template/apply support. The Python companion handles heavier
validation, reconciliation, artifact auditing, and report generation.

Current docs:

- `tools/xli/README.md`
- `tools/xli/SKILL.md`
- `tools/xli/xli-spec.md`
- `tools/xli/PYTHON_COMPANION_TO_XLI.md`
- `tools/xli/xli-companion/README.md`

Current caveats:

- Mutating commands still use the `umya-spreadsheet` fallback path and emit a
  warning; artifact-preserving OOXML patch coverage remains active work.
- The spec is broader than the MVP. Use `tools/xli/README.md` for the current
  parity matrix.

### `vizli`

Visualization and explainer output tooling. Current docs:

- `tools/vizli/SKILL.md`
- `tools/vizli/VIZLI_README.md`
- `tools/vizli/VIZLI_OUTPUT_SPEC.md`
- `tools/vizli/OUTPUT_SPEC_FINAL.md`
- `tools/vizli/TEMPLATE_SPEC_FINAL.md`
- `tools/vizli/SIDECAR_SPEC.md`
- `tools/vizli/PLAN.md`

### `framerli`

Framer integration tooling. Current docs:

- `tools/framerli/README.md`
- `tools/framerli/SKILL.md`
- `tools/framerli/framerli_prd.md`
- `tools/framerli/framerli_brainstorm_features.md`

### `notionli`

Notion integration tooling. Current docs:

- `tools/notionli/README.md`
- `tools/notionli/SKILL.md`
- `tools/notionli/notionli_prd.md`
- `tools/notionli/notionli_brainstorm_features.md`

### `bashli`

Shell workflow tooling. Current docs:

- `tools/bashli/SKILL.md`
- `tools/bashli/bashli-spec-final.md`
- `tools/bashli/PLAN.md`
- `tools/bashli/CLAUDE.md`

### `jirali`

Jira integration ideas. Current doc:

- `tools/jirali/jirali_brainstorming_features.md`

### `mdli`

Agent-native Markdown document operations. Treats Markdown as an editable AST,
not a string. Stable IDs, named tables, managed blocks with checksums,
NDJSON-to-table rendering, idempotent mutations, and dry-run plans.

MVP surface implemented (PRD Phases 1–4): `inspect`, `tree`, `context`,
`id list/assign`, `section list/get/ensure/replace/delete/move/rename`,
`table list/get/replace/upsert/delete-row/sort/fmt`,
`block list/get/ensure/replace/lock/unlock`, `frontmatter get/set/delete`,
`lint`, `validate --schema`. Post-MVP layer also implemented (Phases 5–8):
`template render`, `recipe validate`, `apply`, `build`, `plan`, `apply-plan`,
`patch`, `diff` (semantic, identity-anchored on stable IDs / table names /
block IDs). `block replace --on-modified three-way` writes a
`<file>.mdli.conflict` JSON sidecar with the recorded, on-disk, and incoming
bodies. `E_AMBIGUOUS_SELECTOR` carries a structured `details.matches` array
so agents can disambiguate without re-parsing.

`apply` and `apply-plan` produce byte-identical output (recipe-hash
provenance is threaded through the plan). The `diff` command reports
structural events (sections renamed/moved, table rows added/removed/updated
by key, managed-block content/lock changes, locked-edit attempts, tampering,
frontmatter deltas) and emits a `summary` block of counts suitable for CI
gating thresholds.

Git integration (`--require-clean-git`, snapshot mode) is backlogged.

Current docs:

- `tools/mdli/README.md`
- `tools/mdli/SKILL.md`
- `tools/mdli/mdli-prd-final.md`

Test coverage: 99 integration tests across `cli_contract`, `fixtures`,
`recipe`, `template`, `tree`, `context`, `validate`, and `diff`, plus a
fixture corpus covering the PRD's documented edge cases (duplicate headings,
escaped `>`, Unicode, code-fence content, malformed tables, locked/tampered
blocks, orphan markers, newer-version markers, inline HTML, YAML/TOML
frontmatter, CRLF, BOM).

The legacy Python script `tools/mdli/markdown_cleaner.py` is superseded by the
Rust crate.

## Legacy Python Tool Ideas

Older docs described two Python tools. Tests for them still exist, but the
scripts are not present in the current tree.

### Markdown Search

Intended path: `tools/md_search.py`.

Proposed behavior:

- Extract headings with `{level, text, line}`.
- Extract links with `{text, url, line, type}`.
- Extract fenced code blocks with `{language, content, start_line, end_line}`.
- Support filters such as heading level, external links only, and code language.

Agent test scenarios:

- Extract all headings from a multi-level markdown file.
- Extract all external links from a README with inline, reference, and autolinks.
- Extract only Python fenced code blocks from a mixed-language file.

### Image Manipulation

Intended path: `tools/img_manipulate.py`.

Proposed behavior:

- Resize by width, height, or scale.
- Crop by coordinates or centered region.
- Convert between common image formats.
- Batch-convert directories.
- Flatten transparency onto a background color.

Agent test scenarios:

- Resize a large image to a thumbnail with predictable aspect ratio behavior.
- Batch-convert `.bmp` files to `.webp`.
- Crop with out-of-bounds coordinates and return either a clear error or a
  documented clamped result.

## Planned Tool Families

These names appeared in older planning notes and remain useful placeholders:

| Name | Domain |
|---|---|
| `pdfli` | PDF inspection, extraction, conversion, and repair |
| `gitli` | GitHub issues, labels, wiki, PRs, and repository workflows |

Before adding a new tool family, write down the smallest useful command surface,
the structured output contract, and at least three agent test scenarios.
