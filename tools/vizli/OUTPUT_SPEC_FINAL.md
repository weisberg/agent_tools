# VIZLI_OUTPUT_SPEC.md — vizli CLI & Rendering Pipeline v1.0

## 1. Purpose

This document defines how vizli behaves as a Python CLI that renders
visualization templates into output files.

vizli is the runtime companion to TEMPLATE_SPEC.md. Its responsibilities:

- Load a template (single-file or directory package)
- Validate the payload against the template's schema
- Render the visualization with the appropriate engine
- Emit SVG, PNG, HTML, and/or PDF
- Fail clearly when the contract is violated

vizli is a **template renderer**, not a chart authoring UI and not a
notebook replacement. Think of it as part renderer, part validator, part
regression harness. The CLI makes it easy to go from template + payload
to output artifact without remembering which library or export path to
use for each visualization type.

---

## 2. Supported output matrix

| Engine | SVG | PNG | HTML | PDF |
|---|---|---|---|---|
| `vega-lite` | Yes | Yes | Yes | Yes |
| `altair` | Yes | Yes | Yes | Yes |
| `svg` | Yes | Yes | No | Yes |
| `html` | Optional | Yes | Yes | Yes |
| `js` | Yes | Yes | No | Yes |

Notes:
- `html` engine supports SVG only if the rendered page exposes an SVG
  element or a custom extraction hook (see TEMPLATE_SPEC §8).
- `html` and `js` produce PNG through browser screenshotting.
- `svg` engine does not produce HTML. SVG with `<script>` should use
  the `html` engine instead.
- PDF is produced by converting SVG (via weasyprint) or HTML (via
  Playwright print-to-PDF). No engine renders PDF natively.

---

## 3. CLI commands

### 3.1 `render`

Render one template to one or more outputs.

```bash
vizli render <template> [options]
```

**Arguments:**

| Arg | Description |
|---|---|
| `<template>` | Path to a `.vizli` file or a template directory |

**Payload options:**

| Flag | Description |
|---|---|
| `--payload FILE` | JSON file with full canonical payload |
| `--dataset KEY=FILE` | Load a named dataset from a file (repeatable) |
| `--param KEY=VAL` | Set a param value (repeatable) |
| `--params-file FILE` | YAML/JSON file with param values |

**Output options:**

| Flag | Default | Description |
|---|---|---|
| `-f, --format` | from template | Output format(s): `svg`, `png`, `html`, `pdf` |
| `-o, --output` | `./<stem>.<fmt>` | Output file path or directory |
| `--all` | false | Produce all formats declared in template |
| `--out-dir DIR` | `.` | Output directory when using `--all` |

**Canvas overrides:**

| Flag | Default | Description |
|---|---|---|
| `--width` | from template | Override width |
| `--height` | from template | Override height |
| `--scale` | from template | PNG pixel density multiplier |
| `--theme` | `light` | `light`, `dark`, or `auto` |
| `--bg` | from template | Background color or `transparent` |

**Behavior flags:**

| Flag | Description |
|---|---|
| `--timeout` | Rendering timeout in milliseconds (default from template) |
| `--strict` | Treat warnings as errors |
| `--dry-run` | Validate and print resolved config without rendering |
| `--watch` | Re-render on template or data file change |
| `--preview` | Open output in default viewer after render |
| `--verbose` | Verbose logging with diagnostics |
| `--trace` | Capture browser trace for HTML/JS debugging |
| `--quiet` | Suppress non-error output |

**Payload resolution order** (later wins):

1. Sample payload (only if `--payload` not provided and no datasets given)
2. `--payload` file
3. `--dataset` flags (merged into `datasets`)
4. `--param` flags (merged into `params`)
5. `--params-file` contents (merged into `params`)
6. `--width`, `--height`, `--theme`, `--scale`, `--bg` (merged into `meta`)

**Examples:**

```bash
# Render with a full payload file
vizli render templates/monthly-bars \
  --payload payload.json \
  --format svg \
  --output report/revenue.svg

# Render with named datasets and params
vizli render charts/scatter.vizli.json \
  --dataset table=measurements.csv \
  --dataset targets=benchmarks.json \
  --param x_field=temperature \
  --param y_field=yield \
  --param title="Yield vs temperature" \
  --format png \
  --output yield_scatter.png

# Render all declared formats
vizli render charts/revenue.vizli.json \
  --payload q3.json \
  --all \
  --out-dir report/

# Dark mode, high density for slides
vizli render charts/revenue.vizli.json \
  --payload q3.json \
  --theme dark \
  --scale 3 \
  --format png

# Interactive HTML explainer
vizli render explainers/ab_test.vizli.html \
  --param baseline_rate=0.03 \
  --format html

# Dry run to check payload resolution
vizli render charts/revenue.vizli.json \
  --payload q3.json \
  --param color_scheme=dark2 \
  --dry-run

# Watch mode for development
vizli render explainers/cuped.vizli.html --watch --preview
```

### 3.2 `validate`

Validate template structure and payload without rendering.

```bash
vizli validate <template> [--payload FILE] [--strict]
```

Checks:
- Manifest/frontmatter schema validity
- Payload schema validity (if payload provided)
- Path safety (no traversal, no escapes)
- Template compilation (Jinja parse, JSON parse, XML parse)
- Engine-specific contract (readiness config, SVG extraction, etc.)
- Output declarations vs engine capability
- Sidecar existence and cross-validation

