---
name: docli
description: Use docli to inspect, create, patch, query, render, validate, and finalize Word .docx documents from an agent-native CLI. Use when the user needs structured DOCX automation, high-fidelity document edits, OOXML-safe patching, templates, comments, redlines, or JSON-readable document operations.
---

# docli — Agent Guide

`docli` is the planned DOCX counterpart to `xli` and `mdli`: a Rust CLI that
turns Word document work into structured operations instead of ad hoc OOXML
editing.

## Agent Contract

- Prefer high-level commands and specs over hand-editing XML.
- Treat the `.docx` package as the transaction boundary.
- Use JSON output for all command results.
- Inspect before patching; validate after patching.
- Preserve formatting and styles unless the user asked for a redesign.
- For high-stakes edits, create a copy and patch the copy first.

## Intended Workflow

1. Inspect document structure:

   ```bash
   docli inspect input.docx --json
   docli query input.docx --selector headings --json
   ```

2. Create a patch spec:

   ```yaml
   operations:
     - replace_text:
         selector: heading:Executive Summary
         text: "Updated executive summary..."
     - insert_after:
         selector: bookmark:metrics
         block:
           type: table
           rows:
             - ["Metric", "Value"]
             - ["Revenue", "$4.2M"]
   ```

3. Apply to a new output:

   ```bash
   docli patch input.docx --spec patch.yml --out output.docx --json
   ```

4. Validate and render:

   ```bash
   docli validate output.docx --json
   docli render output.docx --out pages/ --json
   ```

## Command Families

| Need | Command family |
|---|---|
| Inspect package, styles, sections, headings, tables | `inspect` |
| Query document content with stable selectors | `query` |
| Create a document from a spec/template | `create` |
| Patch text, tables, styles, comments, sections | `patch` |
| Validate OOXML/package integrity | `validate` |
| Render previews for visual QA | `render` |
| Emit schemas and examples | `schema` |
| Connect templates and style rules | `kb` / template integration |

## Safety Rules

- Never unzip/edit/repack a user document by hand when `docli` can express the
  operation.
- Do not rely on visible text alone if headings/bookmarks/content controls are
  available.
- Keep patch specs in files so they can be reviewed and rerun.
- Render page images after layout-sensitive changes.
- Preserve track changes, comments, and document metadata unless explicitly
  asked to remove them.

## Current Status

The detailed product spec lives in [`docli-spec.md`](docli-spec.md). Treat this
skill as the agent operating contract for the intended CLI. If a command is not
implemented yet, use the spec to plan the operation and clearly report the
implementation gap rather than silently falling back to fragile XML edits.

## When Not To Use docli

- Use `mdli` for Markdown.
- Use `xli` for Excel workbooks.
- Use `deckli` for live PowerPoint documents.
- Use the document artifact/runtime skills when the user specifically requests a
  `.docx` artifact and the CLI is not yet available.
