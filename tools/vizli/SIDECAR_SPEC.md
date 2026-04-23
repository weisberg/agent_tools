# SIDECAR_SPEC.md — Sidecar file specification v0.1

## Purpose

Every vizli template has a sidecar markdown file. The sidecar is the
template's documentation, discovery surface, and operating manual. It is
the ONLY file an AI agent reads when deciding which template to use and
how to use it. The template file itself is never read during selection.

This means the sidecar must be complete, accurate, and self-contained. If
something is true about the template but absent from the sidecar, it does
not exist from the agent's perspective.

This document specifies exactly how to write sidecar files — the YAML
frontmatter schema, the markdown body structure, the writing standards,
and the validation rules.

---

## 1. File naming and location

The sidecar file shares the template's full filename with `.md` appended:

```
bar_grouped.vizli.json      → bar_grouped.vizli.json.md
ab_test_power.vizli.html    → ab_test_power.vizli.html.md
ci_pipeline.vizli.svg       → ci_pipeline.vizli.svg.md
scatter_regression.vizli.js → scatter_regression.vizli.js.md
cuped_explainer.vizli.py    → cuped_explainer.vizli.py.md
```

The sidecar MUST live in the same directory as its template. A template
without a sidecar is invisible to the agent and will never be selected.

---

## 2. File structure

Every sidecar has two parts separated by the `---` fence:

```markdown
---
# YAML frontmatter (structured metadata for search and selection)
---

# Markdown body (human/agent-readable operating manual)
```

Both parts are required. A sidecar with only frontmatter or only a markdown
body is invalid.

---

## 3. YAML frontmatter schema

### 3.1 Required fields

Every sidecar MUST include all of the following:

```yaml
---
template: charts/bar_grouped.vizli.json
type: vega-lite
title: Grouped bar chart
description: >
  Vertical bars grouped by a categorical field with color encoding.
  Good for comparing values across categories and sub-categories.
tags:
  - bar-chart
  - comparison
  - categorical
  - grouped
formats:
  - svg
  - png
interactive: false
params:
  - name: x_field
    type: string
    required: true
    description: Column name for x-axis categories
---
```

**Field definitions:**

#### `template` (string, required)

Relative path from the templates root to the template file. This is the
path the agent passes to `vizli render`.

```yaml
# Correct — relative from templates root
template: charts/bar_grouped.vizli.json
template: explainers/ab_test_power.vizli.html
template: diagrams/ci_pipeline.vizli.svg

# Wrong — absolute path
template: /home/user/projects/vizli/templates/charts/bar_grouped.vizli.json

# Wrong — just the filename (ambiguous if templates are in subdirectories)
template: bar_grouped.vizli.json
```

#### `type` (enum, required)

The template's rendering type. Must be one of:

| Value | Template body format |
|---|---|
| `vega-lite` | JSON Vega-Lite v5 spec |
| `altair` | Python script producing an Altair chart |
| `svg` | Raw SVG markup |
| `html` | Complete HTML document with CSS/JS |
| `js` | JavaScript module with D3 render function |

This field is used by agents to filter templates by rendering capability
(e.g., "does this environment have Playwright?" → exclude `html` and `js`
types if not).

#### `title` (string, required)

A concise, descriptive name for the visualization. This is the primary
human-readable identifier. Rules:

- Sentence case ("Grouped bar chart", not "Grouped Bar Chart")
- No articles at the start ("Grouped bar chart", not "A grouped bar chart")
- 3-8 words, never more than 60 characters
- Describe the visual form, not the data ("Line chart with annotations",
  not "Revenue over time chart")
- Must be unique across all templates in the collection

```yaml
# Good titles
title: Grouped bar chart
title: Multi-series line chart
title: Interactive A/B test power calculator
title: Horizontal waterfall chart
title: Data pipeline architecture diagram

# Bad titles
title: Chart                          # too vague
title: A Beautiful Grouped Bar Chart  # title case, adjective fluff
title: bar_grouped                    # filename, not a title
title: Grouped bar chart for comparing quarterly revenue by region
  across multiple business segments   # too long, too specific
```

#### `description` (string, required)

A 1-3 sentence description of what this visualization does, what it looks
like, and what it's good for. This is the primary search target for
keyword-based discovery. Rules:

- First sentence: what the visualization IS (form + encoding)
- Second sentence: what it's GOOD FOR (use cases, data scenarios)
- Optional third sentence: a distinguishing detail or constraint
- Use plain language, not jargon
- Include synonyms and related terms that a user might search for
- 30-120 words

