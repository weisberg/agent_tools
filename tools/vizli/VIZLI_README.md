# VIZLI_README.md — Agent operating manual

This document tells you, the AI agent, exactly how to use vizli to produce
visualizations. You are operating inside a Claude Code skill. Your job is
to find the right template, resolve its parameters, feed it data, call
`vizli render`, and deliver the output file to the user.

You are NOT creating templates. You are NOT converting raw files. You are
rendering existing templates with user-supplied data and parameters.

---

## 1. Mental model

The vizli workflow from your perspective:

```
User request → Search sidecars → Pick template → Resolve params + data
    → vizli render → Verify output → Deliver file
```

Every step has exactly one thing to do. Don't skip steps, don't combine
them, don't improvise. This document covers each one.

---

## 2. Template discovery via sidecar files

### What sidecar files are

Every vizli template has a companion markdown file that describes it.
The sidecar lives next to the template and shares its name with `.md`
appended:

```
templates/
├── charts/
│   ├── bar_grouped.vizli.json
│   ├── bar_grouped.vizli.json.md      ← sidecar
│   ├── line_timeseries.vizli.json
│   ├── line_timeseries.vizli.json.md  ← sidecar
│   ├── scatter_regression.vizli.json
│   └── scatter_regression.vizli.json.md
├── explainers/
│   ├── ab_test_power.vizli.html
│   ├── ab_test_power.vizli.html.md
│   ├── cuped_variance.vizli.html
│   └── cuped_variance.vizli.html.md
└── diagrams/
    ├── ci_cd_pipeline.vizli.svg
    └── ci_cd_pipeline.vizli.svg.md
```

### Sidecar file structure

Every sidecar has YAML frontmatter and a markdown body:

```markdown
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
formats: [svg, png]
interactive: false
params:
  - name: data_url
    type: url
    required: true
    description: CSV or JSON data source
  - name: x_field
    type: string
    required: true
    description: Column name for x-axis categories
  - name: y_field
    type: string
    required: true
    description: Column name for y-axis values
  - name: color_field
    type: string
    required: true
    description: Column name for grouping/color
  - name: color_scheme
    type: string
    default: tableau10
    description: Vega color scheme name
  - name: title
    type: string
    default: ""
    description: Chart title (empty for no title)
  - name: y_format
    type: string
    default: ""
    description: D3 number format for y-axis (e.g., "$,.0f", ".1%")
data:
  expects: tabular
  required_columns: []
  notes: >
    Columns are specified via params (x_field, y_field, color_field),
    so any tabular data works as long as those columns exist.
---

# Grouped bar chart

## When to use

Use this template when the user wants to compare values across categories,
with a second categorical dimension shown as grouped colored bars. Common
scenarios: revenue by region and quarter, scores by student and subject,
counts by department and status.

## When NOT to use

- For a single category (no grouping) → use `bar_simple.vizli.json`
- For time series data → use `line_timeseries.vizli.json`
- For part-of-whole → use `bar_stacked.vizli.json`
- For more than 6-7 groups → consider a heatmap or faceted chart instead

## Parameter guide

### Required: data source and field mapping

You MUST provide `x_field`, `y_field`, and `color_field`. These are the
column names from the user's data. Inspect the data before rendering to
get the correct column names.

```bash
# Example: user has a CSV with columns: region, quarter, revenue
vizli render charts/bar_grouped.vizli.json \
  --data user_data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region
```

### Optional: formatting and appearance

- `color_scheme`: Any Vega scheme name. Use "category10" for ≤10 groups,
  "tableau20" for 11-20. For sequential data use "blues" or "viridis".
- `title`: Set this to a clear, descriptive title based on the user's
  request. If they didn't specify, infer one from the data.
- `y_format`: Use D3 format strings. Common ones:
  - `"$,.0f"` → currency with commas, no decimals ($12,345)
  - `",.0f"` → integer with commas (12,345)
  - `".1%"` → percentage with one decimal (45.2%)
  - `".2f"` → two decimal places (0.45)

## Example invocations

### Basic: revenue by region and quarter
```bash
vizli render charts/bar_grouped.vizli.json \
  --data quarterly_revenue.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --param title="Quarterly revenue by region" \
  --param y_format="$,.0f" \
  --format png \
  --output report/revenue_chart.png
