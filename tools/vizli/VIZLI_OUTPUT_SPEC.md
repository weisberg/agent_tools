# VIZLI_OUTPUT_SPEC.md — vizli CLI & Rendering Pipeline v0.1

## Overview

**vizli** is a Python CLI that reads vizli templates (as defined in
TEMPLATE_SPEC.md), resolves parameters and data, renders the visualization
through the appropriate engine, and writes output files (SVG, PNG, HTML, PDF).

This document specifies the CLI interface, the rendering pipeline architecture,
the Python dependencies, and the output behavior for each template type.

---

## 1. CLI interface

### Installation

```bash
pip install vizli

# Or for development:
git clone <repo> && cd vizli
pip install -e ".[dev]"
```

### Core command: `vizli render`

```bash
vizli render <template> [options]
```

**Arguments:**

| Arg | Description |
|---|---|
| `<template>` | Path to a `.vizli` template file |

**Options:**

| Flag | Type | Default | Description |
|---|---|---|---|
| `-o, --output` | path | `./<stem>.<fmt>` | Output file path or directory |
| `-f, --format` | enum | from template | Override output format(s): `svg`, `png`, `html`, `pdf` |
| `-d, --data` | path | from template | Data file (CSV, JSON, Parquet, NDJSON, TSV) |
| `-p, --param` | key=val | — | Set a template parameter (repeatable) |
| `--params-file` | path | — | YAML/JSON file with parameter values |
| `-w, --width` | int | from template | Override output width |
| `-h, --height` | int | from template | Override output height |
| `--scale` | float | from template | PNG pixel density multiplier |
| `--theme` | enum | from template | `light`, `dark`, or `auto` |
| `--bg` | string | from template | Background color or `transparent` |
| `--timeout` | int | 30 | Rendering timeout in seconds |
| `--watch` | flag | false | Re-render on template or data file change |
| `--preview` | flag | false | Open output in default viewer after render |
| `--dry-run` | flag | false | Validate template and print resolved config |
| `--verbose` | flag | false | Verbose logging |
| `--quiet` | flag | false | Suppress non-error output |

**Parameter precedence** (highest to lowest):
1. `--param key=value` CLI flags
2. `--params-file` contents
3. Template frontmatter `params[].default` values

**Examples:**

```bash
# Render with defaults, output SVG + PNG
vizli render charts/revenue.vizli.json

# Override data and a param, output PNG only
vizli render charts/revenue.vizli.json \
  --data q3_actuals.csv \
  --param color_scheme=dark2 \
  --format png \
  --output report/revenue_q3.png

# Render interactive HTML explainer
vizli render explainers/ab-test-power.vizli.html \
  --format html \
  --param baseline_rate=0.12

# Batch render with a params file
vizli render charts/revenue.vizli.json \
  --params-file configs/q3_report.yaml \
  --output report/

# Watch mode for iterative development
vizli render explainers/cuped.vizli.html --watch --preview

# Dark mode PNG at 3x density
vizli render diagrams/pipeline.vizli.svg \
  --theme dark --scale 3 --format png
```

### Utility commands

```bash
# Validate a template without rendering
vizli validate <template>

# List params and their defaults
vizli info <template>

# Render all templates in a directory
vizli render-all <dir> [--format png] [--output <dir>]

# Convert a raw file to a vizli template (calls LLM)
vizli convert <input_file> [--type vega-lite|altair|svg|html|js]

# Preview in browser (for html type, starts a local server)
vizli serve <template> [--port 8400]
```

---

## 2. Architecture

### High-level pipeline

```
┌──────────┐    ┌───────────┐    ┌──────────┐    ┌──────────┐
│  Parse    │───→│  Resolve  │───→│  Render  │───→│  Output  │
│  template │    │  params + │    │  engine  │    │  writer  │
│           │    │  data     │    │          │    │          │
└──────────┘    └───────────┘    └──────────┘    └──────────┘
```

### Module structure