```yaml
# Good description
description: >
  Vertical bars grouped side-by-side by a categorical field, with color
  encoding for the grouping variable. Good for comparing values across
  categories when there are 2-6 sub-groups per category. Works well for
  revenue by region and quarter, scores by student and subject, or any
  two-dimensional categorical comparison.

# Bad description — too terse, no search surface
description: A bar chart with groups.

# Bad description — too long, rambling
description: >
  This template creates a grouped bar chart visualization using the
  Vega-Lite grammar of graphics specification. It supports multiple
  categorical dimensions on the x-axis with color encoding on the
  secondary categorical field. The bars are rendered vertically with
  automatic spacing calculated by the Vega-Lite layout engine. You can
  customize the color scheme, axis labels, title, and number formatting.
  The template accepts CSV or JSON data and produces SVG or PNG output.
  It was created by the analytics team in Q3 2025 and has been tested
  with datasets up to 10,000 rows. Performance may degrade with larger
  datasets due to the client-side rendering approach used by Vega-Lite.
  For very large datasets, consider pre-aggregating the data before
  passing it to the template.
```

#### `tags` (list of strings, required)

Searchable keywords for template discovery. Tags are the primary
mechanism agents use to find templates. Rules:

- Lowercase, hyphenated (`bar-chart`, not `Bar Chart` or `bar_chart`)
- 4-10 tags per template
- Include at least one tag from each of these categories:
  - **Chart type**: `bar-chart`, `line-chart`, `scatter-plot`, `heatmap`,
    `pie-chart`, `treemap`, `sankey`, `histogram`, `box-plot`, etc.
  - **Use case**: `comparison`, `trend`, `distribution`, `composition`,
    `correlation`, `flow`, `hierarchy`, `geographic`, etc.
  - **Data shape**: `categorical`, `temporal`, `numeric`, `grouped`,
    `stacked`, `multi-series`, `matrix`, etc.
- Do NOT include generic tags like `chart`, `visualization`, `data` — they
  match everything and help nothing
- Do NOT duplicate words already in the title or description — tags are
  for terms the agent might search that AREN'T in those fields

```yaml
# Good tags for a grouped bar chart
tags:
  - bar-chart
  - grouped
  - comparison
  - categorical
  - side-by-side
  - multi-group

# Bad tags
tags:
  - chart           # too generic
  - good            # meaningless
  - vega-lite       # that's the `type` field
  - bar_chart       # wrong format (underscore)
  - Revenue         # data-specific, not template-specific
```

#### `formats` (list of strings, required)

The output formats this template supports. Must be a subset of:
`svg`, `png`, `html`, `pdf`.

```yaml
# Static chart
formats: [svg, png]

# Interactive explainer
formats: [html, png]

# SVG diagram (PNG via cairosvg)
formats: [svg, png]

# Interactive only (no static capture possible)
formats: [html]
```

The first format in the list is the **default** — what vizli produces when
the agent doesn't specify `--format`.

#### `interactive` (boolean, required)

Whether the output supports user interaction (sliders, toggles, buttons,
hover effects). This is a hard filter: agents searching for interactive
templates will filter on `interactive: true`.

```yaml
# Static chart
interactive: false

# Calculator with sliders
interactive: true

# SVG diagram with no controls
interactive: false

# Diagram with clickable nodes (but no state changes)
interactive: false
```

