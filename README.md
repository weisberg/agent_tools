# agent_tools

A collection of agent-native tools built on the [`tooli`](https://github.com/weisberg/tooli) framework. Each tool exposes a typed Python API, a CLI, and a JSON Schema — making them consumable by AI agents, scripts, and humans alike.

## Tools

### Markdown Search — `tools/md_search.py`

Extract structured elements from markdown files.

```bash
# All headings with level, text, and line number
uv run tools/md_search.py headers <file> [--level 1-6] [--json]

# All links (inline, reference-style, autolinks)
uv run tools/md_search.py links <file> [--external-only] [--json]

# Fenced code blocks filtered by language
uv run tools/md_search.py code-blocks <file> [--language python] [--json]
```

### Image Manipulation — `tools/img_manipulate.py`

Resize, crop, convert, and process images via Pillow.

```bash
uv run tools/img_manipulate.py resize <file> [--width N] [--height N] [--scale 0.5] [--fit contain|cover|stretch] [--out-file out.png] [--json]
uv run tools/img_manipulate.py crop <file> [--x N] [--y N] [--width N] [--height N] [--out-file out.png] [--json]
uv run tools/img_manipulate.py convert <file> --format webp [--out-file out.webp] [--json]
uv run tools/img_manipulate.py batch-convert <dir> --format webp [--output-dir ./out] [--pattern "*.png"] [--json]
uv run tools/img_manipulate.py add-background <file> [--color black] [--out-file out.png] [--json]
```

### Markdown Cleaner — `tools/md_clean.py`

Unwrap hard-wrapped paragraphs, strip `<span>`/`<div>` tags, and normalize EPUB index links.

```bash
uv run tools/md_clean.py clean <file> [--out-file out.md] [--in-place] [--keep-divs] [--convert-index] [--json]
```

### Knowledge Vault — `tools/vaultli/`

Manage a file-based knowledge base with YAML frontmatter, sidecar markdown for non-markdown assets, and a derived `INDEX.jsonl` for metadata search.

```bash
uv run python -m tools.vaultli init ./kb
uv run python -m tools.vaultli add ./kb/docs/guide.md --root ./kb
uv run python -m tools.vaultli scaffold ./kb/queries/report.sql --root ./kb
uv run python -m tools.vaultli validate --root ./kb --json
uv run python -m tools.vaultli search report --root ./kb --json
```

See `tools/vaultli/README.md` for the full guide and `tools/vaultli/SKILL.md` for the agent-oriented workflow.

### Clipboard Intelligence — `tools/clipli/`

Capture clipboard content as reusable templates, render formatted output back to the clipboard, generate Excel-native HTML from CSV, and convert between clipboard-friendly formats.

```bash
tools/clipli/target/release/clipli inspect
tools/clipli/target/release/clipli excel data.csv --dry-run
tools/clipli/target/release/clipli capture --name quarterly_report --preview
tools/clipli/target/release/clipli paste quarterly_report -D '{"title":"Q2"}' --dry-run
tools/clipli/target/release/clipli lint quarterly_report --json
```

See `tools/clipli/clipli/SKILL.md` for the agent-oriented workflow and `tools/clipli/CLIPLI_SPEC.md` for the full command surface.

## Output Format

All tools return a consistent JSON envelope:

```json
{"ok": true, "result": { ... }, "meta": {"tool": "...", "duration_ms": 12, ...}}
{"ok": false, "error": {"code": "E3001", "message": "...", "suggestion": {"fix": "..."}}}
```

Pass `--json` to force JSON output in any context. Use `--dry-run` on write commands to preview without mutating files.

## Setup

```bash
uv sync
uv run pytest          # run all 97 tests
```

Requires Python 3.10+. Dependencies: `tooli`, `Pillow`.

## Skills

The `skills/` directory contains Claude Code skills used during development of this repo:

- **`skills/github-issues/`** — Full GitHub Issues lifecycle via the `gh` CLI (create, label, close, search, bulk ops, Projects integration).
- **`skills/claude-md-author/`** — Author and improve `CLAUDE.md` / `AGENTS.md` files for Claude Code projects.

## Reference

- [`TOOLI_DEV_GUIDE.md`](TOOLI_DEV_GUIDE.md) — Comprehensive developer guide for building tooli-based tools.
- [`tooli_feedback.md`](tooli_feedback.md) — Bug reports and improvement requests filed against the tooli framework.
- [`CLAUDE.md`](CLAUDE.md) / [`AGENTS.md`](AGENTS.md) — Agent instructions for working in this repo.