```

### Dark mode for slides
```bash
vizli render charts/bar_grouped.vizli.json \
  --data data.csv \
  --param x_field=month \
  --param y_field=count \
  --param color_field=status \
  --theme dark \
  --scale 3 \
  --format png
```

## Troubleshooting

- "Invalid field X in encoding" → The column name in your --param doesn't
  match the actual CSV column. Check the CSV headers.
- Bars are too thin → Too many x-axis categories. Consider filtering or
  aggregating the data before rendering.
- Colors are indistinguishable → Too many groups for the color scheme.
  Switch to "tableau20" or reduce group count.
```

### How to search sidecars

Use `grep`, `ripgrep`, or file reading to search sidecar frontmatter. The
search strategy depends on what the user asked for:

**By tag** (most reliable):
```bash
rg -l "tags:" templates/ --include="*.md" | xargs rg -l "bar-chart"
```

**By description keyword**:
```bash
rg -l "grouped|comparison|categorical" templates/ --include="*.md"
```

**By template type**:
```bash
rg -l "^type: vega-lite" templates/ --include="*.md"
```

**By output format**:
```bash
rg -l "interactive: true" templates/ --include="*.md"
```

**By required params** (to match what you have):
```bash
rg -l "required: true" templates/charts/ --include="*.md"
```

### Search priority

When the user's request is ambiguous, search in this order:

1. **Exact tag match** — user says "bar chart" → search `tags` for "bar-chart"
2. **Description match** — user says "compare regions" → search descriptions
3. **Title match** — user says "timeseries" → search titles
4. **Browse by directory** — user says "something for my report" → list
   templates in `charts/` and pick based on their data

If multiple templates match, read their sidecar "when to use" and
"when NOT to use" sections to disambiguate.

---

## 3. Selecting the right template

### Decision tree

```
Is the user asking for interactivity (sliders, toggles, animation)?
├── Yes → Look in explainers/ or interactive/
│         Output format will be HTML
└── No → Is this a standard chart (bar, line, scatter, etc.)?
         ├── Yes → Look in charts/
         │         Match chart type to data shape
         └── No → Is this a diagram (architecture, flow, process)?
                  ├── Yes → Look in diagrams/
                  └── No → Search across all directories by tags
```

### Matching data shape to chart type

| Data shape | Template type |
|---|---|
| 1 categorical + 1 numeric | bar_simple, pie, treemap |
| 1 categorical + 1 numeric + 1 group | bar_grouped, bar_stacked |
| 1 temporal + 1 numeric | line_timeseries |
| 1 temporal + 1 numeric + 1 series | line_multiseries |
| 2 numeric | scatter_simple |
| 2 numeric + 1 categorical | scatter_colored |
| 2 numeric + 1 size | scatter_bubble |
| Matrix / grid | heatmap |
| Hierarchical | treemap, sunburst |
| Flow / connections | sankey, chord |

### Reading the sidecar

Once you've identified a candidate template, read its full sidecar file.
Pay attention to:

1. **"When NOT to use"** — disqualify the template early if it doesn't fit
2. **Required params** — make sure you can supply all of them from the
   user's data and request
3. **Data expectations** — `data.expects` and `data.required_columns`
   tell you what shape the data must be in
4. **Example invocations** — these are your primary reference for building
   the correct command

---

## 4. Inspecting the user's data

Before rendering, always inspect the data the user provides or references.
This is mandatory — never guess column names.

### For CSV files

```bash
head -5 user_data.csv          # see headers + first rows
wc -l user_data.csv            # row count
```

Or in Python:
```python
import pandas as pd
df = pd.read_csv("user_data.csv")
print(df.columns.tolist())     # column names
print(df.dtypes)               # column types
print(df.shape)                # (rows, cols)
print(df.head())               # first rows
```

### For JSON files

```bash
python3 -c "
import json
data = json.load(open('user_data.json'))
if isinstance(data, list):
    print(f'{len(data)} records')
    print(f'Keys: {list(data[0].keys())}')
else:
    print(f'Keys: {list(data.keys())}')
"
```

### Column name matching

The most common rendering failure is a column name mismatch. After
inspecting the data:

1. Map user-facing names to actual column names. If the user says "revenue"
   but the column is `total_revenue_usd`, use the actual column name.
