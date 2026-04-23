---
name: visualizer
description: >
  Operate the vizli tool to turn existing vizli templates into charts,
  diagrams, explainers, and other rendered artifacts. Use this skill when the
  user asks to render, generate, preview, inspect, validate, or troubleshoot a
  visualization with vizli, or when they want a chart or diagram produced from
  an existing vizli template library and data files. This skill is for
  template discovery, payload resolution, command execution, and output
  verification. It is not for authoring new vizli templates from scratch
  unless the user explicitly asks for conversion or template creation.
---

# Visualizer Skill

Use this skill as the runtime operator for vizli. The job is to find the
right template, resolve its payload from the user's files and parameters,
run `vizli`, verify the output, and return the artifact or the blocking
constraint.

## What this skill is for

Use `visualizer` when the user wants:

- A chart, diagram, report visual, or explainer rendered with `vizli`
- Help choosing among existing vizli templates
- Help mapping their data files onto template params or datasets
- `vizli inspect`, `vizli validate`, `vizli render`, `vizli golden`, or `vizli doctor`
- Troubleshooting a vizli render failure or output mismatch

Do not use this skill for template authoring by default. If the user wants a
new template or a conversion from raw HTML/SVG/JSON into a vizli template,
that is a separate template-authoring task.

## Default workflow

1. Read the user's request for the visual form, output format, and destination.
2. Discover candidate templates by searching sidecar files, not template bodies.
3. Read only the best matching sidecars and select one template.
4. Inspect the user's data shape and resolve the required params and datasets.
5. Validate or inspect the template before rendering when the mapping is unclear.
6. Render with explicit flags for format, output path, and any param overrides.
7. Verify the output file exists and that its format matches what the user asked for.
8. Report what was rendered, which template was used, and any unresolved limits.

Read the references progressively:

- `references/spec-map.md` for where the authoritative repo docs live
- `references/template-discovery.md` when choosing a template
- `references/render-recipes.md` when building commands and payloads
- `references/failures.md` when validation or rendering fails
- `references/workflow.md` when you need the full operator checklist

## Hard rules

- Search sidecars first. The sidecar is the selection surface.
- Do not invent params or dataset names. Read them from the sidecar or schema.
- Prefer `vizli inspect` or `vizli validate` before guessing at a render command.
- Treat the template as payload-driven. Fix the payload mapping before changing anything else.
- Do not promise formats the engine does not support.
- If the repo has no usable template library or sidecars, say so plainly and stop.
- If the user asks for a brand new visual and no template fits, recommend template creation instead of forcing a bad match.

## Repo authority

These repo docs define the real contract. Read them only when needed:

- [`../../VIZLI_README.md`](../../VIZLI_README.md) for the agent runtime model
- [`../../SIDECAR_SPEC.md`](../../SIDECAR_SPEC.md) for template discovery and sidecar structure
- [`../../OUTPUT_SPEC_FINAL.md`](../../OUTPUT_SPEC_FINAL.md) for CLI behavior and output support
- [`../../TEMPLATE_SPEC_FINAL.md`](../../TEMPLATE_SPEC_FINAL.md) for payload shape and template constraints

## Minimal success criteria

A vizli task is complete only when you can state:

- Which template was selected
- Which inputs filled `params`, `datasets`, and output flags
- Which `vizli` command ran
- Which output artifact was produced, or the precise reason rendering was blocked
