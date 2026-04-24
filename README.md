# agent_tools

Agent-native tools and workflow helpers. The repository is centered on practical,
composable CLIs that return structured output for agents while still being usable
by humans.

## What Lives Here

- `tools/` contains individual tool projects and prototypes.
- `skills/` contains reusable agent skills used while developing this repo.
- `markdown_cleaner.py` is a standalone Markdown cleanup utility.
- `tests/` contains Python tests for the Python tools.
- `docs/` contains the edited project reference material.

## Current Tool Families

- `vaultli`: file-based knowledge base management with YAML frontmatter,
  sidecar markdown, and `INDEX.jsonl` metadata search.
- `clipli`: macOS clipboard inspection, capture, templated paste, Excel-native
  HTML generation, and clipboard format conversion.
- `barli`: macOS menubar automation experiments.
- `xli`: Rust Excel workbook CLI with inspect/read/write/format/sheet/batch,
  create/import, template/apply, quality checks, and schema discovery.
- `deckli`, `docli`, `vizli`, `framerli`, `notionli`, `bashli`, and `jirali`:
  tool workspaces, specs, or prototypes for document, presentation,
  visualization, design, shell, and integration workflows.
- Legacy Python test fixtures still reference `tools/md_search.py`,
  `tools/img_manipulate.py`, and `tools/md_clean.py`, but those scripts are not
  present in the current tree.

## Setup

```bash
uv sync
uv run pytest
```

Requires Python 3.10+. The Python project currently pins `tooli==6.6.0` and
uses `Pillow`, `pyyaml`, `rumps`, and `watchdog`.

## Documentation

- [`docs/tooli.md`](docs/tooli.md): concise guide for building agent-friendly
  Python CLIs with `tooli`.
- [`docs/skills.md`](docs/skills.md): skill authoring guide plus this repo's
  skills inventory.
- [`docs/tool-roadmap.md`](docs/tool-roadmap.md): active tool inventory,
  planned tool families, and agent-facing test expectations.
- [`docs/RUST_CRATES_FOR_TOOLS.md`](docs/RUST_CRATES_FOR_TOOLS.md): Rust crate
  notes for CLI tools.
- [`docs/integrating_jq_in_rust.md`](docs/integrating_jq_in_rust.md): notes on
  jq-style behavior in Rust.
- [`docs/excel_format_on_clipboard.md`](docs/excel_format_on_clipboard.md):
  Excel clipboard format notes.
- [`tools/xli/README.md`](tools/xli/README.md): current `xli` build, usage,
  test, and spec-parity guide.

## Agent Notes

Agents should read [`AGENTS.md`](AGENTS.md) before making changes. When you find
bugs, missing features, stale docs, or usability problems in `tooli` or these
tools, add actionable notes to [`tooli_feedback.md`](tooli_feedback.md).
