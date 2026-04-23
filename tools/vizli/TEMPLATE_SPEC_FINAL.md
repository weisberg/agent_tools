# TEMPLATE_SPEC.md — vizli Template Specification v1.0

## 1. Purpose

This document defines the contract that template authors — human and AI
alike — must follow when creating templates for vizli.

A vizli template is a **deterministic, self-contained visualization package**
that can be rendered by the CLI into one or more output files: SVG, PNG,
HTML, or PDF.

The design goal is brutally practical: templates should be easy for LLM
agents to author, easy for vizli to validate, and easy for the runtime to
render without ad-hoc logic per template.

---

## 2. Core principles

Every vizli template MUST satisfy these rules:

### 2.1 Deterministic

Rendering must not depend on wall-clock time, random numbers, network
state, browser cache, or host-specific environment variables unless those
are explicitly provided in the payload. If randomness is required, it must
be seeded from the payload.

### 2.2 Self-contained

No CDN dependencies. No remote fonts. No remote images, scripts,
stylesheets, data files, or API calls. All assets must be local files
inside the template package or inlined in the template body.

### 2.3 Offline-renderable

A template must render correctly with the network fully disabled. vizli
disables network access in browser contexts by default; templates that
attempt network requests will fail, not degrade.

### 2.4 Payload-driven

All variability must come from a structured payload. Hardcoded values are
allowed only when they are truly invariant design choices — colors that
define the template's identity, layout proportions that are architectural
decisions, not user preferences.

### 2.5 Validation-first

Every template must provide a machine-validatable input schema. Invalid
payloads fail before rendering starts, not halfway through a 30-second
Playwright render.

### 2.6 One obvious entrypoint

The manifest must point to one canonical entry file. vizli should never
need to guess which file to render.

### 2.7 Render, don't improvise

Templates are presentational artifacts, not general applications. Heavy
ETL, data cleaning, and business logic belong upstream. The only
transformations allowed inside a template are those intrinsic to the
visual form: sorting for display, stacking, binning, faceting, derived
labels.

---

## 3. Template formats

vizli supports two template formats: **single-file** for simple templates
and **directory packages** for complex ones. Both are first-class citizens;
vizli detects which format it's dealing with automatically.

### 3.1 Single-file templates

A single file with YAML frontmatter and a visualization body, separated
by `---` fences. Best for agent-authored templates that need no local
assets — most charts, diagrams, and simple explainers.

```
revenue_bars.vizli.json       ← Vega-Lite body
cuped_explainer.vizli.html    ← HTML body
pipeline_diagram.vizli.svg    ← SVG body
scatter_builder.vizli.py      ← Altair/Python body
force_graph.vizli.js          ← D3/JS body
```

The file extension after `.vizli.` determines the body format and provides
editor syntax highlighting. vizli also accepts the bare `.vizli` extension
and reads the `type` field from frontmatter.

Structure:

```
---
# YAML frontmatter (manifest + schema + sample data)
---
(visualization body: JSON, Python, SVG, HTML, or JS)
```

The frontmatter contains everything that would be separate files in a
directory package: manifest metadata, the param schema, sample data, and
rendering config. See §5 for the full schema.

### 3.2 Directory packages

A directory containing a manifest, schema, entrypoint, and optional
assets. Best for templates with local JavaScript, CSS, images, fonts, or
golden reference outputs.

```
monthly-bars/
  vizli.template.json          ← manifest (required)
  schema.json                  ← payload schema (required)
  sample-payload.json          ← example payload (required)
  entry/
    template.vl.json.j2        ← entrypoint
  assets/                      ← local JS, CSS, images, fonts
    helper.js
    styles.css
  golden/                      ← reference outputs for regression
    output.svg
    output.png
```

Required files:
- `vizli.template.json` — manifest
- `schema.json` — payload schema (JSON Schema)
- `sample-payload.json` — canonical example payload
- The entrypoint file declared in the manifest

Optional:
- `assets/` — local dependencies (no remote loading allowed)
- `golden/` — reference outputs for `vizli golden --check`
- `README.md` — author-facing notes (not the sidecar — sidecars
  live outside the package)

### 3.3 Detection rules

vizli determines the format by checking:
1. If the path is a directory containing `vizli.template.json` → directory package
2. If the path is a file with `.vizli` in its name → single-file template
3. Otherwise → error

---

## 4. The canonical payload

vizli normalizes all CLI inputs into one internal payload object with
three namespaces:

```json
{
  "params": {},
  "datasets": {},
  "meta": {}
}
```

### 4.1 `params`

Scalar or structured inputs that control rendering. These are the values
authors expose as configurable.

Examples: title, subtitle, width, height, color palette, label format,
axis limits, column names, boolean feature flags, numeric thresholds.

### 4.2 `datasets`

A dictionary of named datasets. Each key is a semantic name; each value
is an array of objects (tabular rows), a nested object, or a GeoJSON
structure.