2. Check for case sensitivity. `Region` ≠ `region` in Vega-Lite.
3. Watch for whitespace in CSV headers. `" revenue"` (leading space) will
   cause a silent mismatch. Strip if necessary.

### Data preparation

If the user's data needs reshaping before it fits a template, do the
reshaping yourself before passing it to vizli:

```python
import pandas as pd

df = pd.read_csv("raw_data.csv")

# Example: pivot from wide to long format
df_long = df.melt(
    id_vars=["region"],
    value_vars=["q1", "q2", "q3", "q4"],
    var_name="quarter",
    value_name="revenue"
)
df_long.to_csv("/tmp/vizli_prepared_data.csv", index=False)
```

Then pass the prepared file to vizli:
```bash
vizli render charts/bar_grouped.vizli.json \
  --data /tmp/vizli_prepared_data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region
```

Always clean up temp files after rendering.

---

## 5. Building the vizli render command

### Command structure

```bash
vizli render <template_path> \
  --data <data_file> \
  --param <key>=<value> \    # repeat for each param
  --format <fmt> \           # svg, png, html, pdf
  --output <output_path> \   # where to write the file
  [--theme light|dark] \     # optional theme override
  [--width <px>] \           # optional width override
  [--height <px>] \          # optional height override
  [--scale <n>] \            # optional PNG density (default 2)
  [--bg transparent]         # optional transparent background
```

### Choosing the output format

| User intent | Format | Notes |
|---|---|---|
| "Make me a chart" | png | Default for most requests |
| "I need an SVG" | svg | Vector, scalable, editable |
| "For my presentation" | png --scale 3 | High density for slides |
| "Interactive calculator" | html | Only for html-type templates |
| "For a PDF report" | svg | SVGs embed cleanly in reports |
| "With transparent background" | png --bg transparent | For overlays |
| "Both formats" | svg,png | Comma-separated, produces both |

### Choosing the output path

Write output files to the working directory or a location the user can
access. Use descriptive filenames:

```bash
# Good: descriptive, includes context
--output revenue_by_region_q3.png
--output experiment_power_analysis.html
--output ci_pipeline_diagram.svg

# Bad: generic, no context
--output chart.png
--output output.svg
```

If the user specified a filename, use it. If not, derive one from the
template name and data context.

### Required vs optional params

Read the sidecar's `params` list. For every param with `required: true`,
you MUST supply a `--param` flag. For optional params, supply them only if:

- The user explicitly requested the behavior (e.g., "make it dark mode")
- The default value is wrong for the context (e.g., default title is empty
  but the user clearly wants a title)
- The sidecar's parameter guide recommends it for the data shape

### Param value types

| Param type | How to pass | Example |
|---|---|---|
| string | Bare value | `--param title="Revenue by region"` |
| int | Bare number | `--param bins=20` |
| float | Bare decimal | `--param opacity=0.7` |
| bool | true/false | `--param show_legend=true` |
| color | Hex or name | `--param accent_color="#534AB7"` |
| enum | One of allowed | `--param sort_order=descending` |
| url | URL or file path | `--param data_url=https://...` |
| json | JSON string | `--param config='{"key": "val"}'` |

Quote string values that contain spaces. Quote JSON values with single
quotes on the outside.

---

## 6. Rendering

### The render call

Execute `vizli render` as a shell command. Always capture both stdout
and stderr:

```bash
vizli render charts/bar_grouped.vizli.json \
  --data /tmp/vizli_prepared_data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --param title="Quarterly revenue by region" \
  --param y_format='$,.0f' \
  --param color_scheme=tableau10 \
  --format png \
  --scale 2 \
  --output revenue_by_region.png
```

### Checking the result

After rendering, always verify the output exists and is non-trivial:

```bash
ls -la revenue_by_region.png    # exists? size > 0?
```

For PNGs, check dimensions match expectations:
```bash
python3 -c "
from PIL import Image
img = Image.open('revenue_by_region.png')
print(f'Size: {img.size[0]}x{img.size[1]} px')
"
```

For SVGs, quick sanity check:
```bash
head -3 revenue_by_region.svg   # should start with <?xml or <svg
wc -c revenue_by_region.svg     # should be > 1KB for any real chart
```

For HTML, verify it's a complete document:
```bash
grep -c "</html>" output.html   # should be 1
```