### 3.3 `inspect`

Print normalized metadata for a template.

```bash
vizli inspect <template>
```

Shows:
- Template ID, version, engine
- Supported outputs
- Canvas defaults
- Schema summary (all params with types, required status, defaults)
- Entrypoint path
- Readiness configuration
- Golden artifact paths
- Sidecar status

### 3.4 `golden`

Render the sample payload and compare against or update golden outputs.

```bash
vizli golden <template> --check       # compare against golden/
vizli golden <template> --update      # overwrite golden/ with fresh render
vizli golden <template_dir> --check   # check all templates in directory
```

Golden testing is the primary regression mechanism. When a template is
modified, `vizli golden --check` detects visual regressions before the
change ships.

Comparison strategy:
- **SVG**: Structural comparison after normalization (whitespace, attribute
  ordering, generated IDs, timestamps removed)
- **PNG**: Pixel-level comparison against configurable tolerance
  (`visual_diff_tolerance` in manifest, default 0.0)
- **HTML**: DOM structure comparison on key selectors (not raw bytes)

### 3.5 `doctor`

Check local runtime prerequisites.

```bash
vizli doctor
```

Verifies:
- Python version (≥3.10)
- `vl-convert-python` installed and working
- `cairosvg` installed (for SVG→PNG without browser)
- Playwright installed, Chromium browser available
- `defusedxml` installed (for safe XML parsing)
- Ability to launch a headless browser
- Optional: `weasyprint` for PDF, `altair` for Altair engine
- Environment diagnostics (platform, display, temp directory)

### 3.6 `convert`

Convert a raw visualization file to a vizli template using an LLM agent.

```bash
vizli convert <input_file> [--type TYPE] [--model MODEL] [--output DIR]
```

This sends the raw file to an LLM agent (default: Claude Opus 4.6) with
TEMPLATE_SPEC.md as context. The agent produces a conformant template
and sidecar. See §11 for implementation details.

### 3.7 `serve`

Start a local development server with live-reload for HTML templates.

```bash
vizli serve <template> [--port 8400] [--payload FILE]
```

Watches the template, payload, and data files for changes. On change,
re-renders and pushes the update to the browser via WebSocket. Useful
for iterative template development.

### 3.8 `render-all`

Batch render all templates in a directory.

```bash
vizli render-all <dir> [--format png] [--out-dir <dir>] [--payload-dir <dir>]
```

For each template in the directory, vizli looks for a matching payload
file (by convention: `<template_stem>.payload.json`) or uses the sample
payload. Produces output in the specified directory.

---

## 4. Architecture

### 4.1 Pipeline

```
┌─────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌────────┐
│  Parse  │→│ Validate  │→│ Resolve  │→│ Render │→│ Output │
│ template │  │  schema  │  │ payload  │  │ engine │  │ writer │
└─────────┘  └──────────┘  └──────────┘  └────────┘  └────────┘
```

Every render follows this sequence:

1. Resolve template path → detect format (single-file or directory)
2. Parse manifest/frontmatter → load schema
3. Load payload from sources → merge in resolution order
4. Validate payload against schema → fail early on errors
5. Choose renderer adapter by engine type
6. Render requested output format(s)
7. Post-process (SVG optimization, PNG compression)
8. Write output artifact(s)
9. Return structured result with diagnostics

### 4.2 Package layout

```
src/vizli/
  __init__.py
  __main__.py
  cli.py                    # Typer entrypoints
  models.py                 # Pydantic models for manifest, payload, results
  errors.py                 # Structured error hierarchy
  logging.py                # Structured + human-readable logging

  parsing/
    __init__.py
    single_file.py          # YAML frontmatter + body parser
    directory.py            # vizli.template.json + file tree parser
    detect.py               # Format detection logic

  payload/
    __init__.py
    loader.py               # CSV, JSON, Parquet, NDJSON, TSV loading
    resolver.py             # Merge payload sources in priority order
    normalizer.py           # Type coercion, NaN handling

  validation/
    __init__.py
    manifest.py             # Manifest schema validation (Pydantic)
    payload_schema.py       # Payload validation against template schema
    template_compile.py     # Jinja compile, JSON parse, XML parse checks
    engine_contract.py      # Engine-specific validation rules
    path_safety.py          # Path traversal protection

  renderers/
    __init__.py
    base.py                 # RendererAdapter protocol
    registry.py             # Engine → adapter dispatch
    vega_lite.py
    altair_engine.py
    svg_engine.py
    html_engine.py
    js_engine.py

  themes/
    __init__.py
    base.py                 # Theme dataclass
    light.py
    dark.py
    loader.py               # Custom theme loading from ~/.config/vizli/

  html/
    browser.py              # Playwright orchestration
    readiness.py            # Three-tier readiness resolution
    extraction.py           # SVG extraction from rendered pages
    harness.py              # JS engine HTML wrapper generation

  output/
    __init__.py
    writer.py               # File writing, directory creation
    optimizer.py            # SVG normalization, PNG compression
    metadata.py             # Embed provenance in output files
    pdf.py                  # SVG/HTML → PDF conversion

  testing/
    golden.py               # Golden render, compare, update
    diff.py                 # SVG normalization, pixel-level PNG diff

  convert/
    __init__.py
    llm_converter.py        # LLM-based raw → template conversion

  watch.py                  # File watcher for --watch mode (watchdog)
  serve.py                  # Local dev server with live-reload
```

