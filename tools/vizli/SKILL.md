---
name: vizli
description: Use vizli to discover visualization templates, resolve parameters, render charts/diagrams/explainers, and verify generated visual artifacts. Use when the user asks for template-driven SVG/PNG/HTML visualizations, explainers, diagrams, or repeatable data-to-visual workflows.
---

# vizli — Agent Guide

`vizli` is a template-driven visualization workflow. Agents should not improvise
visual output from scratch when a matching template exists; they should discover
the right template, resolve parameters, render, and verify.

## Agent Contract

- Search sidecar metadata before choosing a template.
- Match template type, tags, required params, input data shape, and output
  format to the user's request.
- Do not skip required parameters.
- Render to an explicit output path.
- Verify the output exists and, for visual formats, perform an appropriate
  render/screenshot check before delivery.
- Do not modify templates unless the user asked for template authoring.

## Mental Model

```text
User request
  -> discover sidecars
  -> pick template
  -> resolve params and data
  -> vizli render
  -> verify output
  -> deliver artifact
```

## Sidecar Discovery

Each template should have a Markdown sidecar with YAML frontmatter:

```text
templates/
  charts/bar_grouped.vizli.json
  charts/bar_grouped.vizli.json.md
  explainers/ab_test_power.vizli.html
  explainers/ab_test_power.vizli.html.md
```

Sidecars describe:

- template path
- type, such as Vega-Lite, HTML, SVG, or explainer
- title and description
- tags
- supported output formats
- required and optional params
- expected data shape
- examples and known failure modes

## Safe Default Workflow

1. Find candidate sidecars:

   ```bash
   rg -n "bar|line|scatter|explainer|diagram" tools/vizli
   ```

2. Read the best sidecar and confirm:

   - Does the template match the requested visual?
   - Are all required params available?
   - Is the requested output format supported?
   - Does the data match the expected shape?

3. Render:

   ```bash
   vizli render template.vizli.json \
     --data data.csv \
     --param x_field=month \
     --param y_field=revenue \
     --out out.svg
   ```

4. Verify:

   ```bash
   test -s out.svg
   ```

   For PNG/HTML outputs, render or screenshot when possible and inspect for
   blank, clipped, or overlapping content.

## Choosing A Template

| User asks for | Prefer |
|---|---|
| categorical comparison | grouped/stacked bar template |
| trend over time | line or area timeseries template |
| correlation | scatter/regression template |
| process or architecture | diagram template |
| concept explanation | explainer HTML template |
| statistical concept | domain explainer template |

## Common Failure Modes

- Missing required params: read sidecar `params` and fill every required value.
- Wrong data type: convert or reshape data before rendering.
- Empty output: inspect rendered file size and template data binding fields.
- Labels overlap: use template params for width, height, label rotation, or
  truncation if available.
- Unsupported output format: choose a compatible format or a different template.

## References

- [`VIZLI_README.md`](VIZLI_README.md): operating manual for template rendering.
- [`SIDECAR_SPEC.md`](SIDECAR_SPEC.md): sidecar schema.
- [`VIZLI_OUTPUT_SPEC.md`](VIZLI_OUTPUT_SPEC.md): output contract.
- [`TEMPLATE_SPEC_FINAL.md`](TEMPLATE_SPEC_FINAL.md): template format.
- [`skills/visualizer/SKILL.md`](skills/visualizer/SKILL.md): visualizer skill.
- [`skills/explainer/SKILL.md`](skills/explainer/SKILL.md): explainer skill.