```
vizli/
├── __init__.py
├── __main__.py              # entry point
├── cli.py                   # Click-based CLI definition
├── config.py                # Pydantic models for frontmatter
├── parser.py                # Template file parsing (YAML + body)
├── resolver.py              # Param + data resolution
├── engines/
│   ├── __init__.py          # EngineRegistry, dispatch
│   ├── vegalite.py          # Vega-Lite → SVG via vl-convert
│   ├── altair_engine.py     # Altair → SVG via altair + vl-convert
│   ├── svg_engine.py        # SVG interpolation + cairosvg
│   ├── html_engine.py       # HTML → screenshot via Playwright
│   ├── js_engine.py         # JS harness → Playwright
│   └── pdf_engine.py        # SVG/HTML → PDF via weasyprint or Playwright
├── data/
│   ├── __init__.py
│   ├── loader.py            # CSV/JSON/Parquet/NDJSON loader
│   ├── transforms.py        # Filter, derive, aggregate (pandas)
│   └── schema.py            # JSON Schema validation for data
├── themes/
│   ├── __init__.py
│   ├── light.py             # Light theme config for Vega/Altair
│   ├── dark.py              # Dark theme config
│   └── base.py              # Shared theme constants
├── output/
│   ├── __init__.py
│   ├── writer.py            # File writing, directory creation
│   └── optimizer.py         # SVG optimization (svgo), PNG compression
├── convert/
│   ├── __init__.py
│   └── llm_converter.py     # LLM-based raw → template conversion
└── watch.py                 # File watcher for --watch mode
```

### Key design decisions

**Pydantic for config validation.** The frontmatter schema is defined as
Pydantic v2 models. This gives us type validation, default resolution, and
helpful error messages when a template is malformed.

**Engine registry pattern.** Each template type maps to an engine class.
The registry dispatches based on the `type` field. This makes it trivial
to add new engines later.

**Playwright for browser rendering.** HTML and JS templates require a real
browser for layout, CSS, and JavaScript execution. Playwright (chromium)
is the rendering backend. It's installed as an optional dependency
(`vizli[browser]`) since Vega-Lite and SVG templates don't need it.

**vl-convert for Vega-Lite.** The `vl-convert-python` package compiles
Vega-Lite specs to SVG/PNG natively (no browser needed). This is fast,
deterministic, and runs in CI without Playwright.

---

## 3. Rendering engines

### 3.1 Vega-Lite engine (`vegalite.py`)

**Input**: JSON string (Vega-Lite v5 spec with interpolated params).

**Pipeline**:
```
JSON string → param interpolation → vl-convert → SVG string
                                                → PNG bytes (if requested)
```

**Dependencies**: `vl-convert-python`

**Implementation**:
```python
import vl_convert as vlc

class VegaLiteEngine:
    def render_svg(self, spec_json: str, width: int, height: int,
                   theme: str, scale: float) -> str:
        vl_spec = json.loads(spec_json)
        vl_spec["width"] = width
        vl_spec["height"] = height

        theme_config = load_theme(theme)  # from themes/
        svg = vlc.vegalite_to_svg(
            vl_spec=json.dumps(vl_spec),
            config=json.dumps(theme_config)
        )
        return svg

    def render_png(self, spec_json: str, width: int, height: int,
                   theme: str, scale: float) -> bytes:
        vl_spec = json.loads(spec_json)
        vl_spec["width"] = width
        vl_spec["height"] = height

        theme_config = load_theme(theme)
        png = vlc.vegalite_to_png(
            vl_spec=json.dumps(vl_spec),
            config=json.dumps(theme_config),
            scale=scale
        )
        return png
```

**Theme application**: vizli injects a Vega config object that sets
background, font, axis styles, and color schemes. The `themes/` module
defines light and dark configs.

**Error handling**: vl-convert reports spec errors with line numbers. vizli
surfaces these as structured errors pointing to the template body.

---

### 3.2 Altair engine (`altair_engine.py`)

**Input**: Python source code string.

**Pipeline**:
```
Python source → inject params as locals → exec() → chart object
    → chart.to_dict() → vl-convert → SVG/PNG
```

**Dependencies**: `altair`, `pandas`, `vl-convert-python`