### 4.3 Key design decisions

**Pydantic for config validation.** The manifest schema is defined as
Pydantic v2 models. This gives type validation, default resolution, and
helpful error messages when a template is malformed. Every field has a
type, and optional fields have defaults. Unknown top-level keys produce
warnings, not errors (forward compatibility).

**Engine registry pattern.** Each template type maps to a renderer
adapter class. The registry dispatches based on the `type` field. This
makes it trivial to add new engines later, and impossible to accidentally
invoke the wrong one.

**Playwright for browser rendering.** HTML and JS templates require a
real browser for layout, CSS, and JavaScript execution. Playwright
(Chromium) is the rendering backend. It's installed as an optional
dependency (`vizli[browser]`) since Vega-Lite and SVG templates don't
need it.

**vl-convert for Vega-Lite.** The `vl-convert-python` package compiles
Vega-Lite specs to SVG/PNG natively (no browser needed). This is fast,
deterministic, and runs in CI without Playwright.

**Typer + Rich for CLI.** Typer provides a modern CLI framework with
auto-generated help and type validation. Rich provides formatted terminal
output, tracebacks, and progress indicators.

**Jinja2 for template interpolation.** All template types that support
`{{param}}` placeholders use Jinja2 in strict undefined mode. This
catches missing params at render time with clear error messages instead
of silently producing empty strings.

---

## 5. Internal data model

vizli normalizes all inputs into structured objects before any engine
touches them.

### 5.1 `TemplateManifest`

Pydantic model representing the parsed manifest (from either frontmatter
or `vizli.template.json`). Contains all identity, canvas, schema, runtime,
and golden configuration.

### 5.2 `CanonicalPayload`

```python
@dataclass
class CanonicalPayload:
    params: dict[str, Any]
    datasets: dict[str, list[dict] | dict | Any]
    meta: dict[str, Any]
```

All CLI inputs are normalized into this structure before any engine
touches them. This is the single source of truth for what data the
template will render.

### 5.3 `RenderRequest`

```python
@dataclass
class RenderRequest:
    template_root: Path
    manifest: TemplateManifest
    payload: CanonicalPayload
    format: str                    # svg, png, html, pdf
    output_path: Path
    theme: Theme
    trace: bool = False
```

### 5.4 `RenderResult`

```python
@dataclass
class RenderResult:
    format: str
    output_path: Path
    width: int
    height: int
    engine: str
    duration_ms: int
    warnings: list[str]
    diagnostics: dict[str, Any]    # engine-specific debug info
```

Every render returns a structured result. Agents and CI systems can
inspect the result programmatically.

### 5.5 Error hierarchy

```python
class VizliError(Exception): ...

class ManifestError(VizliError): ...
class SchemaError(VizliError): ...
class PayloadError(VizliError): ...
class TemplateCompileError(VizliError): ...
class RenderError(VizliError): ...
class ExtractionError(VizliError): ...
class TimeoutError(VizliError): ...
class PathSafetyError(VizliError): ...
class OutputWriteError(VizliError): ...
class DependencyError(VizliError): ...
```

Errors should be boringly specific. "Something went wrong" is not an
error message; it is a cry for help.

---

## 6. Renderer adapter contract

Every engine adapter implements the same protocol:

```python
class RendererAdapter(Protocol):
    engine_name: str
    supported_outputs: set[str]

    def validate(self, manifest: TemplateManifest,
                 payload: CanonicalPayload) -> list[str]:
        """Engine-specific validation. Returns list of warnings."""
        ...

    def render(self, request: RenderRequest) -> RenderResult:
        """Render the requested format. Raises RenderError on failure."""
        ...
```

The registry maps `manifest.type` (or `manifest.engine` for directory
packages) to the adapter class. Unknown engine types raise `ManifestError`.

---

## 7. Engine implementations

### 7.1 `vega-lite` renderer

**Input**: Jinja2 template → JSON string → Vega-Lite v5 spec.

**Pipeline**:
1. Render Jinja2 template with payload as context
2. Parse JSON, validate structure
3. Inject canvas overrides (width, height)
4. Apply theme config
5. Render via `vl-convert-python`:
   - SVG: `vlc.vegalite_to_svg()`
   - PNG: `vlc.vegalite_to_png(scale=scale)`
   - HTML: generate offline HTML with bundled Vega runtime
   - PDF: render SVG → weasyprint

**Dependencies**: `vl-convert-python` (required), `jinja2`

**No browser needed.** vl-convert renders natively in Python. This is
the fastest engine and the one that works in every CI environment.

**Implementation reference**:

