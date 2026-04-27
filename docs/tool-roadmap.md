# Tool Roadmap

This document consolidates the old root `TOOLS.md` planning checklist and the
tool-family notes that lived in `tools/TOOLS.md`.

## Agent Tool Standards

All tools should be designed for agent consumption:

- Inputs must be explicit and validated.
- Outputs should be structured, stable, and documented.
- Errors should include machine-readable codes and actionable recovery guidance.
- Mutating commands should support `--dry-run` when possible.
- Commands should be focused enough to compose with other tools.
- Human-readable output is welcome, but non-interactive calls should produce JSON
  or another predictable format.

## Current Tool Families

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

macOS menubar automation experiments. See `tools/barli/README.md`.

### `deckli`

Presentation/deck tooling. Current useful docs include:

- `tools/deckli/SKILL.md`
- `tools/deckli/DECKLI_SPECS.md`
- `tools/deckli/LAYOUTS.md`
- `tools/deckli/RECIPES.md`

### `docli`

Document tooling. Current docs:

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

- `tools/vizli/VIZLI_README.md`
- `tools/vizli/VIZLI_OUTPUT_SPEC.md`
- `tools/vizli/OUTPUT_SPEC_FINAL.md`
- `tools/vizli/TEMPLATE_SPEC_FINAL.md`
- `tools/vizli/SIDECAR_SPEC.md`
- `tools/vizli/PLAN.md`

### `framerli`

Framer integration tooling. Current docs:

- `tools/framerli/README.md`
- `tools/framerli/framerli_prd.md`
- `tools/framerli/framerli_brainstorm_features.md`

### `notionli`

Notion integration tooling. Current docs:

- `tools/notionli/README.md`
- `tools/notionli/notionli_prd.md`
- `tools/notionli/notionli_brainstorm_features.md`

### `bashli`

Shell workflow tooling. Current docs:

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

MVP surface implemented (Phases 1–4 from the PRD): `inspect`, `tree`,
`id list/assign`, `section list/get/ensure/replace/delete/move/rename`,
`table list/get/replace/upsert/delete-row/sort/fmt`,
`block list/get/ensure/replace/lock/unlock`, `frontmatter get/set/delete`,
`lint`. Post-MVP layer also implemented: `template render`, `recipe validate`,
`apply`, `build`, `plan`, `apply-plan`, `patch`.

Current docs:

- `tools/mdli/mdli-prd-final.md`

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