### Dry run

If you're uncertain about the parameters, use `--dry-run` to validate
without rendering:

```bash
vizli render charts/bar_grouped.vizli.json \
  --data data.csv \
  --param x_field=quarter \
  --param y_field=revenue \
  --param color_field=region \
  --dry-run
```

This prints the resolved configuration (all params with their final values,
data shape, output settings) without invoking the rendering engine. Use it
to catch param mismatches before a potentially slow Playwright render.

---

## 7. Error handling

### Exit codes

| Code | Meaning | What to do |
|---|---|---|
| 0 | Success | Proceed to deliver the file |
| 1 | Template or render error | Read stderr, fix the issue, retry |
| 2 | CLI usage error | Fix the command syntax |
| 3 | Missing dependency | Install the needed package |
| 4 | Timeout | Increase `--timeout` or simplify the template |

### Common errors and fixes

**"Unresolved param: {{x_field}}"**
You forgot a required `--param`. Check the sidecar for required params.

**"Invalid field 'reveneu' in encoding"**
Column name typo. Re-inspect the data with `head -1 data.csv` and fix the
`--param` value.

**"Data file not found: data.csv"**
Wrong path. Use an absolute path or verify the relative path from your
current working directory.

**"Timeout: HTML did not signal readiness within 30s"**
The HTML template's JavaScript didn't set `window.__VIZLI_READY__ = true`.
This is a template bug, not your fault. Try `--timeout 60` as a workaround.
If it still fails, report the template as broken.

**"vl-convert error: Cannot read properties of undefined"**
Usually means the Vega-Lite spec references a field that doesn't exist
in the data. Double-check column names.

**"Playwright not installed"**
The template requires a browser engine. Run:
```bash
pip install "vizli[browser]" && playwright install chromium
```

### Retry strategy

1. Read the full error message from stderr
2. Diagnose: is it a param issue, data issue, or template issue?
3. Fix the diagnosed problem
4. Re-run `vizli render` with the fix
5. If the same error persists after 2 retries, tell the user what's
   wrong and ask for guidance

Do NOT retry blindly without changing something. Each retry must address
the specific error.

---

## 8. Delivering the output

### For file output

After a successful render, present the output file to the user:

```python
# Copy to the user-accessible output directory
import shutil
shutil.copy("revenue_by_region.png", "/mnt/user-data/outputs/revenue_by_region.png")
```

Then use `present_files` or the equivalent mechanism to make it available
for download.

### What to tell the user

Keep it brief. The visualization speaks for itself. Say:

- What you rendered and from what data
- Any decisions you made (which template, which params, any data prep)
- Nothing else — don't describe the chart in prose

**Good**: "Here's the quarterly revenue breakdown. I used the grouped bar
chart template with your Q3 data, formatting the y-axis as currency."

**Bad**: "I have created a grouped bar chart visualization showing the
quarterly revenue data broken down by region. The x-axis displays the
quarters (Q1 through Q4) while the y-axis shows revenue in US dollars.
The bars are color-coded by region using the Tableau 10 color scheme,
with East shown in blue, West in orange, and..."

### Multiple outputs

If the template produces both SVG and PNG (as most chart templates do),
present the primary format the user needs and mention the other is
available:

```
Here's the revenue chart as a PNG. I also generated an SVG version if
you need a vector format.
```

---

## 9. Template info shortcut

If you need a quick overview of a template's params without reading the
full sidecar, use `vizli info`:

```bash
vizli info charts/bar_grouped.vizli.json
```

Output:
```
Template: charts/bar_grouped.vizli.json
Type: vega-lite
Formats: svg, png
Size: 800 x 500

Parameters:
  data_url     (url, required)      CSV or JSON data source
  x_field      (string, required)   Column name for x-axis categories
  y_field      (string, required)   Column name for y-axis values
  color_field  (string, required)   Column name for grouping/color
  color_scheme (string, "tableau10") Vega color scheme name
  title        (string, "")         Chart title
  y_format     (string, "")         D3 number format for y-axis
```

This is faster than reading the sidecar, but the sidecar has the "when to
use", examples, and troubleshooting that `vizli info` doesn't.

---

## 10. Full workflow examples

### Example A: user asks "chart my sales data"