```python
import vl_convert as vlc

class VegaLiteRenderer:
    engine_name = "vega-lite"
    supported_outputs = {"svg", "png", "html", "pdf"}

    def render_svg(self, spec_json: str, theme: Theme,
                   width: int, height: int) -> str:
        vl_spec = orjson.loads(spec_json)
        vl_spec["width"] = width
        vl_spec["height"] = height

        svg = vlc.vegalite_to_svg(
            vl_spec=orjson.dumps(vl_spec).decode(),
            config=orjson.dumps(theme.to_vega_config()).decode()
        )
        return svg

    def render_png(self, spec_json: str, theme: Theme,
                   width: int, height: int, scale: float) -> bytes:
        vl_spec = orjson.loads(spec_json)
        vl_spec["width"] = width
        vl_spec["height"] = height

        png = vlc.vegalite_to_png(
            vl_spec=orjson.dumps(vl_spec).decode(),
            config=orjson.dumps(theme.to_vega_config()).decode(),
            scale=scale
        )
        return png
```

**Theme application**: vizli injects a Vega config object that sets
background, font, axis styles, and color schemes. The `themes/` module
defines light and dark configs. The config is passed directly to
vl-convert — the template spec is never mutated.

**Error handling**: vl-convert reports spec errors with line numbers.
vizli surfaces these as structured errors pointing to the template body,
including "did you mean?" suggestions when field names are close to
dataset column names.

---

### 7.2 `altair` renderer

**Input**: Python module with `build_chart(params, datasets, meta)`.

**Pipeline**:
1. Load module from file path in isolated import namespace
2. Call `build_chart(payload.params, payload.datasets, payload.meta)`
3. Validate returned object is an Altair chart
4. Apply theme, inject width/height
5. Convert to Vega-Lite spec via `chart.to_dict()`
6. Render via vl-convert (same as vega-lite engine from here)

**Dependencies**: `altair`, `pandas`, `vl-convert-python`

**Implementation reference**:

```python
import altair as alt
import importlib.util

class AltairRenderer:
    engine_name = "altair"
    supported_outputs = {"svg", "png", "html", "pdf"}

    ALLOWED_IMPORTS = frozenset({
        "altair", "pandas", "math", "json", "datetime",
        "dataclasses", "typing", "collections", "itertools",
        "functools", "decimal", "fractions", "statistics",
    })

    BLOCKED_IMPORTS = frozenset({
        "os", "sys", "subprocess", "importlib", "socket",
        "http", "urllib", "requests", "pathlib", "shutil",
        "tempfile", "glob", "io", "pickle", "shelve",
    })

    def load_module(self, module_path: Path) -> ModuleType:
        spec = importlib.util.spec_from_file_location(
            "vizli_template", module_path
        )
        module = importlib.util.module_from_spec(spec)
        # Install import guard before executing
        module.__builtins__ = self._restricted_builtins()
        spec.loader.exec_module(module)
        return module

    def render(self, request: RenderRequest) -> RenderResult:
        module = self.load_module(request.template_root / request.manifest.entrypoint)

        if not hasattr(module, "build_chart"):
            raise RenderError("Altair template must export build_chart()")

        chart = module.build_chart(
            request.payload.params,
            request.payload.datasets,
            request.payload.meta,
        )

        if not isinstance(chart, alt.TopLevelMixin):
            raise RenderError(
                f"build_chart() returned {type(chart).__name__}, "
                f"expected an Altair chart"
            )

        chart = chart.properties(
            width=request.payload.meta.get("width", 800),
            height=request.payload.meta.get("height", 500),
        )

        vl_spec = chart.to_dict()
        # Delegate to vega-lite renderer from here
        ...
```

**Security**: The module runs in a constrained import namespace. vizli
does NOT use `exec()` on arbitrary strings. It imports the module, calls
the single declared function, and validates the return type. Side effects
(file writes, network, subprocess) are bugs in the template, and vizli
logs warnings if any are detected.

**Caching**: The imported module is cached only for the duration of the
command. No cross-command state.

---

### 7.3 `svg` renderer

**Input**: Jinja2 template → SVG markup string.

**Pipeline**:
1. Render Jinja2 template with payload as context
2. Parse SVG safely (defusedxml)
3. Validate root `<svg>` element and `viewBox`
4. Inject `data-theme` attribute for theme support
5. Set explicit `width` and `height` attributes
6. Normalize formatting
7. Emit SVG string
8. For PNG: rasterize via cairosvg (default) or Playwright (fallback)

**Dependencies**: `jinja2`, `cairosvg` (for PNG), `defusedxml`

**Implementation reference**:

```python
import cairosvg
from defusedxml import ElementTree as SafeET

class SVGRenderer:
    engine_name = "svg"
    supported_outputs = {"svg", "png", "pdf"}

    def render_svg(self, svg_raw: str, payload: CanonicalPayload,
                   theme: Theme, width: int, height: int) -> str:
        # Jinja2 renders params into the SVG
        env = jinja2.Environment(undefined=jinja2.StrictUndefined)
        template = env.from_string(svg_raw)
        svg = template.render(**payload.params, **payload.datasets)

        # Safe XML parse to validate structure
        root = SafeET.fromstring(svg.encode())
        if root.tag != '{http://www.w3.org/2000/svg}svg':
            raise RenderError("SVG template must have root <svg> element")

        # Inject theme and dimensions
        root.set('data-theme', theme.name)
        root.set('width', str(width))
        root.set('height', str(height))

        return SafeET.tostring(root, encoding='unicode')

    def render_png(self, svg: str, scale: float,
                   width: int, height: int,
                   bg_color: str) -> bytes:
        return cairosvg.svg2png(
            bytestring=svg.encode(),
            output_width=int(width * scale),
            output_height=int(height * scale),
            background_color=bg_color,
        )
```