The bar is genuine interactivity that changes the visual state. Hover
tooltips alone do NOT make a template interactive. Clickable nodes that
navigate elsewhere (but don't change the diagram) are NOT interactive.

#### `params` (list of Param objects, required)

Every configurable parameter of the template, fully documented. This is
the most critical section of the frontmatter — an agent that cannot
understand the params cannot use the template.

**Param object schema:**

```yaml
params:
  - name: x_field              # (string, required) snake_case identifier
    type: string               # (enum, required) see type table below
    required: true             # (bool, required) must the agent supply this?
    description: >             # (string, required) what this param controls
      Column name for the x-axis categories. Must match an exact column
      name in the provided data file.
    default: null              # (any, required if required=false) fallback value
    enum_values: null          # (list, optional) allowed values for type=enum
    min: null                  # (number, optional) minimum for int/float
    max: null                  # (number, optional) maximum for int/float
    examples:                  # (list, optional) 2-3 example values
      - "quarter"
      - "product_category"
      - "region"
```

**Param type table:**

| Type | Python equivalent | Example values | Notes |
|---|---|---|---|
| `string` | str | `"revenue"`, `"Q3 Report"` | Free text or column name |
| `int` | int | `20`, `800` | Whole numbers |
| `float` | float | `0.7`, `2.5` | Decimal numbers |
| `bool` | bool | `true`, `false` | Feature toggles |
| `url` | str | `"data.csv"`, `"https://..."` | File path or URL |
| `color` | str | `"#534AB7"`, `"steelblue"` | CSS color value |
| `enum` | str | `"ascending"` | Must include `enum_values` |
| `json` | dict/list | `'{"key": "val"}'` | Complex nested config |

**Param field rules:**

- `name`: Must be snake_case. Must be unique within the template. Must
  match the `{{name}}` placeholder in the template body exactly.
- `type`: Must be one of the types in the table above.
- `required`: If `true`, the agent MUST supply this param via `--param`.
  If `false`, the `default` field MUST be present and non-null.
- `description`: 1-3 sentences. First sentence says what the param
  controls. Subsequent sentences give guidance on valid values. If the
  param is a column name, say so explicitly ("Must match an exact column
  name in the provided data file.").
- `default`: The value used when the param is not supplied. Must be a
  valid value for the param's type. For string params that default to
  empty, use `""` not `null`.
- `examples`: Optional but strongly recommended for string params,
  especially column-name params where the agent needs to understand the
  kind of value expected.

**Param ordering:**

List params in this order:
1. Required params, in the order they logically relate to each other
   (data source first, then field mappings, then labels)
2. Optional params, from most commonly changed to least

### 3.2 Optional frontmatter fields

```yaml
data:
  expects: tabular             # tabular | json-object | none
  required_columns: []         # columns that MUST exist regardless of params
  notes: >                     # free text about data expectations
    Data should have one row per observation. Columns are specified via
    params, so any CSV with the right column names works.
  sample_rows: 5               # how many sample rows the template embeds
category: charts               # top-level category for directory organization
author: analytics-team         # who created or last updated this template
version: "1.0"                 # template version (semver string)
last_updated: "2025-11-15"     # ISO date of last update
related:                       # pointers to similar templates
  - template: charts/bar_simple.vizli.json
    relationship: simpler alternative
  - template: charts/bar_stacked.vizli.json
    relationship: alternative for part-of-whole
```

**Field definitions for optional fields:**

#### `data` (object, optional)

Describes the data the template expects. Helps agents validate data
before rendering and prepare it if needed.

- `expects`: One of `tabular` (CSV/DataFrame rows), `json-object`
  (nested JSON structure), or `none` (template needs no external data,
  e.g., a static diagram). Default: `tabular`.
- `required_columns`: Columns that must exist in the data regardless of
  param values. Use this when the template hardcodes certain column
  references rather than parameterizing them. If all column references
  are parameterized, leave this as `[]`.
- `notes`: Free text about data shape, constraints, preprocessing
  requirements. Be specific: "One row per day per region" is helpful,
  "Must be valid data" is useless.
- `sample_rows`: How many sample rows are embedded in the template's
  `data.sample` frontmatter for preview rendering. Helps agents know
  if a preview is possible without external data.

#### `category` (string, optional)

The subdirectory or logical group this template belongs to. Used for
browsing when tag search is too broad.

Values: `charts`, `explainers`, `diagrams`, `dashboards`, `maps`,
`tables`, or a custom category.

#### `related` (list, optional)

Pointers to other templates that an agent should consider. Each entry
has a `template` path and a `relationship` description. This helps agents
navigate when the first template isn't quite right.

```yaml
related:
  - template: charts/bar_simple.vizli.json
    relationship: Use when there's only one category (no grouping)
  - template: charts/bar_stacked.vizli.json
    relationship: Use for part-of-whole instead of side-by-side comparison
  - template: charts/bar_horizontal.vizli.json
    relationship: Use when category labels are long text strings
```

---

## 4. Markdown body structure

The markdown body follows the frontmatter and serves as the detailed
operating manual. It has a fixed section structure. Every section is
required unless marked (optional).

### 4.1 Required sections

```markdown
# {Title}

## When to use

## When NOT to use

## Parameter guide

## Example invocations

## Troubleshooting
```

### 4.2 Section specifications

---

#### `# {Title}`

Repeat the title from frontmatter as an H1. This is the only H1 in the
file. It anchors the document and is displayed when the agent reads the
sidecar.

```markdown
# Grouped bar chart
```

---

#### `## When to use`

A concise description of the scenarios where this template is the right
choice. Written as 2-5 bullet points or a short paragraph. Focus on the
DATA SHAPE and USER INTENT, not the visual form.

```markdown
## When to use

Use this template when the user wants to compare values across categories,
with a second categorical dimension shown as grouped colored bars. Common
scenarios:

- Revenue by region and quarter
- Test scores by student and subject
- Headcount by department and employment type
- Any comparison where there are 2-6 sub-groups per category
```

**Rules:**
- Lead with the user intent, not the chart mechanics
- Give 3-5 concrete data scenarios as examples
- Mention the grouping constraint (e.g., "2-6 sub-groups")
- Keep to 40-80 words

---

#### `## When NOT to use`

Equally important as "when to use." Lists the scenarios where this
template is wrong and points to the correct alternative. Written as
bullet points with arrows to alternatives.

```markdown
## When NOT to use

- For a single category with no grouping → use `bar_simple.vizli.json`
- For time series data (dates on x-axis) → use `line_timeseries.vizli.json`
- For part-of-whole (percentages summing to 100%) → use `bar_stacked.vizli.json`
- For more than 6-7 groups → consider `heatmap.vizli.json` instead
- For long category labels → use `bar_horizontal.vizli.json`
```

**Rules:**
- Every bullet must name a specific alternative template
- Use the arrow syntax: `→ use template_name`
- 3-6 bullets, covering the most common mismatches
- Include the most tempting mistake first (the one agents make most often)

---

#### `## Parameter guide`

Detailed documentation for every parameter, organized into logical groups
with usage guidance. This is the longest section and the one agents
reference most heavily.

Structure the parameter guide with H3 subheadings grouping related params:

```markdown
## Parameter guide

### Data source and field mapping (required)

You MUST provide `x_field`, `y_field`, and `color_field`. These are the
column names from the user's data. Inspect the data before rendering to
get the correct column names — never guess.

**`x_field`** (string, required)
The column name for x-axis categories. Typically a categorical or nominal
field like `quarter`, `product`, `department`.

**`y_field`** (string, required)
The column name for y-axis values. Must be numeric. If the column contains
formatted strings like "$1,234", the user's data needs to be cleaned first.

**`color_field`** (string, required)
The column name for the grouping/color dimension. Each unique value becomes
a colored group of bars. Works best with 2-6 unique values.

### Labels and titles (optional)

**`title`** (string, default: "")
Chart title displayed above the visualization. If the user doesn't specify
a title, infer one from the data context. An empty title produces no title
element — this is almost never what the user wants.

**`y_format`** (string, default: "")
D3 number format string for y-axis tick labels. Common formats:
- `"$,.0f"` → currency with commas, no decimals ($12,345)
- `",.0f"` → integer with commas (12,345)
- `".1%"` → percentage with one decimal (45.2%)
- `".2f"` → two decimal places (0.45)
- `""` → automatic formatting (Vega-Lite decides)

### Appearance (optional)

**`color_scheme`** (string, default: "tableau10")
Vega color scheme name. Recommendations:
- 2-10 categorical groups: `"tableau10"` (default) or `"category10"`
- 11-20 groups: `"tableau20"`
- Sequential/ordered data: `"blues"`, `"viridis"`, `"plasma"`
- Diverging data: `"redblue"`, `"redyellowblue"`
```

**Rules for the parameter guide:**
- Group params under H3 headings by function (data, labels, appearance,
  behavior, etc.)
- Mark each group as (required) or (optional) in the heading
- Use bold param name + type + default on one line, then description below
- For string params that accept specific values, list the common options
- For numeric params, state the valid range and what extreme values do
- For column-name params, always say "Inspect the data before rendering"
- For format-string params, provide a table of common patterns
- Never assume the agent knows D3 format strings, Vega scheme names, or
  CSS color syntax — spell out the options

---

#### `## Example invocations`

Complete, tested `vizli render` commands that an agent can copy and adapt.
This is the most directly useful section — agents pattern-match against
these when building their own commands.

```markdown
## Example invocations

### Basic: revenue by region and quarter
```bash
vizli render charts/bar_grouped.vizli.json \
  --data quarterly_revenue.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --param title="Quarterly revenue by region" \
  --param y_format='$,.0f' \
  --format png \
  --output revenue_chart.png
```

### Minimal: only required params
```bash
vizli render charts/bar_grouped.vizli.json \
  --data data.csv \
  --param x_field=month \
  --param y_field=count \
  --param color_field=status
```

### Dark mode for presentation slides
```bash
vizli render charts/bar_grouped.vizli.json \
  --data data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --param title="Revenue breakdown" \
  --theme dark \
  --scale 3 \
  --format png \
  --output slide_chart.png
```

### SVG for embedding in a report
```bash
vizli render charts/bar_grouped.vizli.json \
  --data data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --format svg \
  --output report/revenue.svg
```

### Transparent background for overlay
```bash
vizli render charts/bar_grouped.vizli.json \
  --data data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --bg transparent \
  --format png \
  --output overlay.png
```
```

**Rules for example invocations:**
- Include 3-6 examples covering different scenarios
- Every example must have an H3 title describing the scenario
- The first example should be the most common use case with all
  recommended params (not just the minimum)
- The second example should be the minimal case (only required params)
- Include at least one example for each output format the template supports
- Include a dark mode example if the template supports themes
- Use realistic, descriptive filenames (not `output.png`)
- Every example must be a valid, tested command that produces correct output
- Use `\` line continuations for readability — one flag per line

---

#### `## Troubleshooting`

Common errors agents encounter with this template and how to fix them.
Written as problem → cause → fix triplets.

```markdown
## Troubleshooting

**"Invalid field 'reveneu' in encoding"**
Cause: The column name in `--param y_field=reveneu` doesn't match the
actual CSV column. This is almost always a typo.
Fix: Run `head -1 data.csv` to see the exact column names and correct the
param value.

**Bars are too thin or overlapping**
Cause: Too many x-axis categories (>15) or too many color groups (>8).
Fix: Pre-aggregate the data to reduce categories, or switch to
`heatmap.vizli.json` which handles dense grids better.

**Colors are indistinguishable**
Cause: Too many groups for the default "tableau10" scheme (>10 unique
values in the color field).
Fix: Use `--param color_scheme=tableau20` for up to 20 groups. If you
have more than 20, the data needs to be grouped or filtered.

**Empty chart (renders but shows nothing)**
Cause: Data is empty after loading, or all values are null/NaN.
Fix: Inspect the data file: `wc -l data.csv` (should be >1) and
`head -5 data.csv` (values should be present, not empty strings).

**Title is missing**
Cause: The `title` param defaults to empty string.
Fix: Always supply `--param title="Descriptive title"` unless the user
explicitly asked for no title.
```

**Rules for troubleshooting:**
- 4-8 entries covering the most common failures
- Use the exact error message text when possible (agents will grep for it)
- Every entry must have a cause AND a fix
- Fixes must be actionable commands, not vague advice
- Include at least one entry about data issues (empty data, wrong columns)
- Include at least one entry about visual issues (too dense, wrong colors)

### 4.3 Optional sections

These sections are included when relevant to the template:

---

#### `## Data preparation` (optional)

Include this section when the template commonly requires data
transformation before rendering. Provide copy-paste Python snippets.

```markdown
## Data preparation

This template expects long-format data (one row per observation). If your
data is in wide format (one column per series), reshape it first:

```python
import pandas as pd

df = pd.read_csv("wide_data.csv")
df_long = df.melt(
    id_vars=["date"],
    value_vars=["product_a", "product_b", "product_c"],
    var_name="product",
    value_name="revenue"
)
df_long.to_csv("/tmp/vizli_data.csv", index=False)
```

Then render with:
```bash
vizli render charts/line_multiseries.vizli.json \
  --data /tmp/vizli_data.csv \
  --param x_field=date \
  --param y_field=revenue \
  --param series_field=product
```
```

**When to include:**
- Template expects long-format but users commonly have wide-format data
- Template needs date parsing or type coercion
- Template needs a derived column (e.g., profit margin from revenue and cost)
- Template needs filtered data (e.g., exclude nulls, top N only)

---

#### `## Visual reference` (optional)

A text description of what the output looks like, for agents that cannot
see images. Describe the layout, the encoding channels, and the overall
visual form.

```markdown
## Visual reference

The output is a vertical bar chart with bars grouped side-by-side. The
x-axis shows categorical labels, the y-axis shows numeric values, and
each group of bars is colored by the `color_field`. A legend appears at
the right side mapping colors to group names. The chart has a light gray
grid on the y-axis and no grid on the x-axis. The title, if provided,
appears centered above the chart.
```

**When to include:**
- Always include for complex or unusual visualizations
- Optional for standard chart types (bar, line, scatter) where the form
  is obvious from the title

---

#### `## Limitations` (optional)

Known constraints, performance boundaries, or edge cases.

```markdown
## Limitations

- Maximum ~15 x-axis categories before labels start overlapping. For more
  categories, consider `bar_horizontal.vizli.json`.
- Maximum ~8 color groups before the chart becomes hard to read.
- Very large datasets (>10,000 rows) may render slowly in Vega-Lite.
  Pre-aggregate if possible.
- Negative values are supported but render below the x-axis baseline,
  which may confuse users unfamiliar with diverging bar charts.
```

---

#### `## Changelog` (optional)

Version history for templates that evolve over time.

```markdown
## Changelog

- **1.2** (2025-12-01): Added `y_format` param for number formatting
- **1.1** (2025-11-15): Fixed dark mode axis label colors
- **1.0** (2025-10-01): Initial template
```

---

## 5. Writing standards

### 5.1 Voice and tone

Write for an AI agent that is competent but unfamiliar with this specific
template. The agent is under time pressure and will scan, not read in
full. Therefore:

- Lead every section with the most important information
- Use imperative voice: "Use this template when..." not "This template
  can be used when..."
- Be direct: "You MUST provide x_field" not "It is necessary to provide
  the x_field parameter"
- Be specific: "Use `$,.0f` for currency" not "Use an appropriate format"
- Never assume knowledge of Vega-Lite internals, D3 format strings, or
  CSS details — spell out every option the agent might need

### 5.2 Formatting rules

- Use fenced code blocks for all commands, code, and file contents
- Use `bash` language tag for shell commands
- Use `python` language tag for Python snippets
- Use `yaml` language tag for YAML examples
- Use bold for param names in the parameter guide: `**x_field**`
- Use inline code for param values, column names, and format strings
- Use H2 (`##`) for main sections, H3 (`###`) for subsections
- Never use H1 except for the title
- Never use H4 or deeper — if you need that level of nesting, restructure

### 5.3 Length guidelines

| Section | Target length | Hard maximum |
|---|---|---|
| Frontmatter | 30-80 lines | 120 lines |
| When to use | 40-80 words | 120 words |
| When NOT to use | 3-6 bullets | 8 bullets |
| Parameter guide | 100-300 words | 500 words |
| Example invocations | 3-6 examples | 8 examples |
| Troubleshooting | 4-8 entries | 10 entries |
| Total file | 150-350 lines | 500 lines |

If a sidecar exceeds 500 lines, it's trying to do too much. Split complex
templates into variants with their own sidecars.

### 5.4 Keyword density for search

The sidecar is a search surface. The agent finds templates by grepping
sidecars for keywords from the user's request. To maximize discoverability:

- Include synonyms in the description: "bar chart" AND "column chart"
- Include data-domain terms in "when to use": "revenue", "sales", "scores"
- Use the tags field for visual-form synonyms the user might type
- Repeat important terms naturally (don't keyword-stuff, but don't use a
  synonym when the original term would help search)

### 5.5 Self-containment rule

The sidecar must be fully self-contained. An agent reading ONLY the
sidecar must be able to:

1. Decide whether this template fits the user's request
2. Know every required and optional param, with valid values
3. Construct a correct `vizli render` command
4. Diagnose and fix common errors

Never say "see the template file for details" or "refer to the Vega-Lite
documentation." If the agent needs to know it, it must be in the sidecar.

---

## 6. Validation rules

A sidecar file is valid if and only if ALL of the following are true:

### Frontmatter validation

- [ ] `template` field is present and points to an existing file
- [ ] `type` is one of: `vega-lite`, `altair`, `svg`, `html`, `js`
- [ ] `title` is present, 3-8 words, sentence case, ≤60 characters
- [ ] `description` is present, 30-120 words
- [ ] `tags` is present with 4-10 entries, all lowercase hyphenated
- [ ] `formats` is present with 1-4 entries, all valid format names
- [ ] `interactive` is present and is a boolean
- [ ] `params` is present (may be empty list `[]` for no-param templates)
- [ ] Every param has `name`, `type`, `required`, and `description`
- [ ] Every param with `required: false` has a `default` value
- [ ] Every param `name` is snake_case and unique
- [ ] Every param `type` is one of the valid type enum values

### Body validation

- [ ] H1 title matches frontmatter `title`
- [ ] `## When to use` section is present
- [ ] `## When NOT to use` section is present with template alternatives
- [ ] `## Parameter guide` section is present and covers every param
- [ ] `## Example invocations` section has ≥3 examples
- [ ] Every example is a valid `vizli render` command
- [ ] Every example uses the correct template path from frontmatter
- [ ] `## Troubleshooting` section has ≥4 entries
- [ ] Total file is ≤500 lines

### Cross-validation with template

- [ ] Every `{{param_name}}` in the template body has a matching entry
      in the sidecar's `params` list
- [ ] The sidecar's `type` matches the template's frontmatter `type`
- [ ] The sidecar's `formats` matches the template's `output.formats`
- [ ] The sidecar's `template` path resolves to the actual template file

---

## 7. LLM authoring instructions

When an LLM agent is tasked with writing a sidecar for a new or existing
template, follow this process:

### Step 1: Read the template

Read the template file completely. Extract:
- The `type` and `output` fields from the template frontmatter
- Every `{{param_name}}` placeholder in the body
- Any hardcoded values that should be documented as non-configurable
  behavior
- The data binding pattern (`__INLINE_DATA__`, `__VIZLI_DATA__`, etc.)

### Step 2: Write the frontmatter

Fill in every required field following the schema in §3. For params:
- List every `{{param_name}}` you found in the template
- Determine whether each is required or optional (is there a default?)
- Write a clear description for each, including valid values
- Add 2-3 examples for string params

### Step 3: Write "When to use"

Think about the data shapes and user intents this template serves. Write
3-5 concrete scenarios. Be specific — name the kind of data (revenue,
scores, counts) and the comparison being made.

### Step 4: Write "When NOT to use"

For each common misuse, identify the correct alternative template. If you
don't know the exact alternative template filename, use a descriptive
placeholder like `a stacked bar chart template` — but always provide the
redirection.

### Step 5: Write the parameter guide

Organize params into logical groups. For each param:
- State what it controls in one sentence
- List valid values or common patterns
- Note any constraints or gotchas
- For column-name params, always include: "Must match an exact column
  name in the provided data file."

### Step 6: Write example invocations

Create at least:
1. A full example with all recommended params and realistic filenames
2. A minimal example with only required params
3. A dark mode / high-DPI example
4. An alternative output format example (SVG if default is PNG, etc.)

Test each example mentally: does it include all required params? Is the
template path correct? Are the flag names correct?

### Step 7: Write troubleshooting

Anticipate the most common errors:
- Column name mismatches (always include this one)
- Missing required params
- Data shape mismatches (wide vs long, wrong types)
- Visual issues specific to this chart type (too many categories, etc.)
- Empty or null data

### Step 8: Validate

Run through the validation checklist in §6. Every box must be checked
before the sidecar is complete.

---

## 8. Complete example

Here is a full, valid sidecar file for reference:

```markdown
---
template: charts/scatter_regression.vizli.json
type: vega-lite
title: Scatter plot with regression line
description: >
  Points plotted on x/y axes with an optional linear regression trend line
  overlay. Good for showing correlation between two numeric variables,
  identifying outliers, and visualizing the strength and direction of a
  relationship.
tags:
  - scatter-plot
  - regression
  - correlation
  - trend-line
  - bivariate
  - numeric
formats: [svg, png]
interactive: false
params:
  - name: x_field
    type: string
    required: true
    description: >
      Column name for the x-axis numeric variable. Must match an exact
      column name in the provided data file.
    examples: ["hours_studied", "marketing_spend", "temperature"]
  - name: y_field
    type: string
    required: true
    description: >
      Column name for the y-axis numeric variable. Must match an exact
      column name in the provided data file.
    examples: ["test_score", "revenue", "ice_cream_sales"]
  - name: color_field
    type: string
    required: false
    default: ""
    description: >
      Optional column name for color-coding points by category. Leave
      empty for a single-color scatter. If provided, each unique value
      gets a distinct color and a legend entry.
    examples: ["region", "grade_level", ""]
  - name: show_regression
    type: bool
    required: false
    default: true
    description: >
      Whether to overlay a linear regression trend line. Set to false
      for a plain scatter plot without the trend.
  - name: title
    type: string
    required: false
    default: ""
    description: >
      Chart title. If empty, no title is displayed. Infer a descriptive
      title from the data context when the user doesn't specify one.
  - name: point_size
    type: int
    required: false
    default: 60
    description: >
      Size of each scatter point in pixels squared. Default 60 works for
      most datasets. Increase to 100-150 for small datasets (<50 points),
      decrease to 30-40 for large datasets (>500 points).
    min: 10
    max: 300
  - name: opacity
    type: float
    required: false
    default: 0.7
    description: >
      Point opacity from 0 (transparent) to 1 (opaque). Lower values
      help when points overlap. Default 0.7 works for most cases.
    min: 0.1
    max: 1.0
  - name: color_scheme
    type: string
    required: false
    default: "tableau10"
    description: >
      Vega color scheme for the color_field encoding. Only applies when
      color_field is set.
data:
  expects: tabular
  required_columns: []
  notes: >
    Requires at least two numeric columns. Non-numeric values in x_field
    or y_field columns will cause a rendering error.
related:
  - template: charts/scatter_bubble.vizli.json
    relationship: Use when a third numeric variable should encode point size
  - template: charts/scatter_matrix.vizli.json
    relationship: Use for pairwise comparison of 3+ numeric variables
---

# Scatter plot with regression line

## When to use

Use this template when the user wants to see the relationship between two
numeric variables. Common scenarios:

- Study hours vs test scores (education)
- Marketing spend vs revenue (business)
- Temperature vs ice cream sales (classic stats example)
- Experience years vs salary (HR analytics)
- Any bivariate correlation analysis

## When NOT to use

- For three numeric dimensions (x, y, size) → use `scatter_bubble.vizli.json`
- For pairwise comparison of 3+ variables → use `scatter_matrix.vizli.json`
- For time series (date on x-axis) → use `line_timeseries.vizli.json`
- For categorical x-axis → use `bar_simple.vizli.json` or `box_plot.vizli.json`
- For >5,000 points → consider a hexbin or density plot instead

## Parameter guide

### Data field mapping (required)

**`x_field`** (string, required)
Column name for the horizontal axis. Must be numeric. Inspect the data
first: `head -1 data.csv`.

**`y_field`** (string, required)
Column name for the vertical axis. Must be numeric.

### Grouping (optional)

**`color_field`** (string, default: "")
Set this to a categorical column name to color-code points by group. Leave
as empty string `""` for a uniform scatter. Works best with 2-6 groups.

### Regression line (optional)

**`show_regression`** (bool, default: true)
Controls whether the linear regression trend line is displayed. Disable
with `--param show_regression=false` for a plain scatter.

### Appearance (optional)

**`title`** (string, default: "")
Chart title. Always provide one unless the user explicitly wants no title.

**`point_size`** (int, default: 60)
Adjust based on dataset size:
- Small datasets (<50 points): `100-150`
- Medium (50-500): `60` (default)
- Large (>500): `30-40`

**`opacity`** (float, default: 0.7)
Reduce to `0.3-0.5` when points heavily overlap. Increase to `1.0` for
small, non-overlapping datasets.

**`color_scheme`** (string, default: "tableau10")
Only applies when `color_field` is set. Use `"tableau10"` for categorical
groups, `"viridis"` for ordered groups.

## Example invocations

### Full example: study hours vs test scores by grade
```bash
vizli render charts/scatter_regression.vizli.json \
  --data student_data.csv \
  --param x_field=hours_studied \
  --param y_field=test_score \
  --param color_field=grade_level \
  --param title="Study hours vs test scores" \
  --param point_size=80 \
  --format png \
  --output study_correlation.png
```

### Minimal: two numeric columns
```bash
vizli render charts/scatter_regression.vizli.json \
  --data data.csv \
  --param x_field=x \
  --param y_field=y
```

### Without regression line
```bash
vizli render charts/scatter_regression.vizli.json \
  --data data.csv \
  --param x_field=spend \
  --param y_field=revenue \
  --param show_regression=false \
  --param title="Marketing spend vs revenue"
```

### Dense dataset with transparency
```bash
vizli render charts/scatter_regression.vizli.json \
  --data large_dataset.csv \
  --param x_field=feature_1 \
  --param y_field=feature_2 \
  --param point_size=25 \
  --param opacity=0.3 \
  --param title="Feature correlation" \
  --format svg \
  --output correlation.svg
```

### Dark mode for slides
```bash
vizli render charts/scatter_regression.vizli.json \
  --data data.csv \
  --param x_field=hours \
  --param y_field=score \
  --param title="Correlation analysis" \
  --theme dark \
  --scale 3 \
  --format png
```

## Troubleshooting

**"Invalid field 'hours' in encoding"**
Cause: Column name mismatch. The CSV might have `hours_studied` not `hours`.
Fix: Run `head -1 data.csv` and use the exact column name.

**Points are invisible or missing**
Cause: The x or y column contains non-numeric values (strings, dates, nulls).
Fix: Inspect with `python3 -c "import pandas as pd; print(pd.read_csv('data.csv')[['x_col','y_col']].dtypes)"`.
Clean non-numeric rows before rendering.

**Regression line looks flat or wrong**
Cause: One of the columns has very low variance, or there's no real
correlation. This is not a bug — the line is correct for the data.
Fix: No fix needed. If the user expected a trend, the data may not support it.

**Too many overlapping points**
Cause: Large dataset with tight clustering.
Fix: Reduce `point_size` to 25-30 and `opacity` to 0.3-0.4. If still
unreadable, pre-sample the data or suggest a density/hexbin plot.

**Legend takes too much space**
Cause: `color_field` has too many unique values (>8).
Fix: Filter the data to the most important groups, or remove `color_field`
and use a single color.
```

---

## 9. Quick reference checklist for sidecar authors

Before submitting a sidecar, verify:

- [ ] File is named `<template_filename>.md` and in the same directory
- [ ] YAML frontmatter has all 7 required fields
- [ ] Every param in the template has a matching sidecar param entry
- [ ] Every required param is marked `required: true`
- [ ] Every optional param has a `default` value
- [ ] "When to use" has 3-5 concrete data scenarios
- [ ] "When NOT to use" has 3-6 bullets with alternative template pointers
- [ ] "Parameter guide" covers every param with valid values spelled out
- [ ] "Example invocations" has ≥3 tested commands
- [ ] First example is a full, common use case (not the minimal case)
- [ ] "Troubleshooting" has ≥4 entries including a column-name mismatch
- [ ] Total file is ≤500 lines
- [ ] No references to external docs ("see Vega-Lite docs" is not allowed)