**Implementation**:
```python
import altair as alt
import pandas as pd

class AltairEngine:
    def execute_template(self, source: str, params: dict,
                         data: Optional[pd.DataFrame]) -> alt.Chart:
        namespace = {
            "alt": alt,
            "pd": pd,
            "data": data,
            **params  # inject params as local variables
        }
        exec(source, namespace)

        chart = namespace.get("chart")
        if chart is None:
            raise VizliError("Altair template must assign to `chart`")
        return chart

    def render_svg(self, source: str, params: dict,
                   data: Optional[pd.DataFrame], width: int,
                   height: int, theme: str, scale: float) -> str:
        chart = self.execute_template(source, params, data)
        chart = chart.properties(width=width, height=height)

        with alt.themes.enable(theme):
            vl_spec = chart.to_dict()

        return vlc.vegalite_to_svg(json.dumps(vl_spec))
```

**Security note**: The `exec()` call runs user-provided Python code. In
production, templates should be from trusted sources. vizli logs a warning
when executing Altair templates and supports a `--sandbox` flag that
restricts imports to a safe allowlist via a custom import hook.

---

### 3.3 SVG engine (`svg_engine.py`)

**Input**: SVG markup string with `{{param}}` placeholders.

**Pipeline**:
```
SVG string → param interpolation → data injection (if scripted)
    → SVG string output
    → cairosvg → PNG bytes (if requested)
    → Playwright (if SVG has <script>) → PNG bytes
```

**Dependencies**: `cairosvg` (for static SVG → PNG), `playwright` (only if
SVG contains `<script>` elements)

**Implementation**:
```python
import cairosvg
import re

class SVGEngine:
    def interpolate(self, svg: str, params: dict) -> str:
        def replacer(match):
            key = match.group(1).strip()
            if key in params:
                return str(params[key])
            raise VizliError(f"Unresolved param: {{{{{key}}}}}")
        return re.sub(r'\{\{(.+?)\}\}', replacer, svg)

    def has_scripts(self, svg: str) -> bool:
        return '<script' in svg.lower()

    def render_svg(self, svg_raw: str, params: dict,
                   theme: str, width: int, height: int) -> str:
        svg = self.interpolate(svg_raw, params)
        svg = self.apply_theme(svg, theme)
        svg = self.set_dimensions(svg, width, height)
        return svg

    def render_png(self, svg: str, scale: float,
                   width: int, height: int) -> bytes:
        if self.has_scripts(svg):
            return self._render_png_playwright(svg, scale, width, height)
        return cairosvg.svg2png(
            bytestring=svg.encode(),
            output_width=int(width * scale),
            output_height=int(height * scale),
            background_color=self.bg_color
        )
```

**Theme injection**: For SVGs with `data-theme` support, the engine sets
the attribute before rendering. For SVGs without theme support, vizli
can optionally run a simple color replacement pass (configurable in
frontmatter via `rendering.theme_remap`).

---

### 3.4 HTML engine (`html_engine.py`)

**Input**: Complete HTML document string.

**Pipeline**:
```
HTML string → param interpolation → data injection (<script> block)
    → write to temp file → Playwright opens → wait for __VIZLI_READY__
    → screenshot (PNG) or print-to-PDF
    → (for HTML output: write interpolated HTML directly)
```

**Dependencies**: `playwright`

**Implementation**:
```python
from playwright.async_api import async_playwright
import asyncio

class HTMLEngine:
    async def render_png(self, html: str, width: int, height: int,
                         scale: float, timeout: int,
                         theme: str) -> bytes:
        async with async_playwright() as p:
            browser = await p.chromium.launch()
            page = await browser.new_page(
                viewport={"width": width, "height": height},
                device_scale_factor=scale,
                color_scheme="dark" if theme == "dark" else "light"
            )

            # Write HTML to temp file (file:// protocol for local resources)
            tmp = self._write_temp(html)
            await page.goto(f"file://{tmp}")

            # Wait for readiness signal
            await page.wait_for_function(
                "window.__VIZLI_READY__ === true",
                timeout=timeout * 1000
            )

            # Let animations settle (one extra frame)
            await page.wait_for_timeout(100)

            # Screenshot the viewport
            png = await page.screenshot(
                type="png",
                full_page=True,
                omit_background=self.transparent_bg
            )

            await browser.close()
            return png

    def render_html(self, html_raw: str, params: dict,
                    data: Optional[str]) -> str:
        html = self.interpolate(html_raw, params)
        if data:
            html = self.inject_data(html, data)
        return html
```