**Rasterization fallback**: If the manifest sets `browser_rasterize: true`,
or if cairosvg fails, vizli renders the SVG in a browser page and captures
a screenshot. This fallback exists because SVG in the wild is a chaotic
carnival — cairosvg handles most cases but some CSS features (animations,
advanced filters, `foreignObject`) require a real browser.

**Theme injection**: For SVGs with `data-theme` support, the engine sets
the attribute before rendering. For SVGs without theme support, vizli
can optionally run a CSS variable injection pass (configurable in
manifest via `rendering.theme_vars`).

---

### 7.4 `html` renderer

**Input**: Jinja2 template → complete HTML document.

**Pipeline**:
1. Render Jinja2 template with payload as context
2. Inject dataset payload as a `<script>` block (see below)
3. Bundle or resolve local assets (inline CSS/JS if configured)
4. Launch headless Chromium via Playwright
5. Disable network in browser context
6. Set viewport from canvas dimensions, scale from config
7. Set `color_scheme` for theme (controls `prefers-color-scheme`)
8. Load the HTML from a temp file (`file://` protocol for local resources)
9. Wait for readiness (three-tier resolution, see TEMPLATE_SPEC §8)
10. Capture console errors (surface as warnings)
11. Emit outputs:
    - HTML: write the offline document
    - PNG: page or element screenshot
    - SVG: DOM extraction via selector, getter, or auto-detect
    - PDF: Playwright print-to-PDF

**Dependencies**: `playwright`, `jinja2`

**Implementation reference**:

```python
from playwright.async_api import async_playwright

class HTMLRenderer:
    engine_name = "html"
    supported_outputs = {"svg", "png", "html", "pdf"}

    async def render_png(self, html: str, width: int, height: int,
                         scale: float, timeout_ms: int,
                         theme: Theme, transparent_bg: bool) -> bytes:
        async with async_playwright() as p:
            browser = await p.chromium.launch()
            context = await browser.new_context(
                viewport={"width": width, "height": height},
                device_scale_factor=scale,
                color_scheme="dark" if theme.name == "dark" else "light",
            )

            page = await context.new_page()

            # Disable network — requests fail, not ignored
            await context.route("**/*", lambda route: route.abort())

            # Capture console errors
            console_errors = []
            page.on("console", lambda msg: (
                console_errors.append(msg.text)
                if msg.type == "error" else None
            ))

            # Write HTML to temp file (file:// for local resource access)
            tmp = self._write_temp(html)
            await page.goto(f"file://{tmp}")

            # Wait for readiness signal
            await page.wait_for_function(
                "window.__VIZLI_READY__ === true",
                timeout=timeout_ms,
            )

            # Let animations settle (one extra frame)
            await page.wait_for_timeout(100)

            # Screenshot the viewport
            png = await page.screenshot(
                type="png",
                full_page=True,
                omit_background=transparent_bg,
            )

            await browser.close()
            return png
```

**Dataset injection**: When the template uses datasets, vizli injects a
`<script>` block immediately after the opening `<body>` tag (or before
the first `<script>` if no `<body>`):

```html
<script>
  const __VIZLI_PARAMS__ = {"baseline_rate": 0.03, "title": "Power Analysis"};
  const __VIZLI_DATA__ = [{"region":"East","revenue":12000}, ...];
  const __VIZLI_DATASETS__ = {"table": [...], "targets": [...]};
  const __VIZLI_META__ = {"width": 800, "height": 500, "theme": "light"};
</script>
```

The template's JavaScript reads from these globals. Jinja2 interpolation
handles `{{param}}` in markup; the script injection handles structured
data that would be unwieldy as Jinja2 expressions.

**Browser context rules**:
- Ephemeral and isolated (non-persistent context)
- Network disabled (requests fail, not silently dropped)
- Console errors captured and surfaced as render warnings
- Timeout enforced (from `runtime.timeout_ms`)
- One browser launch per command, reused across renders when practical
- Context per render (no shared state between templates)

**Viewport sizing**: The `viewport` option controls the browser window
size. For `full_page: True` screenshots, the actual height is determined
by the document content — the viewport height sets a minimum. Templates
should avoid `height: 100vh` patterns that lock to viewport.

---

### 7.5 `js` renderer

**Input**: JavaScript module with `render(container, params, datasets, meta)`.

**Pipeline**:
1. Generate HTML harness page (see template below)
2. Load in Playwright (same browser rules as html renderer)
3. Wait for readiness signal
4. Extract SVG from DOM (same extraction logic as html renderer)
5. Screenshot for PNG

**Dependencies**: `playwright`, D3 v7 (bundled locally by vizli)

**Implementation reference — the harness template**:

The harness page is generated by vizli, not by the template author. The
author writes only the `render()` function. vizli wraps it in:

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { background: {{bg_color}}; }
    #container { width: {{width}}px; height: {{height}}px; }
  </style>
