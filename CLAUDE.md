# Agent Tools

This repository contains a collection of tools designed for use by AI agents, with a focus on the `tooli` module.

## Purpose

The goal of this project is to build a suite of practical, composable tools that agents can leverage to perform common tasks — particularly text processing, file manipulation, and content transformation.

## Key Modules

- **`tooli`** — The primary module for agent tooling. Tools should be developed here as part of a cohesive toolkit.
- **`markdown_cleaner.py`** — A standalone markdown cleaning utility (paragraph unwrapping, span/div cleanup, EPUB index link conversion).

## Available Tools

These tools are ready to use. Invoke them via `uv run` from the repo root. All commands return structured JSON when called non-interactively or with `--json`.

### Markdown Search — `tools/md_search.py`

Search markdown files for structured elements.

```bash
# Extract all headings (returns [{level, text, line}, ...])
uv run tools/md_search.py headers <file> [--level 1-6] [--json]

# Extract all links (returns [{text, url, line, type}, ...])
uv run tools/md_search.py links <file> [--external-only] [--json]

# Extract fenced code blocks (returns [{language, content, start_line, end_line}, ...])
uv run tools/md_search.py code-blocks <file> [--language python] [--json]
```

### Image Manipulation — `tools/img_manipulate.py`

Resize, crop, convert, and batch-process images.

```bash
# Resize by width, height, or scale factor
uv run tools/img_manipulate.py resize <file> [--width N] [--height N] [--scale 0.5] [--fit contain|cover|stretch] [--out-file out.png] [--json]

# Crop to a rectangular region (coordinates clamped to bounds; omit x/y to center)
uv run tools/img_manipulate.py crop <file> [--x N] [--y N] [--width N] [--height N] [--out-file out.png] [--json]

# Convert to a different format (jpeg, png, webp, gif, bmp, tiff)
uv run tools/img_manipulate.py convert <file> --format webp [--out-file out.webp] [--json]

# Convert all images in a directory
uv run tools/img_manipulate.py batch-convert <dir> --format webp [--output-dir ./out] [--pattern "*.png"] [--json]

# Flatten transparency onto a solid background (default: black)
uv run tools/img_manipulate.py add-background <file> [--color black] [--out-file out.png] [--json]
```

### Markdown Cleaner — `tools/md_clean.py`

Clean markdown files: unwrap paragraphs, strip `<span>`/`<div>` tags, normalize EPUB links.

```bash
# Clean a file (writes <name>_cleaned.md by default)
uv run tools/md_clean.py clean <file> [--out-file out.md] [--in-place] [--keep-divs] [--convert-index] [--json]
```

### Invocation Notes

- All tools accept `--json` to force JSON envelope output — use this in agent contexts.
- All tools return structured errors with `"ok": false, "error": {"code": ..., "message": ..., "suggestion": ...}` on failure.
- All tools support `--dry-run` on commands that modify or write files.
- Run `uv run tools/<tool>.py --help` for full option reference.

## Core Responsibilities

- **Improving the `tooli` module**: A primary responsibility of agents working in this repo is to identify and document improvements for the `tooli` module. When you encounter bugs, missing features, usability issues, or opportunities for enhancement, record them in `tooli_feedback.md`. This file should be detailed and actionable — it will be processed by the `tooli` developers to drive changes. Include context such as what you were trying to do, what went wrong or was missing, and any suggested fixes.

## Reference

- **`TOOLI_DEV_GUIDE.md`** — Comprehensive developer guide for building tools with the `tooli` framework. Covers app configuration, command registration, annotations, output modes, error handling, special input types, dry-run, pagination, security, Python API, MCP integration, testing, and common patterns. Read this before writing any tooli-based tool.

## Development Guidelines

- Tools should be designed with agent consumption in mind: clear inputs/outputs, minimal dependencies, and predictable behavior.
- Keep individual tools focused and composable.
- Use `uv` for Python dependency management.