**Data injection**: vizli inserts a `<script>` block immediately after the
opening `<body>` tag (or before the first `<script>` if no `<body>`):

```html
<script>const __VIZLI_DATA__ = [{"region":"East","revenue":12000}, ...];</script>
```

**Color scheme**: Playwright's `color_scheme` option controls
`prefers-color-scheme` media query behavior, so the template's CSS
automatically responds to the `--theme` flag.

**Viewport sizing**: The `viewport` option controls the browser window size.
For `full_page: True` screenshots, the actual height is determined by the
document content — the viewport height sets a minimum. Templates should
avoid `height: 100vh` patterns that lock to viewport.

---

### 3.5 JS engine (`js_engine.py`)

**Input**: JavaScript source defining a `render(container, params, data)` function.

**Pipeline**:
```
JS source → wrap in HTML harness → inject D3, params, data
    → Playwright renders → wait for __VIZLI_READY__
    → extract <svg> from DOM → SVG string
    → screenshot → PNG bytes
```

**Dependencies**: `playwright`

**Implementation**: The JS engine generates a wrapper HTML page:

```html
<!DOCTYPE html>
<html>
<head>
  <style>
    body { margin: 0; background: {{bg_color}}; }
    #container { width: {{width}}px; height: {{height}}px; }
  </style>
</head>
<body>
  <div id="container"></div>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/d3/7.9.0/d3.min.js"></script>
  <script>
    const __VIZLI_PARAMS__ = {{params_json}};
    const __VIZLI_DATA__ = {{data_json}};
  </script>
  <script>
    {{user_js_source}}
    render(
      document.getElementById('container'),
      __VIZLI_PARAMS__,
      __VIZLI_DATA__
    );
  </script>
</body>
</html>
```

**SVG extraction**: After rendering, the engine queries the DOM for the
outermost `<svg>` element and extracts its `outerHTML`. This gives a clean
SVG string without the HTML wrapper.

---

### 3.6 PDF engine (`pdf_engine.py`)

**Input**: SVG string or HTML string (output from another engine).

**Pipeline**:
```
SVG → weasyprint → PDF
HTML → Playwright print-to-PDF → PDF
```

**Dependencies**: `weasyprint` (for SVG/lightweight HTML), `playwright`
(for complex HTML with JS)

PDF output is a secondary format — vizli first renders to SVG or HTML via
the primary engine, then converts to PDF. The template doesn't need to know
about PDF.

---

## 4. Output behavior

### File naming

If `--output` is a directory, vizli writes files named
`<template_stem>.<format>`:

```bash
vizli render revenue.vizli.json --output report/
# Produces: report/revenue.svg, report/revenue.png
```

If `--output` is a file path, vizli uses it for the primary format and
derives secondary format names:

```bash
vizli render revenue.vizli.json --output report/q3_revenue.png
# Produces: report/q3_revenue.png
# (If template also requests SVG: report/q3_revenue.svg)
```

### Multi-format output

When a template specifies multiple formats (e.g., `[svg, png]`), vizli
produces all of them in a single render pass. For Vega-Lite/Altair, this
means one compilation + two serializations. For HTML, one Playwright launch
produces both the HTML file and the screenshot.

### SVG optimization

SVG output is optionally optimized before writing:

```python
# vizli/output/optimizer.py
def optimize_svg(svg: str, config: OptConfig) -> str:
    """
    Lightweight SVG optimization without external tools.
    - Remove XML comments
    - Remove metadata elements
    - Collapse whitespace in path `d` attributes
    - Remove unused `id` attributes
    - Round numeric attributes to 2 decimal places
    """
```

For more aggressive optimization, vizli supports `--optimize` which shells
out to `svgo` if installed (Node.js dependency — optional).

### PNG compression

PNG output uses maximum compression by default. For smaller files at the
cost of render time, vizli supports `--png-optimize` which runs `oxipng`
(Rust-based PNG optimizer) if installed.

### Metadata embedding

Output files include provenance metadata:

- **SVG**: `<!-- Generated by vizli v0.1 from template: <name> -->`
  comment at the top, plus `<metadata>` element with params used.