**Step 1: Inspect the data**
```bash
head -5 /mnt/user-data/uploads/sales.csv
```
Output: `date,product,units_sold,revenue`

**Step 2: Search for a matching template**
User said "chart" with no specifics. Data has a date column → probably a
timeseries. Also has a product column → multi-series.

```bash
rg -l "timeseries|time-series" templates/ --include="*.md"
```
Finds: `line_timeseries.vizli.json.md`, `line_multiseries.vizli.json.md`

**Step 3: Read the sidecar**
Read `line_multiseries.vizli.json.md`. It expects a temporal field, a
numeric field, and a series field. Perfect match.

**Step 4: Render**
```bash
vizli render charts/line_multiseries.vizli.json \
  --data /mnt/user-data/uploads/sales.csv \
  --param x_field=date \
  --param y_field=revenue \
  --param series_field=product \
  --param title="Sales revenue over time" \
  --param y_format='$,.0f' \
  --format png \
  --scale 2 \
  --output sales_chart.png
```

**Step 5: Verify and deliver**
```bash
ls -la sales_chart.png   # confirm it exists, > 0 bytes
cp sales_chart.png /mnt/user-data/outputs/
```

### Example B: user asks "A/B test power calculator"

**Step 1: Identify as interactive**
"Calculator" → interactive, HTML output. Search explainers.

```bash
rg -l "ab.test|sample.size|power" templates/explainers/ --include="*.md"
```
Finds: `ab_test_power.vizli.html.md`

**Step 2: Read the sidecar**
This template has optional params: `baseline_rate`, `mde`, `power`, `alpha`.
All have sensible defaults. User didn't specify values.

**Step 3: Check if user provided any specifics**
User said "for our email campaign with a 3% click rate." Extract:
`baseline_rate=0.03`.

**Step 4: Render**
```bash
vizli render explainers/ab_test_power.vizli.html \
  --param baseline_rate=0.03 \
  --format html \
  --output ab_test_calculator.html
```

**Step 5: Deliver**
```bash
cp ab_test_calculator.html /mnt/user-data/outputs/
```

### Example C: user asks "architecture diagram for our data pipeline"

**Step 1: No data file involved**
This is a diagram, not a data visualization. Search diagrams.

```bash
rg -l "pipeline|architecture|data.flow" templates/diagrams/ --include="*.md"
```
Finds: `data_pipeline.vizli.svg.md`, `etl_flow.vizli.svg.md`

**Step 2: Read both sidecars, pick best match**
`data_pipeline.vizli.svg.md` has params for source names, destination names,
and processing step labels. Good fit.

**Step 3: Map user's description to params**
User described: "S3 → Spark → Snowflake → Tableau"

```bash
vizli render diagrams/data_pipeline.vizli.svg \
  --param source="S3" \
  --param processor="Spark" \
  --param warehouse="Snowflake" \
  --param viz_tool="Tableau" \
  --param title="Data pipeline architecture" \
  --format svg \
  --output data_pipeline.svg
```

**Step 4: Also produce PNG for convenience**
```bash
vizli render diagrams/data_pipeline.vizli.svg \
  --param source="S3" \
  --param processor="Spark" \
  --param warehouse="Snowflake" \
  --param viz_tool="Tableau" \
  --param title="Data pipeline architecture" \
  --format png \
  --scale 3 \
  --output data_pipeline.png
```

**Step 5: Deliver both**

---

## 11. Rules

1. **Always inspect user data before rendering.** Never guess column names.
2. **Always read the sidecar before calling vizli render.** It has the
   param guide and the example invocations you need.
3. **Always verify output after rendering.** Check that the file exists
   and has a reasonable size.
4. **Never describe a chart in prose.** Show the file, say what you did
   briefly, stop.
5. **Use --dry-run when uncertain.** It's cheap and catches mistakes.
6. **Clean up temp files.** If you created intermediate data files, delete
   them after rendering.
7. **Default to PNG at 2x scale** unless the user asked for something
   specific. It's the safest format for most contexts.
8. **Infer a good title** from the user's request and data when the user
   doesn't specify one. An empty title is almost never the right default.
9. **Prefer the sidecar's example invocations** over improvising. They
   are tested and known to work.
10. **If no template matches, say so.** Don't force a bad-fit template.
    Tell the user what templates are available and ask which is closest.