```json
{
  "datasets": {
    "revenue": [
      {"region": "East", "quarter": "Q1", "amount": 12000},
      {"region": "West", "quarter": "Q1", "amount": 9800}
    ],
    "targets": [
      {"quarter": "Q1", "target": 11000},
      {"quarter": "Q2", "target": 12500}
    ]
  }
}
```

Named datasets solve the multi-source problem: a chart with actual vs
target data, a network diagram with nodes and edges, a dashboard with
main and benchmark datasets. Single unnamed data blobs don't scale.

Rules for dataset naming:
- Use semantic names that describe the content: `revenue`, `nodes`,
  `edges`, `benchmarks` — not `data1`, `thing`, `df`
- Keep names short, lowercase, snake_case
- Document expected shapes in the schema

### 4.3 `meta`

Runtime-provided metadata injected by vizli. Templates may read `meta`
but should not depend on it unless the behavior is explicitly documented.

Contents (injected by vizli, not by the user):
- `meta.format` — requested output format
- `meta.theme` — resolved theme name
- `meta.width` — resolved output width
- `meta.height` — resolved output height
- `meta.scale` — resolved pixel density
- `meta.template_id` — template identifier
- `meta.vizli_version` — vizli version string

### 4.4 Payload resolution order

vizli resolves the payload from multiple sources, with later sources
overriding earlier ones:

1. Sample payload from template (only when explicitly used as base)
2. `--payload payload.json` (the primary payload file)
3. `--dataset key=file.csv` flags (merged into `datasets`)
4. `--param key=value` flags (merged into `params`)
5. `--params-file config.yaml` (merged into `params`)
6. Explicit `--width`, `--height`, `--theme`, `--scale` flags (merged
   into `meta`)

Later sources win. This means a `--param` flag always overrides the
same key in the payload file.

---

## 5. Frontmatter schema (single-file templates)

For single-file templates, the YAML frontmatter contains all manifest,
schema, and sample data in one block.

### 5.1 Complete example

```yaml
---
spec_version: "1.0"
template_id: charts.revenue-bars
type: vega-lite
title: Grouped bar chart
description: >
  Vertical bars grouped by a categorical field with color encoding.
  Good for comparing values across categories and sub-categories.
tags: [bar-chart, comparison, categorical, grouped]

outputs: [svg, png]

canvas:
  width: 800
  height: 500
  background: white
  scale: 2

schema:
  params:
    title:
      type: string
      default: ""
      description: Chart title
    x_field:
      type: string
      required: true
      description: Column name for x-axis categories
    y_field:
      type: string
      required: true
      description: Column name for y-axis values
    color_field:
      type: string
      required: true
      description: Column name for grouping/color
    color_scheme:
      type: string
      default: tableau10
      enum: [tableau10, category10, tableau20, dark2, viridis, blues]
      description: Vega color scheme name
  datasets:
    table:
      required: true
      description: Primary data table with columns matching field params
      columns: []

sample:
  params:
    title: Revenue by region
    x_field: quarter
    y_field: revenue
    color_field: region
    color_scheme: tableau10
  datasets:
    table:
      - {quarter: Q1, region: East, revenue: 12000}
      - {quarter: Q1, region: West, revenue: 9800}
      - {quarter: Q2, region: East, revenue: 13500}
      - {quarter: Q2, region: West, revenue: 11200}
---
```

### 5.2 Field definitions

#### Identity and discovery

| Field | Type | Required | Description |
|---|---|---|---|
| `spec_version` | string | yes | Always `"1.0"` |
| `template_id` | string | yes | Stable dotted identifier (e.g., `charts.revenue-bars`) |
| `type` | enum | yes | One of: `vega-lite`, `altair`, `svg`, `html`, `js` |
| `title` | string | yes | 3-8 words, sentence case, ≤60 chars |
| `description` | string | yes | 30-120 words describing the visual and use cases |
| `tags` | list | yes | 4-10 lowercase hyphenated keywords |

#### Output and canvas

| Field | Type | Required | Description |
|---|---|---|---|
| `outputs` | list | yes | Supported formats: `svg`, `png`, `html`, `pdf` |
| `canvas.width` | int | yes | Default output width in pixels |
| `canvas.height` | int | yes | Default output height in pixels |
| `canvas.background` | string | no | Background color; default `"white"`, or `"transparent"` |
| `canvas.scale` | float | no | PNG density multiplier; default `2` |

#### Schema (inline)

The `schema` block defines the payload contract. It replaces the separate
`schema.json` file from directory packages.

```yaml
schema:
  params:
    param_name:
      type: string|int|float|bool|enum|color|json
      required: true|false         # if false, must have default
      default: value               # used when param not supplied
      description: "..."           # agent-readable description
      enum: [a, b, c]              # only for type: enum
      min: 0                       # only for int/float
      max: 100                     # only for int/float
      examples: [val1, val2]       # optional example values
  datasets:
    dataset_name:
      required: true|false
      description: "..."
      columns: [col1, col2]        # expected columns (advisory)
```

**Param naming conventions**:
- Use `snake_case`
- Prefix data-related params with `data_` (e.g., `data_url`, `data_path`)
  only when disambiguating against display params