</head>
<body>
  <div id="container"></div>

  <!-- D3 bundled locally by vizli — no CDN -->
  <script src="file://{{vizli_assets}}/d3.v7.min.js"></script>

  <!-- Payload injection -->
  <script>
    const __VIZLI_PARAMS__ = {{params_json}};
    const __VIZLI_DATASETS__ = {{datasets_json}};
    const __VIZLI_META__ = {{meta_json}};
    // Legacy alias for single-dataset templates
    const __VIZLI_DATA__ = __VIZLI_DATASETS__[
      Object.keys(__VIZLI_DATASETS__)[0]
    ] ?? null;
  </script>

  <!-- User's JS module -->
  <script>
    {{user_js_source}}

    render(
      document.getElementById('container'),
      __VIZLI_PARAMS__,
      __VIZLI_DATASETS__,
      __VIZLI_META__
    );
  </script>
</body>
</html>
```

**SVG extraction**: After rendering, the engine queries the DOM for the
outermost `<svg>` element and extracts its `outerHTML`. This gives a
clean SVG string without the HTML wrapper. The extraction selector is
configurable in the manifest for templates that produce nested or
multiple SVGs.

---

### 7.6 PDF engine

**Input**: SVG string or HTML string (output from another engine).

**Pipeline**:
```
SVG → weasyprint → PDF
HTML → Playwright print-to-PDF → PDF
```

**Dependencies**: `weasyprint` (for SVG/lightweight HTML), `playwright`
(for complex HTML with JS)

PDF output is a secondary format — vizli first renders to SVG or HTML via
the primary engine, then converts to PDF. The template doesn't need to
know about PDF.

---

## 8. Validation strategy

### 8.1 Manifest validation

Pydantic models validate the manifest structure. Every field has a type,
and optional fields have defaults. Unknown top-level keys produce warnings,
not errors (forward compatibility).

### 8.2 Payload validation

vizli validates payloads against the template's schema BEFORE rendering.
For single-file templates, the schema is in frontmatter. For directory
packages, it's in `schema.json` (JSON Schema format).

Validation catches:
- Missing required params
- Wrong types (string where int expected)
- Values outside declared enum/range
- Missing required datasets
- Structural mismatches in dataset objects

### 8.3 Template compilation validation

Before rendering, vizli does a compilation pass:
- Jinja2 templates: parse without rendering, check for syntax errors
- JSON templates: parse the rendered output as JSON
- SVG templates: parse as XML via defusedxml, validate root element
- HTML templates: basic structural checks (doctype, body)

### 8.4 Engine contract validation

Examples of engine-specific checks:
- `html` declaring `svg` output must also declare extraction behavior
- `svg` must have root `<svg>` element with `viewBox`
- `altair` module must export `build_chart`
- `vega-lite` body must parse to a JSON object, not a string
- Readiness config must be present for `html` and `js` engines
- Output declarations must be a subset of engine's `supported_outputs`

All validation errors surface as structured error objects with the
template path, the failing field, and the reason.

---

## 9. Theme system

### 9.1 Theme definition

Themes control background, foreground, axis, and grid colors across all
engines.

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
        """Convert to CSS custom properties for injection into HTML/SVG."""
        ...

    def to_svg_data_theme(self) -> str:
        """Return the theme name for data-theme attribute injection."""
        ...
```

### 9.2 Built-in themes

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

### 9.3 Custom themes

User-defined themes in `~/.config/vizli/themes/` as YAML files:

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

### 9.4 Theme injection by engine

| Engine | Mechanism |
|---|---|
| `vega-lite` | Vega config object passed to vl-convert |
| `altair` | `alt.themes.enable()` with custom theme registration |
| `svg` | `data-theme` attribute on root `<svg>` + CSS variable injection |
| `html` | Playwright `color_scheme` + CSS variable injection via `<style>` |
| `js` | `meta.theme` in payload + Playwright `color_scheme` |

---

## 10. Output handling

### 10.1 File naming

If `--output` is a directory, vizli writes `<template_stem>.<format>`:
```bash
vizli render revenue.vizli.json --output report/
# → report/revenue.svg, report/revenue.png
```

If `--output` is a file path, vizli uses it for the primary format:
```bash
vizli render revenue.vizli.json --output report/q3.png
# → report/q3.png
```

With `--all`, vizli writes all declared formats to `--out-dir`:
```bash
vizli render revenue.vizli.json --all --out-dir report/
# → report/revenue.svg, report/revenue.png, report/revenue.html
```

### 10.2 Multi-format output

When a template specifies multiple formats (e.g., `[svg, png]`), vizli
produces all of them in a single render pass. For Vega-Lite/Altair, this
means one compilation + two serializations. For HTML, one Playwright
launch produces both the HTML file and the screenshot.

### 10.3 SVG optimization

Before writing SVG output, vizli applies lightweight normalization:
- Remove XML comments
- Remove metadata elements
- Collapse whitespace in path `d` attributes
- Round numeric attributes to 2 decimal places
- Remove unused `id` attributes

For aggressive optimization, `--optimize` shells out to `svgo` if
installed (Node.js dependency, optional).

### 10.4 PNG compression

PNG output uses maximum compression by default. `--png-optimize` runs
`oxipng` if installed for additional compression.

### 10.5 Metadata embedding

Output files include provenance metadata:
- **SVG**: `<!-- vizli v1.0 | template: ID | rendered: ISO8601 -->`
- **PNG**: EXIF Software field, ImageDescription from title
- **HTML**: `<meta name="generator" content="vizli v1.0">`
- **PDF**: PDF metadata fields (Author, Creator, Subject)