- **PNG**: EXIF `Software` field set to `vizli v0.1`, `ImageDescription`
  set to template title.
- **HTML**: `<meta name="generator" content="vizli v0.1">` in `<head>`.
- **PDF**: PDF metadata fields (Author, Creator, Subject).

---

## 5. Python dependencies

### Core (always required)

```
click>=8.1                # CLI framework
pydantic>=2.0             # Config/schema validation
pyyaml>=6.0               # YAML frontmatter parsing
jinja2>=3.1               # Param interpolation engine
pandas>=2.0               # Data loading and transforms
vl-convert-python>=1.3    # Vega-Lite → SVG/PNG (no browser)
```

### Optional groups

```
[browser]                 # For html, js types and SVG-with-scripts
playwright>=1.40

[altair]                  # For altair type
altair>=5.0

[pdf]                     # For PDF output
weasyprint>=60

[images]                  # For SVG → PNG without browser
cairosvg>=2.7
pillow>=10.0

[parquet]                 # For Parquet data input
pyarrow>=14.0

[dev]                     # Development and testing
pytest>=7.0
pytest-asyncio>=0.23
ruff>=0.1
mypy>=1.5
```

### Full install

```bash
pip install "vizli[browser,altair,pdf,images,parquet]"
```

### Minimal install (Vega-Lite charts only)

```bash
pip install vizli
# Handles: vega-lite → SVG/PNG, svg → SVG, data from CSV/JSON
```

---

## 6. Theme system

### Theme configuration

Themes control background, foreground, axis, and gridline colors across
all rendering engines. They're defined as Python dataclasses:

```python
@dataclass
class Theme:
    name: str
    background: str
    foreground: str
    text_primary: str
    text_secondary: str
    text_muted: str
    border: str
    grid: str
    axis: str
    font_family: str = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    font_size: int = 12
    title_font_size: int = 14

    def to_vega_config(self) -> dict:
        """Convert to a Vega config object for vl-convert."""
        ...

    def to_css_vars(self) -> str:
        """Convert to CSS custom properties for injection."""
        ...
```

### Built-in themes

**`light`** (default):
```python
Theme(
    name="light",
    background="#FAFAF8",
    foreground="#2C2C2A",
    text_primary="#2C2C2A",
    text_secondary="#5F5E5A",
    text_muted="#888780",
    border="rgba(0,0,0,0.12)",
    grid="#F1EFE8",
    axis="#B4B2A9",
)
```

**`dark`**:
```python
Theme(
    name="dark",
    background="#1a1a18",
    foreground="#e8e6dc",
    text_primary="#e8e6dc",
    text_secondary="#c2c0b6",
    text_muted="#9c9a92",
    border="rgba(255,255,255,0.12)",
    grid="#2C2C2A",
    axis="#5F5E5A",
)
```

### Custom themes

Users can define custom themes in `~/.config/vizli/themes/` as YAML files:

```yaml
name: brand
background: "#0B1120"
foreground: "#E2E8F0"
text_primary: "#E2E8F0"
text_secondary: "#94A3B8"
text_muted: "#64748B"
border: "rgba(148,163,184,0.2)"
grid: "#1E293B"
axis: "#475569"
```

Applied via `--theme brand`.

---

## 7. Watch mode

`vizli render <template> --watch` uses `watchdog` to monitor:
1. The template file itself
2. Any data file referenced by `--data`
3. Any file referenced in the template's frontmatter

On change, vizli re-renders and writes output. Combined with `--preview`,
this opens the output file and refreshes on each render — useful for
iterative template development.

For HTML templates, `vizli serve <template> --watch` is preferred: it
starts a local HTTP server with live-reload (WebSocket-based), so the
browser auto-refreshes without re-opening the file.

---

## 8. LLM conversion pipeline (`vizli convert`)

The `vizli convert` command sends a raw visualization file to an LLM agent
with the TEMPLATE_SPEC.md as context, and writes the resulting template.

### Implementation