- Prefix color params with `color_` (e.g., `color_scheme`, `color_primary`)
- Use descriptive names: `min_detectable_effect` not `mde`
- Name boolean params as assertions: `show_legend`, `include_gridlines`

#### Sample payload (inline)

The `sample` block provides a canonical example payload for testing and
preview renders. It mirrors the payload structure:

```yaml
sample:
  params: { ... }
  datasets:
    table: [ ... ]
```

Sample data should be 5-20 representative rows — enough to verify
rendering, small enough to not bloat the file.

#### Optional fields

| Field | Type | Description |
|---|---|---|
| `template_version` | string | Semver for the template (e.g., `"1.2.0"`) |
| `interactive` | bool | Whether output supports user interaction |
| `author` | string | Creator name or team |
| `category` | string | Grouping: `charts`, `explainers`, `diagrams`, etc. |
| `runtime.timeout_ms` | int | Max render wait; default `30000` |
| `runtime.ready_event` | string | Custom event name for HTML readiness |
| `runtime.ready_selector` | string | CSS selector that exists when ready |
| `runtime.svg_selector` | string | CSS selector for SVG extraction |
| `runtime.offline_only` | bool | Default `true`; emphasizes no-network rule |

---

## 6. Manifest schema (directory packages)

Directory packages use `vizli.template.json` as the manifest. It contains
the same information as single-file frontmatter but in JSON format, with
schema and sample payload in separate files.

```json
{
  "spec_version": "1.0",
  "template_id": "charts.revenue-bars",
  "template_version": "1.0.0",
  "type": "vega_lite",
  "title": "Grouped bar chart",
  "description": "Vertical bars grouped by a categorical field.",
  "tags": ["bar-chart", "comparison", "categorical"],

  "engine": "vega_lite",
  "entrypoint": "entry/template.vl.json.j2",
  "schema": "schema.json",
  "sample_payload": "sample-payload.json",

  "outputs": ["svg", "png"],
  "canvas": {
    "width": 800,
    "height": 500,
    "background": "white",
    "scale": 2
  },

  "runtime": {
    "offline_only": true,
    "timeout_ms": 30000
  },

  "golden": {
    "svg": "golden/output.svg",
    "png": "golden/output.png"
  },

  "assets_dir": "assets"
}
```

### Manifest-specific fields not in single-file format

| Field | Description |
|---|---|
| `entrypoint` | Relative path to the main template file |
| `schema` | Relative path to `schema.json` |
| `sample_payload` | Relative path to sample payload file |
| `assets_dir` | Directory for local JS, CSS, images; default `"assets"` |
| `golden.svg`, `.png`, `.html` | Paths to golden reference outputs |
| `visual_diff_tolerance` | Pixel tolerance for golden comparisons |
| `inline_assets` | Whether to inline CSS/JS in HTML output |

### Path safety rules

- All paths must be relative to the template root directory
- Paths must not escape the template directory (no `../`)
- Paths must not contain absolute components
- Symlinks are resolved and re-validated
- vizli validates path safety before loading any file

---

## 7. Engine types

Choose the **simplest engine that can faithfully render the visualization**.
When multiple engines could work, prefer them in this order:

1. `vega-lite` — declarative, no browser, fast, testable
2. `altair` — when Python assembly is simpler than raw JSON
3. `svg` — for custom diagrams and vector artwork
4. `html` — for bespoke JS/DOM interactivity
5. `js` — for D3 render functions that need a browser harness

### 7.1 `vega-lite`

**Body format**: JSON conforming to Vega-Lite v5, with Jinja2 placeholders.

**Placeholder style**: Use namespaced references with the `| tojson`
filter for all values injected into JSON context:

```jinja2
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "autosize": {"type": "fit", "contains": "padding"},
  "width": {{ params.width | default(800) }},
  "title": {{ params.title | tojson }},
  "data": {"values": {{ datasets.table | tojson }}},
  "mark": {"type": "bar"},
  "encoding": {
    "x": {
      "field": {{ params.x_field | tojson }},
      "type": "nominal"
    },
    "y": {
      "field": {{ params.y_field | tojson }},
      "type": "quantitative"
    },
    "color": {
      "field": {{ params.color_field | tojson }},
      "scale": {"scheme": {{ params.color_scheme | tojson }}}
    }
  }
}
```

For single-file templates, vizli renders the body as a Jinja2 template
with the resolved payload as context. The `| tojson` filter is mandatory
for any value injected into JSON context — bare `{{ params.title }}`
inside a JSON string is a template bug that breaks on quotes, newlines,
and special characters.

**Data binding**:
- Inline from payload: `"data": {"values": {{ datasets.table | tojson }}}`
- Named Vega dataset: `"data": {"name": "table"}` with dataset injected
  at runtime

**Rules**:
- Use `"autosize": {"type": "fit", "contains": "padding"}` to respect
  canvas dimensions from the manifest