### 10.6 Offline HTML bundling

HTML output must be viewable without network access. vizli supports:
- `single-file` — inline all CSS and JS assets (default)
- `bundle-dir` — write HTML plus sibling asset files
- `auto` — choose based on engine and asset size

For Vega-Lite/Altair HTML output, vizli bundles the Vega runtime locally.
No CDN references in any output file.

---

## 11. LLM conversion pipeline

The `vizli convert` command wraps an LLM agent that reads a raw
visualization file and produces a conformant template plus sidecar.

### 11.1 Implementation

```python
import anthropic

class LLMConverter:
    def __init__(self, model: str = "claude-opus-4-6"):
        self.model = model
        self.client = anthropic.Anthropic()  # reads ANTHROPIC_API_KEY from env
        self.template_spec = load_spec("TEMPLATE_SPEC.md")
        self.sidecar_spec = load_spec("SIDECAR_SPEC.md")

    async def convert(self, input_path: str,
                      type_hint: Optional[str] = None) -> ConvertResult:
        raw_content = Path(input_path).read_text()

        prompt = f"""Convert this visualization into a vizli template.
Follow TEMPLATE_SPEC.md and SIDECAR_SPEC.md exactly.

## TEMPLATE_SPEC.md
{self.template_spec}

## SIDECAR_SPEC.md
{self.sidecar_spec}

## Input file ({input_path})
```
{raw_content}
```

{f'The template type should be: {type_hint}' if type_hint else ''}

Produce:
1. The complete template file
2. The complete sidecar file
3. Brief notes on engine choice and parameterization"""

        message = self.client.messages.create(
            model=self.model,
            max_tokens=8192,
            messages=[{"role": "user", "content": prompt}],
        )

        return self.parse_response(message)
```

### 11.2 API configuration

The API key is read from `ANTHROPIC_API_KEY` environment variable or
`~/.config/vizli/config.yaml`:

```yaml
anthropic:
  api_key: sk-ant-...
  model: claude-opus-4-6
```

---

## 12. Security and sandboxing

### 12.1 Network isolation

vizli disables network access for all browser-based rendering. Playwright
contexts are launched with network disabled. Attempted requests are logged
as warnings, not silently dropped.

For non-browser engines (vega-lite, svg), there is no network path — the
rendering is pure computation.

### 12.2 Path safety

All file access is validated against the template root:
- Paths must be relative
- Paths must not contain `..` components
- Paths must not escape the template directory
- Symlinks are resolved and re-validated
- Violations raise `PathSafetyError`

### 12.3 Python execution containment

The `altair` engine imports a module and calls a single function. It does
NOT run `exec()` on arbitrary strings. The import namespace is constrained:
- `altair`, `pandas`, `math`, `json`, `datetime` are available
- `os`, `sys`, `subprocess`, `importlib`, `socket`, `http` are blocked
- File system access is blocked
- Network access is blocked

### 12.4 XML safety

SVG templates are parsed with `defusedxml` to prevent:
- Entity expansion attacks (billion laughs)
- External entity resolution
- DTD processing

### 12.5 Browser isolation