```python
class LLMConverter:
    def __init__(self, model: str = "claude-opus-4-6"):
        self.model = model
        self.spec = load_template_spec()  # reads TEMPLATE_SPEC.md

    async def convert(self, input_path: str,
                      type_hint: Optional[str] = None) -> str:
        raw_content = Path(input_path).read_text()

        prompt = f"""You are converting a raw visualization file into a vizli
template. Follow TEMPLATE_SPEC.md exactly.

## TEMPLATE_SPEC.md
{self.spec}

## Input file ({input_path})
```
{raw_content}
```

{f'The template type should be: {type_hint}' if type_hint else ''}

Produce a complete vizli template file. Output ONLY the template content
(frontmatter + body), no explanation."""

        response = await self.call_api(prompt)
        return response
```

### API integration

vizli uses the Anthropic Python SDK:

```python
import anthropic

client = anthropic.Anthropic()  # reads ANTHROPIC_API_KEY from env

message = client.messages.create(
    model="claude-opus-4-6",
    max_tokens=8192,
    messages=[{"role": "user", "content": prompt}]
)
```

The API key is read from `ANTHROPIC_API_KEY` environment variable or
`~/.config/vizli/config.yaml`:

```yaml
anthropic:
  api_key: sk-ant-...
  model: claude-opus-4-6
```

---

## 9. Error handling

### Error categories

| Category | Example | Behavior |
|---|---|---|
| `TemplateParseError` | Malformed YAML frontmatter | Exit 1, show line number |
| `ParamResolutionError` | Required param missing | Exit 1, list missing params |
| `DataLoadError` | CSV has wrong columns | Exit 1, show expected vs actual schema |
| `RenderError` | Vega-Lite spec invalid | Exit 1, show engine error |
| `TimeoutError` | HTML didn't signal ready | Exit 1, suggest increasing `--timeout` |
| `DependencyError` | Playwright not installed | Exit 1, show install command |

### Structured output

With `--verbose`, errors include the full resolution context:

```
vizli: RenderError in charts/revenue.vizli.json
  Template type: vega-lite
  Resolved params: {color_scheme: "tableau10", x_label: "Month"}
  Data source: q3_actuals.csv (245 rows, 5 columns)
  Engine: vl-convert 1.3.0
  Error: Invalid field "reveneu" in encoding (did you mean "revenue"?)
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Template/render error |
| 2 | CLI usage error |
| 3 | Dependency missing |
| 4 | Timeout |

---

## 10. Testing

### Test matrix

Every engine is tested against a fixture set of templates:

```
tests/
├── fixtures/
│   ├── vegalite/
│   │   ├── bar_simple.vizli.json
│   │   ├── line_timeseries.vizli.json
│   │   └── expected/            # golden SVG/PNG files
│   ├── altair/
│   ├── svg/
│   ├── html/
│   └── js/
├── test_parser.py
├── test_resolver.py
├── test_engines/
│   ├── test_vegalite.py
│   ├── test_altair.py
│   ├── test_svg.py
│   ├── test_html.py
│   └── test_js.py
├── test_data_loader.py
├── test_themes.py
└── test_cli.py                  # integration tests
```

### Golden file testing

For deterministic engines (Vega-Lite, Altair, static SVG), vizli compares
rendered output against golden files using pixel-level diff for PNGs and
structural diff for SVGs (ignoring whitespace, attribute ordering).

For non-deterministic engines (HTML/JS via Playwright), tests verify:
- Output file exists and is non-empty
- PNG dimensions match expected width × height × scale
- SVG contains expected structural elements (checked via xpath)

### CI configuration

```yaml
# GitHub Actions
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/setup-python@v5
      with: { python-version: "3.12" }
    - run: pip install -e ".[dev,browser,altair,images]"
    - run: playwright install chromium
    - run: pytest --timeout=60
```

---

## 11. Roadmap

### v0.1 (initial release)
- Core CLI (`render`, `validate`, `info`)
- All five engine types
- Light/dark themes
- SVG + PNG output
- CSV/JSON data loading

### v0.2
- `vizli convert` with LLM integration
- Template inheritance (`extends`)
- PDF output
- Parquet/NDJSON data loading
- `--watch` mode
- SVG optimization pass

### v0.3
- Template registry and discovery
- `vizli serve` with live-reload
- Custom theme support
- `render-all` batch command
- Plugin system for custom engines

### v1.0
- Stable template spec (breaking changes require major version)
- Comprehensive golden file test suite
- Published template library
- Documentation site