- No external URLs in `data.url`
- No network-dependent features
- Keep transforms declarative (Vega-Lite transforms, not arbitrary JS)
- Prefer Vega-Lite over full Vega unless the design truly requires Vega

**Outputs**: SVG, PNG, HTML, PDF

---

### 7.2 `altair`

**Body format**: Python module that exports a `build_chart` function.

```python
def build_chart(params, datasets, meta):
    import altair as alt
    import pandas as pd

    df = pd.DataFrame(datasets["table"])

    chart = alt.Chart(df).mark_bar().encode(
        x=alt.X(f'{params["x_field"]}:N'),
        y=alt.Y(f'{params["y_field"]}:Q'),
        color=alt.Color(
            f'{params["color_field"]}:N',
            scale=alt.Scale(scheme=params["color_scheme"])
        )
    ).properties(title=params.get("title", ""))

    return chart
```

**Function signature**: `build_chart(params: dict, datasets: dict, meta: dict) -> alt.Chart`

The function receives the three payload namespaces as plain dicts and
returns an Altair chart object. vizli handles theme application, dimension
overrides, and format conversion.

**Rules**:
- Must define `build_chart(params, datasets, meta)` and return an Altair
  chart object
- No import-time rendering, no file writes, no network access
- No global mutable state, no subprocess calls
- No reading undeclared files
- Allowed: programmatic chart construction, field selection logic,
  lightweight derived columns tightly coupled to the visual, layered or
  faceted chart assembly

**Outputs**: SVG, PNG, HTML, PDF

---

### 7.3 `svg`

**Body format**: SVG markup with Jinja2 placeholders. Root element must be
`<svg>` with `xmlns` and `viewBox` attributes.

```svg
<svg xmlns="http://www.w3.org/2000/svg"
     viewBox="0 0 {{ canvas.width | default(800) }} {{ canvas.height | default(500) }}"
     width="100%">
  <style>
    .bar { fill: {{ params.fill_color | default('#534AB7') }}; }
    svg[data-theme="dark"] .bar { fill: {{ params.fill_color_dark | default('#7B6FDB') }}; }
    svg[data-theme="dark"] .bg { fill: #1a1a18; }
    svg[data-theme="light"] .bg { fill: #FAFAF8; }
  </style>

  <rect class="bg" width="100%" height="100%"/>

  {% for item in datasets.table %}
  <rect class="bar"
        x="{{ item.x }}" y="{{ item.y }}"
        width="{{ item.w }}" height="{{ item.h }}"/>
  {% endfor %}

  <text x="20" y="30" class="title">{{ params.title }}</text>
</svg>
```

**Sizing**: The SVG must use `viewBox="0 0 W H"` matching the canvas
dimensions. Set `width="100%"` for responsive scaling. vizli sets explicit
`width` and `height` attributes at render time for PNG conversion.

**Rules**:
- Root `<svg>` element with `xmlns` and `viewBox`
- No external CSS, remote images, or `<script>` tags that make network
  requests
- Use stable `id` and `class` attributes for important groups
- No `<script>` blocks unless the template also declares HTML output and
  readiness signals
- All text must use system fonts (no custom font dependencies in
  single-file templates)
- Colors should use CSS rules with `data-theme` selectors or be
  parameterized for theme support

**Dark mode**: Include CSS rules keyed to the `data-theme` attribute that
vizli injects on the root `<svg>` element:

```svg
<style>
  svg[data-theme="dark"] .bg { fill: #1a1a18; }
  svg[data-theme="dark"] .text-primary { fill: #e8e6dc; }
  svg[data-theme="light"] .bg { fill: #FAFAF8; }
  svg[data-theme="light"] .text-primary { fill: #2C2C2A; }
</style>
```

**Data-driven SVGs**: For complex data binding that requires JavaScript,
use the `html` engine with embedded SVG instead.

**Outputs**: SVG, PNG, PDF

---

### 7.4 `html`

**Body format**: Complete HTML document, self-contained, all assets local.

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{{ params.title }}</title>
  <style>
    :root {
      --bg: #FAFAF8; --text: #2C2C2A; --muted: #888780;
      --border: rgba(0,0,0,0.12);
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg: #1a1a18; --text: #e8e6dc; --muted: #9c9a92;
        --border: rgba(255,255,255,0.12);
      }
    }
    body { background: var(--bg); color: var(--text); margin: 0; }
    /* ... all CSS inline ... */
  </style>
</head>
<body>
  <div id="viz">
    <svg viewBox="0 0 680 400"><!-- visual content --></svg>
  </div>

  <script>
    const params = {{ params | tojson }};
    const datasets = {{ datasets | tojson }};
    const meta = {{ meta | tojson }};

    // rendering logic ...

    window.__VIZLI_READY__ = true;
  </script>