Playwright browser contexts are:
- Ephemeral (no persistent state)
- Isolated (no shared cookies, storage, or cache)
- Network-disabled
- Sandboxed (Chromium's own sandbox is enabled)
- Timeout-bounded

### 12.6 Logging

vizli surfaces:
- Attempted network access (from browser console)
- Console errors from rendered pages
- Missing asset files
- Undefined template variables
- Extraction failures
- Path safety violations
- Import violations in Python templates

---

## 13. Testing strategy

### 13.1 Unit tests

- Manifest parsing (both single-file and directory formats)
- Payload merging and resolution order
- Path safety validation
- Engine registry dispatch
- Schema validation
- Error formatting
- Theme loading and conversion

### 13.2 Integration tests

For each engine:
- Render sample payload → verify output file exists
- Verify structural validity (SVG is valid XML, PNG has correct
  dimensions, HTML is a complete document)
- Verify no console errors, no network requests
- Verify dimensions match canvas config

### 13.3 Golden tests

For stable templates:
- SVG: structural comparison after normalization
- PNG: pixel-level comparison with configurable tolerance
- HTML: DOM structure comparison on key selectors

The `vizli golden` command automates this workflow.

### 13.4 Browser tests

Playwright-backed tests for HTML/JS rendering:
- Readiness signal fires within timeout
- SVG extraction produces valid SVG
- Screenshots match expected dimensions
- No console errors

### 13.5 Parametrized fixtures

Maintain a library of fixture templates across all engine types. Run
them through the same render contract to verify cross-engine consistency.

---

## 14. Watch mode

`vizli render <template> --watch` uses `watchdog` to monitor:
1. The template file itself
2. Any data file referenced via `--dataset` or `--payload`
3. Any local asset file referenced in the manifest (for directory packages)

On change, vizli re-renders and writes output. Combined with `--preview`,
this opens the output file and refreshes on each render — useful for
iterative template development.

For HTML templates, `vizli serve <template> --watch` is preferred: it
starts a local HTTP server with live-reload (WebSocket-based), so the
browser auto-refreshes without re-opening the file.

---

## 15. Logging and diagnostics

### 15.1 CLI output

Default output (non-verbose):
```
✓ revenue.vizli.json → revenue.png (800×500, 1.2s)
```

Verbose output:
```
Template: charts/revenue.vizli.json (vega-lite)
Payload:  q3.json + 2 overrides
Datasets: table (245 rows, 5 columns)
Engine:   vl-convert 1.3.0
Theme:    light
Canvas:   800×500 @2x
Render:   1.2s
Output:   report/revenue.png (142 KB)
Warnings: none
```

### 15.2 Error output

Errors include full resolution context:
```
✗ RenderError in charts/revenue.vizli.json
  Engine:   vega-lite
  Payload:  {color_scheme: "tableau10", x_field: "quarter", ...}
  Datasets: table (245 rows)
  Error:    Invalid field "reveneu" in encoding
  Hint:     Did you mean "revenue"? (column exists in dataset)
```

### 15.3 Debug artifacts

With `--trace` or `--verbose`:
- Rendered intermediate JSON (for vega-lite)
- Rendered SVG string (for svg engine)
- Console logs from browser page (for html/js)
- Screenshot on HTML rendering failure
- Copied HTML debug bundle on readiness timeout
- Browser trace file (for Playwright trace viewer)

---

## 16. Python dependencies

### Core (always required)

```
typer>=0.9                    # CLI framework
rich>=13.0                    # Terminal output and tracebacks
pydantic>=2.0                 # Config and model validation
jsonschema>=4.20              # Payload validation against template schemas
pyyaml>=6.0                   # YAML frontmatter parsing
jinja2>=3.1                   # Template rendering (strict undefined)
orjson>=3.9                   # Fast JSON parsing/serialization
vl-convert-python>=1.3        # Vega-Lite → SVG/PNG (no browser)
defusedxml>=0.7               # Safe XML parsing for SVG
```

### Optional groups

```
[browser]                     # For html, js engines
playwright>=1.40

[altair]                      # For altair engine
altair>=5.0
pandas>=2.0

[images]                      # For SVG → PNG without browser
cairosvg>=2.7
pillow>=10.0

[pdf]                         # For PDF output
weasyprint>=60

[parquet]                     # For Parquet data input
pyarrow>=14.0

[convert]                     # For vizli convert (LLM integration)
anthropic>=0.30

[watch]                       # For --watch mode
watchdog>=3.0

[dev]                         # Development and testing
pytest>=7.0
pytest-asyncio>=0.23
pytest-playwright>=0.4
ruff>=0.1
mypy>=1.5
```

### Installation profiles

```bash
# Minimal: Vega-Lite charts, SVG output
pip install vizli

# Standard: all engines, browser rendering
pip install "vizli[browser,altair,images]"

# Full: everything
pip install "vizli[browser,altair,images,pdf,parquet,convert,watch]"

# Development
pip install "vizli[browser,altair,images,pdf,parquet,convert,watch,dev]"
```

---

## 17. Performance and caching

### 17.1 v1 priorities

1. Correctness first
2. Predictable rendering second
3. Speed third

### 17.2 Acceptable caching

- Parsed manifest cache within one process
- Jinja2 environment cache (compiled templates)
- Browser reuse within a single command invocation
- Memoized local asset reads

### 17.3 Avoid in v1

- Global daemon mode
- Cross-command cache invalidation
- Speculative rendering
- Persistent browser pools

Do not build a cache dragon before you have a village to defend.

---

## 18. Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Template, validation, or render error |
| 2 | CLI usage error (bad flags, missing arguments) |
| 3 | Missing dependency |
| 4 | Timeout |

---

## 19. Implementation order

### Phase 1 — Foundation

- Pydantic models for manifest and payload
- Single-file template parser
- Payload validation against schema
- Jinja2 template rendering
- `vega-lite` renderer (via vl-convert)
- `svg` renderer (Jinja2 + cairosvg)
- `render` and `validate` commands
- `doctor` command
- Light and dark themes
- Structured error hierarchy

### Phase 2 — Browser engines

- Directory package parser
- `html` renderer with Playwright
- `js` renderer with D3 harness
- Three-tier readiness resolution
- SVG extraction from rendered pages
- PNG screenshotting
- `inspect` command

### Phase 3 — Testing and Altair

- `altair` renderer with constrained execution
- Golden testing framework
- `golden` command (check + update)
- SVG normalization for diff
- Pixel-level PNG comparison
- Parametrized fixture suite

### Phase 4 — Workflow tools

- `convert` command with LLM integration
- `serve` command with live-reload
- `render-all` batch command
- PDF output (weasyprint + Playwright)
- Custom theme loading
- Offline HTML bundling
- SVG optimization pass
- `--watch` mode

---

## 20. Summary

vizli should be:

- **Strict about inputs** — validate before rendering
- **Deterministic about outputs** — same payload, same result
- **Modular in implementation** — one adapter per engine
- **Friendly at the CLI** — clear errors, structured diagnostics
- **Ruthless about offline rendering** — no network, no exceptions
- **Honest about engine capabilities** — don't claim what you can't produce
- **Secure by default** — AI-authored templates are untrusted
- **Concrete in specification** — implementation references, not hand-waves

If the template obeys TEMPLATE_SPEC.md, then `vizli render` should feel
almost boring. That is the dream: industrial-grade boredom, in service of
reliable graphics.
