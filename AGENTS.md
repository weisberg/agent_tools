# Agent Tools

This repository contains agent-native tools, workflow prototypes, and reusable
skills. Prefer small, composable tools with explicit inputs, structured outputs,
and predictable error handling.

## Project Focus

- Build practical tools for AI agents, especially CLIs for text processing,
  file manipulation, content transformation, documents, spreadsheets, browser or
  app workflows, and knowledge management.
- Use the existing tool families in `tools/` as the organizing structure.
- Keep documentation concise and current. Put durable reference material in
  `docs/`, not scattered through the repo root.

## Important References

- `README.md`: project overview and current documentation map.
- `docs/tooli.md`: concise guide for building Python CLIs with `tooli`.
- `docs/skills.md`: skill authoring guide and local skills inventory.
- `docs/tool-roadmap.md`: tool family inventory, standards, and legacy tool
  notes.
- `tooli_feedback.md`: actionable feedback for `tooli` and agent-tool
  usability.

## Working Guidelines

- Use `uv` for Python dependency management.
- Use `rg` / `rg --files` for searching.
- Keep tools focused and composable.
- Return dictionaries, lists, or typed data from tool functions; avoid making
  agents parse prose.
- Prefer JSON or JSONL for non-interactive output.
- Add or preserve `--dry-run` behavior for mutating commands where possible.
- Add focused tests when changing behavior.

## Tooli Feedback

When you encounter a `tooli` bug, missing feature, stale doc, confusing error,
or agent-usability issue, add an actionable entry to `tooli_feedback.md`.
Include:

- What you were trying to do.
- What failed or was missing.
- The observed error or confusing behavior.
- A suggested fix or migration note.

## Current Caveat

Some legacy tests and older notes reference `tools/md_search.py` and
`tools/img_manipulate.py`. These scripts are not present in the current tree.
Treat them as legacy/planned surfaces unless they are restored. The earlier
`tools/md_clean.py` notes are superseded by the Rust `mdli` crate at
`tools/mdli/`.