</body>
</html>
```

**Rules**:
- Must render as a standalone offline document
- All JS and CSS must be inline or in local asset files (no CDN)
- Must expose a readiness signal (see §8)
- No `fetch`, `XMLHttpRequest`, WebSockets, service workers, or
  browser storage dependencies
- No infinite animations that never settle
- No time-based layouts that depend on race conditions
- The `<title>` should reflect the template's `title` field

**Output modes**:
- `outputs: [html]` — vizli writes the interpolated HTML file directly.
  Interactive controls, animations, and JS all work.
- `outputs: [png]` or `outputs: [svg, png]` — vizli opens the HTML in a
  headless browser, waits for rendering, and screenshots. The template
  must signal readiness.

**Outputs**: HTML, PNG (via screenshot), optionally SVG (via extraction),
PDF (via print-to-PDF)

---

### 7.5 `js`

**Body format**: JavaScript module defining a `render` function.

```javascript
function render(container, params, datasets, meta) {
  const width = meta.width || 800;
  const height = meta.height || 500;

  const svg = d3.select(container)
    .append('svg')
    .attr('viewBox', `0 0 ${width} ${height}`);

  const data = datasets.table;

  // D3 rendering logic ...

  const x = d3.scaleBand()
    .domain(data.map(d => d[params.x_field]))
    .range([60, width - 20])
    .padding(0.2);

  const y = d3.scaleLinear()
    .domain([0, d3.max(data, d => d[params.y_field])])
    .range([height - 40, 20]);

  svg.selectAll('.bar')
    .data(data)
    .enter()
    .append('rect')
    .attr('class', 'bar')
    .attr('x', d => x(d[params.x_field]))
    .attr('y', d => y(d[params.y_field]))
    .attr('width', x.bandwidth())
    .attr('height', d => height - 40 - y(d[params.y_field]))
    .attr('fill', params.fill_color || '#534AB7');

  window.__VIZLI_READY__ = true;
}
```

vizli wraps the JS module in an HTML harness with D3 v7 loaded locally.
The function receives a container DOM element and the three payload
namespaces, then renders into the container.

**Available globals**: `d3` (v7), `__VIZLI_PARAMS__`, `__VIZLI_DATASETS__`,
`__VIZLI_DATA__` (alias for the first dataset), `__VIZLI_META__`. These
are also passed as function arguments — prefer the arguments over the
globals.

**Rules**:
- Must define `render(container, params, datasets, meta)`
- D3 v7 is available as a global; no other libraries unless bundled
  locally in a directory package
- Must set readiness signal when rendering is complete
- No network requests, no file access

**Outputs**: SVG (extracted from DOM), PNG (via screenshot)

---

## 8. Readiness and extraction signals

HTML and JS templates require explicit signals so vizli knows when
rendering is complete and how to extract output.

### 8.1 Readiness resolution order

vizli checks for readiness in this order, stopping at the first match:

1. **Custom event**: If `runtime.ready_event` is declared, vizli listens
   for that event on `document`.
2. **CSS selector**: If `runtime.ready_selector` is declared, vizli polls
   for that selector to appear in the DOM.
3. **Window flag**: vizli polls for `window.__VIZLI_READY__ === true`.
4. **Timeout**: If none of the above resolve within `runtime.timeout_ms`,
   rendering fails with a `TimeoutError`.

There is no fallback delay. "Sleep 2 seconds and pray" is not a rendering
strategy.

**Implementation**:

```javascript
// The simplest and most common approach — set the flag after init:
window.__VIZLI_READY__ = true;

// For animated visualizations, fire after the first frame renders,
// NOT after all animations complete:
requestAnimationFrame(() => {
  window.__VIZLI_READY__ = true;
});

// For async data processing within the template:
processData(datasets).then(result => {
  renderChart(result);
  window.__VIZLI_READY__ = true;
});
```

### 8.2 SVG extraction

For HTML templates that declare `svg` in their outputs, vizli needs to
extract the SVG from the rendered page. Resolution order:

1. **Selector**: If `runtime.svg_selector` is declared, vizli queries that
   selector and reads its `outerHTML`.
2. **Getter function**: If the page defines `window.__VIZLI_GET_SVG__()`,
   vizli calls it and expects an SVG string.
3. **Auto-detect**: vizli queries `document.querySelector('svg')` for the
   first SVG element.

If no SVG can be found and the template declares `svg` output, vizli
reports an `ExtractionError`.

### 8.3 Requirements by engine

| Engine | Readiness required? | SVG extraction required? |
|---|---|---|
| `vega-lite` | No (native render) | No (native render) |
| `altair` | No (native render) | No (native render) |
| `svg` | No (static markup) | No (the output IS SVG) |
| `html` | Yes | Only if `svg` is in `outputs` |
| `js` | Yes | Yes (vizli extracts from DOM) |

---

## 9. Templating rules

### 9.1 Jinja2 conventions

For text-based templates (`.json.j2`, `.svg.j2`, `.html.j2`, or inline
bodies in single-file templates), vizli renders with Jinja2 in strict
undefined mode. Referencing an undefined variable is an error, not a
silent empty string.

**Namespaced access**: Always use the payload namespaces:
```jinja2
{{ params.title }}
{{ datasets.table | tojson }}
{{ meta.width }}
```

**JSON safety**: Use `| tojson` for any value injected into JSON context.
Bare `{{ params.title }}` without `tojson` inside a JSON string is a
template bug — it breaks on quotes, newlines, and special characters.

**Allowed Jinja features**:
- Variable insertion
- Loops (`{% for %}`)
- Conditionals (`{% if %}`)
- Filters (`tojson`, `default`, `int`, `float`, `round`)
- Small macros if needed

**Avoid**:
- Deeply nested template logic
- Building giant JSON fragments as concatenated strings
- Metaprogramming or dynamic template inclusion
- Side-effecting filters or extensions

### 9.2 Authoring constraints

Authors MUST NOT:
- Depend on undeclared variables
- Emit malformed JSON, XML, or HTML
- Use templating when a plain static value is correct
- Duplicate the same parameter under several names

Authors SHOULD:
- Keep variable names short but descriptive
- Expose only true degrees of freedom — parameterize what changes,
  hardcode what doesn't
- Provide defaults in the schema for optional params
- Use `tojson` filter for all JSON-context insertions

---

## 10. Data handling

### 10.1 Named datasets

Use named datasets in the payload, not anonymous blobs.

Good:
```json
{
  "datasets": {
    "revenue": [...],
    "targets": [...],
    "annotations": [...]
  }
}
```

Bad:
```json
{
  "datasets": {
    "data1": [...],
    "thing": [...]
  }
}
```

### 10.2 Transformation boundaries

Allowed inside templates:
- Sorting for display
- Aggregation needed by the chart grammar (Vega-Lite transforms)
- Derived labels and display fields
- Stacking, binning, faceting, filtering tied to the visual form

Push upstream (before the payload reaches vizli):
- Heavy joins
- Deduplication
- Fuzzy matching
- Business logic
- Geocoding
- Large graph layout computations

### 10.3 Size discipline

Templates should not force giant inline data blobs when a smaller
filtered or aggregated dataset would do. Sample data in templates
should be 5-20 rows — enough to verify the visual, small enough to
keep the template file readable.

### 10.4 Supported data formats

vizli loads data files into dataset objects. Supported formats:

| Format | Detection | Loaded as |
|---|---|---|
| JSON | `.json` extension or starts with `[`/`{` | Parsed JSON |
| CSV | `.csv` extension | Array of row objects |
| TSV | `.tsv` extension | Array of row objects |
| Parquet | `.parquet` extension | Array of row objects |
| NDJSON | `.ndjson` extension | Array of objects |

NaN/Inf handling: configurable per-template. Default is to convert NaN
to null and error on Inf.

---

## 11. Styling and layout

### 11.1 Fixed dimensions by default

Templates must declare default width and height in the canvas config.
Responsive behavior is allowed in HTML output, but deterministic fixed
defaults are required for SVG and PNG output.

Sizing guidelines:
- Standard chart → 800×500
- Dashboard or wide visualization → 1200×700
- Mobile or narrow visualization → 400×600
- Tall infographic or diagram → 800×1200

### 11.2 Fonts

Use system font stacks. Do not rely on network font loading. If a
specific font is required, bundle it locally in the `assets/` directory
of a directory package.

```css
font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
```

### 11.3 Theme support

Templates should support light and dark themes. The mechanism depends on
the engine:

**Vega-Lite / Altair**: vizli injects a Vega config object with theme
colors. The template doesn't need to handle theming itself.

**SVG**: Use a `data-theme` attribute on the root `<svg>` and provide CSS
rules for both modes:
```svg
<style>
  svg[data-theme="light"] .bg { fill: #FAFAF8; }
  svg[data-theme="dark"] .bg { fill: #1a1a18; }
  svg[data-theme="light"] .text-primary { fill: #2C2C2A; }
  svg[data-theme="dark"] .text-primary { fill: #e8e6dc; }
</style>
```

**HTML**: Use CSS custom properties with `@media (prefers-color-scheme)`
or read `meta.theme` in JavaScript to set the appropriate class.

**JS**: Read `meta.theme` from the payload to select colors. vizli also
sets Playwright's `color_scheme` so `prefers-color-scheme` media queries
work if the harness includes CSS.

### 11.4 Color system

vizli provides a standard color palette available to all themes:

**Light mode**:
```
--bg: #FAFAF8  --bg2: #F1EFE8  --text: #2C2C2A  --muted: #888780
--border: rgba(0,0,0,0.12)
```

**Dark mode**:
```
--bg: #1a1a18  --bg2: #2C2C2A  --text: #e8e6dc  --muted: #9c9a92
--border: rgba(255,255,255,0.12)
```

**Categorical ramps**: purple (#534AB7), teal (#0F6E56), coral (#D85A30),
amber (#BA7517), blue (#185FA5), green (#639922), red (#A32D2D).

Templates are not required to use these colors, but the theme system
provides them for consistency across a collection.

### 11.5 Accessibility

Where practical:
- Provide title and description fields
- Include text alternatives in HTML output (`<title>`, `aria-label`)
- Preserve readable contrast ratios (minimum 4.5:1 for text)
- Avoid encoding key distinctions with color alone (add patterns,
  labels, or shapes)

---

## 12. Security

Templates may be authored by AI agents. This is a sentence that should
make any sensible engineer sit up a little straighter.

### 12.1 Security rules

- **No network**: templates must not make network requests. vizli blocks
  network access in browser contexts.
- **No path traversal**: template paths must not escape the template root.
  vizli validates all paths before loading.
- **No shell execution**: templates must not spawn processes.
- **No arbitrary Python execution**: the `altair` engine calls a single
  function with structured arguments; it does not `exec()` arbitrary code.
  Import scope is restricted.
- **No unsafe XML parsing**: SVG templates are parsed with defenses
  against entity expansion attacks.
- **No environment variables**: templates must not read host environment
  variables unless explicitly passed through the payload.
- **No persistent state**: templates must not write to disk, localStorage,
  cookies, or any storage mechanism.

### 12.2 Containment

- Strict path resolution — all file access relative to template root
- Constrained import path for Python template modules
- Temporary working directories for rendering
- Browser context isolation (ephemeral, non-persistent contexts)
- Safe XML parsing (defusedxml)
- Console errors captured and surfaced (not swallowed)
- Attempted network access logged as warnings

---

## 13. Sidecar files

Every template has a sidecar markdown file for agent-facing discovery and
documentation. Sidecars are defined in SIDECAR_SPEC.md and live alongside
the template:

```
bar_grouped.vizli.json          ← template
bar_grouped.vizli.json.md       ← sidecar

monthly-bars/                   ← directory package
  vizli.template.json
  ...
monthly-bars.md                 ← sidecar (outside the package)
```

Sidecars are the ONLY file agents read during template selection. The
template file itself is never read for selection purposes. A template
without a sidecar is invisible to agents.

See SIDECAR_SPEC.md for the full sidecar format.

---

## 14. File naming and organization

### 14.1 Single templates

```
revenue-bars.vizli.json          # vega-lite (editor highlights as JSON)
cuped-variance.vizli.html        # html type (editor highlights as HTML)
ci-pipeline.vizli.svg            # svg type
scatter-builder.vizli.py         # altair (editor highlights as Python)
force-graph.vizli.js             # js type (editor highlights as JS)
```

The double extension (`.vizli.<format>`) serves two purposes: it tells
vizli what body format to expect, and it gives editors correct syntax
highlighting. The bare `.vizli` extension is also accepted — vizli reads
the `type` field from frontmatter.

### 14.2 Template collections

```
templates/
├── charts/
│   ├── bar-grouped.vizli.json
│   ├── bar-grouped.vizli.json.md
│   ├── line-timeseries.vizli.json
│   ├── line-timeseries.vizli.json.md
│   └── scatter-regression.vizli.json
├── explainers/
│   ├── ab-test-power.vizli.html
│   ├── ab-test-power.vizli.html.md
│   ├── cuped-variance.vizli.html
│   └── gradient-descent.vizli.html
├── diagrams/
│   ├── ci-cd-pipeline.vizli.svg
│   └── data-architecture.vizli.svg
└── dashboards/
    └── executive-summary/          ← directory package
        ├── vizli.template.json
        ├── schema.json
        ├── sample-payload.json
        ├── entry/
        │   └── dashboard.html.j2
        ├── assets/
        │   ├── chart-utils.js
        │   └── styles.css
        └── golden/
            └── output.png
```

Every template should have a sidecar file alongside it. Templates without
sidecars are invisible to agent-driven selection.

### 14.3 Template registry (optional)

A `vizli-templates.yaml` file at the collection root enables discovery
and batch operations:

```yaml
spec_version: "1.0"
templates:
  - path: charts/bar-grouped.vizli.json
    title: Grouped bar chart
    tags: [bar, comparison, categorical]
  - path: explainers/ab-test-power.vizli.html
    title: A/B test power calculator
    tags: [statistics, ab-test, interactive]
  - path: dashboards/executive-summary
    title: Executive summary dashboard
    tags: [dashboard, kpi, quarterly]
```

---

## 15. LLM conversion procedure

When an LLM agent converts a raw visualization sample into a vizli
template, it follows this sequence.

### Step 1 — Inspect the sample

Determine: source format, visual structure, data dependencies, interactive
behaviors, output requirements.

Classify the input:

| Input looks like | Template type |
|---|---|
| JSON with `$schema` containing `vega-lite` | `vega-lite` |
| Python code importing `altair` or using `alt.Chart` | `altair` |
| Raw `<svg>` element (no HTML wrapper) | `svg` |
| Full HTML document with `<script>` and interactivity | `html` |
| JavaScript with D3 calls or a `render()` function | `js` |

If the sample mixes types (e.g., HTML with an embedded Vega-Lite spec),
prefer the outer container type (`html`).

### Step 2 — Choose the engine

Use the simplest engine that works (§7). Default preference order:
vega-lite → altair → svg → html → js.

### Step 3 — Separate constants from parameters

Scan the sample for hardcoded values that a user would want to change:

- **Data sources**: URLs, file paths, inline data arrays → extract to
  datasets in the payload
- **Labels and titles**: chart titles, axis labels → string params
- **Visual properties**: color schemes, font sizes, opacity → typed params
- **Thresholds and constants**: statistical parameters, breakpoints →
  numeric params
- **Feature flags**: whether to show a legend, gridlines, annotations →
  bool params

Convert only meaningful variation into `params`. Keep pure design
constants hardcoded. Parameterize what changes between renders; hardcode
what defines the template's identity.

Not everything that CAN vary SHOULD vary. Chaos is not flexibility.

### Step 4 — Design the payload

Create the `params`, `datasets`, and optional `meta` expectations. Name
datasets semantically. Define required vs optional params with defaults.

### Step 5 — Write the schema

Define types, required fields, defaults, constraints, and enums. The
schema should catch errors that would otherwise only surface during
rendering.

### Step 6 — Build the entrypoint

Create the template file using Jinja2 placeholders (for text-based
engines) or the `build_chart`/`render` function (for Python/JS engines).

For `vega-lite` and `svg`: replace extracted values with `{{ params.x }}`
and `{{ datasets.table | tojson }}` Jinja2 expressions.

For `altair`: reference params via `params["x_field"]` in the
`build_chart` function. Access datasets via `datasets["table"]`.

For `html`: use Jinja2 `{{ params | tojson }}` in `<script>` blocks to
make the full payload available to JavaScript.

For `js`: reference `params.x_field` and `datasets.table` from the
`render` function arguments.

### Step 7 — Add runtime metadata

Define readiness hooks, SVG extraction config, and supported outputs.
For HTML and JS templates, choose the readiness signal mechanism.

### Step 8 — Create the sample payload

Provide one clean, representative example with 5-20 data rows. The sample
should exercise all params (including optional ones with non-default
values) and produce a visually complete rendering.

### Step 9 — Check determinism

Remove network access, randomness, undeclared dependencies, and
time-based assumptions. Ask: would this render identically on a different
machine with the network disabled?

### Step 10 — Validate dark mode

For `svg` and `html` types:
- Ensure all colors use CSS custom properties or `data-theme` selectors
- Include dark mode overrides
- Mentally test: if the background were near-black, is every text element
  still readable?

For `vega-lite` and `altair`: dark mode is handled by vizli's rendering
engine via theme config injection. No action needed in the template.

### Step 11 — Validate

Run the acceptance checklist (§16):
- Can vizli find the entrypoint?
- Can it validate the payload?
- Can it render offline?
- Can it know when rendering is done?
- Can it extract the requested output types?

If any answer is "sort of," the template is not ready.

### Step 12 — Write the sidecar

Create the sidecar markdown file following SIDECAR_SPEC.md. This step is
not optional — a template without a sidecar will never be used.

---

## 16. Acceptance checklist

A template is acceptable only if ALL are true:

- [ ] `spec_version: "1.0"` is present
- [ ] `template_id` is set and follows dotted naming convention
- [ ] `type` correctly matches the body format
- [ ] Manifest/frontmatter is valid and complete
- [ ] Schema is valid and covers all params and datasets
- [ ] Required params have `required: true`, optional params have `default`
- [ ] Sample payload validates against the schema
- [ ] Sample data is 5-20 rows and representative
- [ ] Entrypoint renders without undefined variables
- [ ] Body is valid for its type (parseable JSON, valid SVG XML,
      syntactically correct Python/JS, well-formed HTML)
- [ ] Offline rendering works (no network requests attempted)
- [ ] Requested outputs are actually produced
- [ ] Output dimensions match canvas defaults unless overridden
- [ ] HTML/JS templates signal readiness
- [ ] Templates claiming SVG output can actually yield SVG
- [ ] Dark mode support is present for svg/html types
- [ ] No console errors during render
- [ ] No external dependencies except D3 v7 (loaded by vizli harness)
- [ ] File paths do not escape the template root
- [ ] Sidecar file exists, is valid, and matches the template
- [ ] Golden outputs pass (if golden/ directory exists)

---

## 17. Template inheritance (reserved)

Reserved for a future spec version. Templates will be able to extend a
base template:

```yaml
extends: charts/base-timeseries.vizli.json
overrides:
  params:
    color_scheme:
      default: "dark2"
  canvas:
    width: 1200
```

This is not yet implemented. Do not use `extends` in v1.0 templates.

---

## 18. Summary

A good vizli template is:
- **Simple** — expose only real degrees of freedom
- **Deterministic** — same payload, same output, every time
- **Schema-driven** — invalid inputs fail before rendering starts
- **Offline** — no network, no CDN, no remote anything
- **Discoverable** — sidecar file makes it findable by agents
- **Testable** — golden outputs catch regressions
- **Themed** — light and dark mode support out of the box
- **Boring** — in all the ways production systems should be boring

That last point is not an insult. It is a compliment with steel in it.
