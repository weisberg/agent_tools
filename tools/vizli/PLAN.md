# PLAN.md — vizli Development Plan

## 1. What vizli is

vizli is a Python CLI that renders parameterized visualization templates
into output files (SVG, PNG, HTML, PDF). It is designed for AI agents:
an LLM reads a sidecar file to discover the right template, resolves
parameters from user data, calls `vizli render`, and delivers the output.

vizli is NOT a chart library, a notebook, or a dashboard framework. It
is a deterministic rendering pipeline with strict validation, offline
execution, and regression testing built in.

## 2. Governing documents

| Document | Role |
|---|---|
| `TEMPLATE_SPEC_FINAL.md` | What templates look like, how authors write them |
| `OUTPUT_SPEC_FINAL.md` | How the CLI renders templates into output files |
| `SIDECAR_SPEC.md` | How templates are documented for agent discovery |
| `VIZLI_README.md` | How an AI agent uses vizli at runtime |

These specs are authoritative. The plan implements them — it does not
redefine or contradict them. When in doubt, the spec wins.

## 3. Architecture overview

```
┌─────────────────────────────────────────────────────────┐
│                        CLI (Typer)                       │
│  render │ validate │ inspect │ golden │ doctor │ ...    │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                    Pipeline                              │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐  ┌──────────┐ │
│  │ Parsing  │→│Validation │→│Rendering │→│  Output   │ │
│  │          │  │           │  │          │  │          │ │
│  │ single   │  │ manifest  │  │ registry │  │ writer   │ │
│  │ directory │  │ payload   │  │ adapters │  │ optimizer│ │
│  │ detect   │  │ compile   │  │          │  │ metadata │ │
│  │          │  │ contract  │  │          │  │ pdf      │ │
│  │          │  │ path      │  │          │  │ bundler  │ │
│  └─────────┘  └──────────┘  └─────────┘  └──────────┘ │
└─────────────────────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                    Engines                                │
│  ┌──────────┐ ┌───────┐ ┌─────┐ ┌──────┐ ┌────┐       │
│  │ vega-lite│ │altair │ │ svg │ │ html │ │ js │       │
│  │ vl-conv  │ │import │ │cairo│ │playw.│ │harn│       │
│  └──────────┘ └───────┘ └─────┘ └──────┘ └────┘       │
└─────────────────────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  Cross-cutting                           │
│  themes │ payload │ models │ errors │ logging │ testing │
└─────────────────────────────────────────────────────────┘
```

### Key architectural decisions

**Decision 1: RendererAdapter protocol, not inheritance.**
Each engine implements a protocol with `validate()` and `render()`. The
registry dispatches by engine type string. This means engines share no
state and can't accidentally couple to each other. Adding a new engine
is one file + one registry entry.

**Decision 2: CanonicalPayload as the universal data contract.**
All CLI inputs normalize into `{params, datasets, meta}` before any
engine touches them. This eliminates per-engine input handling and makes
payload validation a single concern.

**Decision 3: Jinja2 for all text-based templates, strict undefined.**
Vega-Lite JSON, SVG markup, and HTML all render through Jinja2 with
`StrictUndefined`. This gives uniform interpolation, one set of filters,
and immediate failure on typos — instead of silent empty strings.

**Decision 4: No `exec()`, ever.**
Altair templates are imported as modules and call a single function.
This is slower to set up than `exec()` but prevents arbitrary code
execution in AI-authored templates.

**Decision 5: Playwright with network disabled.**
Browser-rendered templates (HTML, JS) run in Chromium with network
blocked at the route level. Attempted requests are logged, not silently
dropped. This makes the offline guarantee enforceable, not advisory.

**Decision 6: Golden testing as first-class citizen.**
The `vizli golden` command is not an afterthought — it's how templates
are regression-tested. Every production template ships with golden
reference outputs that CI verifies on every push.

---

## 4. Milestones

| Milestone | Phase | Description | Depends on |
|---|---|---|---|
| `M0: Bootstrap` | 0 | Installable package, CLI skeleton, CI | — |
| `M1: Core Rendering` | 1 | Vega-Lite + SVG rendering, validation | M0 |
| `M2: Browser Engines` | 2 | HTML + JS via Playwright, directory packages | M1 |
| `M3: Full Engine Suite` | 3 | Altair engine, golden testing, all 5 engines | M2 |
| `M4: Workflow Tools` | 4 | Convert, serve, render-all, PDF, watch | M3 |
| `M5: Template Library` | 5 | Production templates, skill, end-to-end | M3 |

```
M0 ──→ M1 ──→ M2 ──→ M3 ──┬──→ M4
                            └──→ M5
```

M4 and M5 can run in parallel after M3. M4 adds developer workflow tools.
M5 adds the content and agent integration that makes vizli useful.

---

## 5. Phase 0 — Project Bootstrap

**Goal**: Installable Python package, CLI skeleton with all commands
stubbed, CI pipeline, `vizli doctor` working.

**Key risk**: None — pure setup.

### Work packages

#### 0.1 Project structure and pyproject.toml
- Create `src/vizli/` package layout per OUTPUT_SPEC_FINAL §4.2
- All subpackages as empty `__init__.py` files
- `pyproject.toml` with hatchling build system
- Core deps: typer, rich, pydantic, jsonschema, pyyaml, jinja2, orjson,
  vl-convert-python, defusedxml
- Optional groups: `[browser]`, `[altair]`, `[images]`, `[pdf]`,
  `[parquet]`, `[convert]`, `[watch]`, `[dev]`
- `python_requires >= "3.10"`
- `.gitignore` for Python, IDE, OS files

#### 0.2 CLI skeleton
- Typer app in `cli.py` with all 8 commands stubbed
- Commands: render, validate, inspect, golden, doctor, convert, serve, render-all
- Each stub prints a "not yet implemented" message and exits cleanly
- `__main__.py` for `python -m vizli`
- `[project.scripts]` entry point: `vizli = "vizli.cli:app"`

#### 0.3 Error hierarchy
- `errors.py` with the full class tree from OUTPUT_SPEC_FINAL §5.5
- Each class accepts structured context kwargs (template_path, field, reason)
- Rich-formatted `__str__` for terminal display

#### 0.4 Logging setup
- `logging.py` with Rich console handler
- Three verbosity levels: default (one-line), verbose (full diagnostics),
  quiet (errors only)
- Trace mode for debug artifacts

#### 0.5 `vizli doctor`
- First real command implementation
- Checks: Python ≥ 3.10, core deps importable, optional deps status,
  Playwright chromium available, temp dir writable
- Rich-formatted output: ✓/✗ per check

#### 0.6 CI and tooling
- GitHub Actions: lint + type check + test on push (Python 3.12)
- `ruff` config in pyproject.toml
- `mypy` strict mode config
- `pytest` config with `tests/` directory and `conftest.py`

### Definition of done
- `pip install -e ".[dev]"` succeeds
- `vizli --help` prints all 8 command names
- `vizli doctor` reports dependency status
- `vizli render foo` exits cleanly with "not implemented" message
- `ruff check src/` and `mypy src/` pass
- `pytest` runs with 0 tests, 0 failures
- CI green on push

---

## 6. Phase 1 — Core Rendering

**Goal**: Parse single-file templates, validate payloads against schemas,
render Vega-Lite and SVG templates to SVG and PNG. The `render`,
`validate`, and `inspect` commands work end-to-end.

**Key risk**: vl-convert-python API surface may differ from what the spec
assumes. Mitigate by writing vl-convert integration tests first.

### Work packages

#### 1.1 Pydantic models
- `models.py`: TemplateManifest, CanonicalPayload, RenderRequest,
  RenderResult, ParamSchema, DatasetSchema, CanvasConfig, RuntimeConfig
- Strict validation: unknown keys warn, missing required keys error
- All fields documented with Field descriptions

#### 1.2 Single-file template parser
- `parsing/single_file.py`: split on `---` fences, parse YAML, return
  (TemplateManifest, body_string)
- Handle: missing fences, malformed YAML, empty body, encoding issues
- `parsing/detect.py`: determine format from path (single-file vs directory)

#### 1.3 Payload system
- `payload/loader.py`: CSV → list[dict], JSON → parsed, TSV → list[dict]
- `payload/resolver.py`: merge sources in priority order per OUTPUT_SPEC_FINAL §3.1
- `payload/normalizer.py`: type coercion ("42" → 42), NaN → None

#### 1.4 Validation layer
- `validation/manifest.py`: Pydantic model validation of frontmatter
- `validation/payload_schema.py`: validate payload against template schema
  (required params, types, enums, ranges, required datasets)
- `validation/template_compile.py`: Jinja2 parse check, JSON parse check,
  XML parse check via defusedxml
- `validation/path_safety.py`: reject `..`, absolute paths, symlink escapes

#### 1.5 Theme system
- `themes/base.py`: Theme dataclass with to_vega_config(), to_css_vars(),
  to_svg_data_theme()
- `themes/light.py` and `themes/dark.py`: built-in theme instances
- `themes/loader.py`: stub for custom theme loading (implemented in Phase 4)

#### 1.6 Renderer infrastructure
- `renderers/base.py`: RendererAdapter protocol
- `renderers/registry.py`: engine type → adapter dispatch

#### 1.7 Vega-Lite renderer
- `renderers/vega_lite.py`: Jinja2 render → JSON parse → inject canvas →
  apply theme → vl-convert to SVG/PNG
- Supports: SVG, PNG (HTML deferred to Phase 4 bundling)

#### 1.8 SVG renderer
- `renderers/svg_engine.py`: Jinja2 render → defusedxml parse → validate
  root `<svg>` + viewBox → inject data-theme → set dimensions → cairosvg for PNG
- Rasterization fallback flag for browser rendering (wired in Phase 2)

#### 1.9 Output system
- `output/writer.py`: file naming logic, directory creation
- `output/optimizer.py`: lightweight SVG normalization
- `output/metadata.py`: provenance embedding (SVG comment, PNG EXIF)

#### 1.10 CLI commands: render, validate, inspect
- `render`: full pipeline parse → validate → resolve → render → write
- `validate`: parse + validate, no render, exit 0/1
- `inspect`: parse + print Rich-formatted metadata table
- CLI output formatting per OUTPUT_SPEC_FINAL §15
- `--dry-run`, `--verbose`, `--quiet`, `--strict` flags

#### 1.11 Test suite
- Unit tests: models, parser, resolver, validation, path_safety, themes
- Integration tests: vega-lite render (SVG + PNG), svg render, CLI commands
- Fixture templates in `tests/fixtures/{vegalite,svg}/`
- Target: >80% coverage on Phase 1 code

### Definition of done
- `vizli render bar.vizli.json --format svg` → valid SVG output
- `vizli render bar.vizli.json --format png` → PNG with correct dimensions
- `vizli render bar.vizli.json --theme dark` → dark theme applied
- `vizli render diagram.vizli.svg --format png` → PNG via cairosvg
- `vizli validate` catches: missing params, malformed YAML, bad types
- `vizli inspect` prints: template ID, engine, outputs, canvas, schema
- Error messages include template path, failing field, and hint
- Payload resolution order correct (CLI > file > sample)
- All tests pass, CI green

---

## 7. Phase 2 — Browser Engines

**Goal**: Render HTML and JS templates via Playwright. SVG extraction
from rendered pages. PNG screenshotting. Directory package support.

**Key risk**: Playwright CI flakiness on different platforms. Mitigate
by running browser tests as a separate CI job with retry, and marking
them with a `@pytest.mark.browser` marker so they can be skipped.

### Work packages

#### 2.1 Directory package parser
- `parsing/directory.py`: detect `vizli.template.json`, parse manifest,
  resolve schema/sample/entrypoint paths, validate path safety
- Update `parsing/detect.py` for directory detection

#### 2.2 Playwright browser orchestration
- `html/browser.py`: launch Chromium, isolated contexts, network disabled
  via route.abort(), viewport/scale/color_scheme, console error capture,
  timeout enforcement, browser reuse within command

#### 2.3 Readiness resolution
- `html/readiness.py`: three-tier resolution per TEMPLATE_SPEC_FINAL §8.1
  (custom event → CSS selector → window flag → timeout)

#### 2.4 SVG extraction
- `html/extraction.py`: three-tier extraction per TEMPLATE_SPEC_FINAL §8.2
  (selector → getter function → auto-detect → ExtractionError)

#### 2.5 HTML renderer
- `renderers/html_engine.py`: Jinja2 render → inject payload `<script>` →
  write temp file → Playwright load → readiness wait → outputs
  (HTML file / PNG screenshot / SVG extraction / PDF print-to-PDF)

#### 2.6 JS engine harness and renderer
- `html/harness.py`: generate HTML wrapper with container, D3 v7 local
  bundle, payload injection, user JS module, render() call
- `renderers/js_engine.py`: generate harness → Playwright → readiness →
  SVG extraction + PNG screenshot
- Bundle `d3.v7.min.js` in `src/vizli/assets/`

#### 2.7 Extend inspect for directory packages
- Show: entrypoint path, schema file, sample payload, asset dir, golden paths

#### 2.8 Test suite
- Unit tests: readiness tiers, extraction tiers, directory parser
- Integration tests: HTML render (PNG), JS render (SVG + PNG), network
  isolation, timeout enforcement
- Browser tests marked with `@pytest.mark.browser`
- Fixture templates in `tests/fixtures/{html,js,directory_pkg}/`

### Definition of done
- `vizli render calc.vizli.html --format html` → working HTML file
- `vizli render calc.vizli.html --format png` → screenshot PNG
- `vizli render bar.vizli.js --format svg` → extracted SVG
- Network requests blocked and logged as warnings
- Console errors captured and surfaced in render result
- Readiness timeout → clear TimeoutError with hint
- Directory packages parse and render
- D3 loads from local bundle (no network)
- All tests pass, CI green

---

## 8. Phase 3 — Full Engine Suite

**Goal**: Altair engine with sandboxed execution, golden testing
framework, engine contract validation, parametrized cross-engine
test suite. All 5 engines operational.

**Key risk**: Import guard completeness for Altair. Mitigate with an
explicit allowlist (not blocklist) approach — only permitted modules
can be imported.

### Work packages

#### 3.1 Altair renderer
- `renderers/altair_engine.py`: importlib module loading, import guard
  (ALLOWED_IMPORTS allowlist), call build_chart(), validate return type,
  delegate to vega-lite renderer for output
- Security: constrained imports, no exec(), logged warnings

#### 3.2 Engine contract validation
- `validation/engine_contract.py`: engine-specific rules per
  OUTPUT_SPEC_FINAL §8.4 (html+svg needs extraction, svg needs viewBox,
  altair needs build_chart, readiness required for html/js, outputs must
  be subset of engine supported_outputs)

#### 3.3 Golden testing framework
- `testing/diff.py`: SVG normalization + structural comparison, PNG
  pixel-level comparison with tolerance, HTML DOM comparison
- `testing/golden.py`: golden_check() and golden_update() functions

#### 3.4 `vizli golden` command
- `--check`: render sample → compare against golden/ → pass/fail with diff
- `--update`: render sample → overwrite golden/
- Batch mode: check all templates in a directory

#### 3.5 Parametrized fixture suite
- Test fixtures for all 5 engine types (2-3 per engine)
- Each fixture has sample payload and expected outputs
- Parametrized pytest runs every fixture through render contract
- Cross-engine consistency verification

#### 3.6 Test suite
- Unit tests: altair import guards, engine contract validation, SVG
  normalization, PNG diff
- Integration tests: altair render, golden check/update workflow
- Cross-engine parametrized tests

### Definition of done
- `vizli render chart.vizli.py --format svg` → Altair template renders
- Import guard blocks os, subprocess, socket (test verified)
- `vizli golden --check` passes for unmodified template
- `vizli golden --check` fails after template modification
- `vizli golden --update` regenerates golden files
- SVG comparison ignores whitespace and attribute ordering
- PNG comparison respects visual_diff_tolerance
- Engine contract validation catches misconfigurations
- All 5 engines pass parametrized fixture suite
- All tests pass, CI green

---

## 9. Phase 4 — Workflow Tools

**Goal**: Developer productivity tools — convert (LLM), serve (live
reload), render-all (batch), PDF output, watch mode, custom themes,
offline HTML bundling, SVG optimization.

**Key risk**: LLM API integration for `vizli convert` requires careful
prompt engineering and response parsing. Mitigate with mock-based tests
and manual validation of convert output quality.

### Work packages

#### 4.1 PDF output
- `output/pdf.py`: SVG → PDF via weasyprint, HTML → PDF via Playwright
  print-to-PDF
- Wire into renderer supported_outputs

#### 4.2 Custom theme loading
- `themes/loader.py`: load YAML from `~/.config/vizli/themes/`, parse
  into Theme, register by name, available via `--theme <name>`

#### 4.3 `vizli render-all`
- Discover templates in directory, find matching payloads, batch render,
  summary report (passed/failed/skipped)

#### 4.4 Watch mode
- `watch.py`: watchdog monitors template + data + payload files,
  re-renders on change, combined with `--preview` opens viewer

#### 4.5 `vizli serve`
- `serve.py`: HTTP server + WebSocket live-reload for HTML templates,
  watches for changes, re-renders and pushes updates

#### 4.6 `vizli convert`
- `convert/llm_converter.py`: Anthropic SDK, sends raw file +
  TEMPLATE_SPEC + SIDECAR_SPEC as context, parses response into
  template + sidecar, validates with `vizli validate`
- API key from env or `~/.config/vizli/config.yaml`

#### 4.7 Offline HTML bundling
- `output/bundler.py`: inline all CSS/JS assets, bundle Vega runtime
  for vega-lite/altair HTML output, no CDN references

#### 4.8 SVG optimization
- Extend `output/optimizer.py`: `--optimize` shells to svgo if installed,
  `--png-optimize` shells to oxipng if installed

#### 4.9 Test suite
- Tests for PDF output, batch rendering, watch mode, serve, bundling
- Convert tests with mocked LLM API

### Definition of done
- `vizli render chart.vizli.json --format pdf` → valid PDF
- `vizli convert raw.json` → valid template + sidecar (manual quality check)
- `vizli serve` → server starts, live-reload works
- `vizli render-all` → batch renders all templates in directory
- `--watch` re-renders on file change
- `--theme brand` loads custom YAML theme
- HTML output fully offline (no external URLs)
- All tests pass, CI green

---

## 10. Phase 5 — Template Library and Skill Integration

**Goal**: Production template library with sidecars, Claude Code
rendering skill, end-to-end agent workflow validation.

**Key risk**: Template quality at scale — each template must pass the
21-item acceptance checklist in TEMPLATE_SPEC_FINAL §16. Mitigate by
building templates incrementally and validating each one before moving
to the next.

### Work packages

#### 5.1 Chart templates (vega-lite)
- bar_simple, bar_grouped, bar_stacked
- line_timeseries, line_multiseries
- scatter_simple, scatter_colored
- heatmap
- Each with sidecar, sample payload, golden outputs

#### 5.2 Explainer templates (html)
- ab_test_power (calculator with sliders)
- cuped_variance (variance reduction explainer)
- Each with sidecar, sample payload, golden outputs

#### 5.3 Diagram templates (svg)
- pipeline_flow (data pipeline architecture)
- system_architecture (service diagram)
- Each with sidecar, sample payload, golden outputs

#### 5.4 D3 templates (js)
- force_graph (force-directed network)
- treemap (hierarchical treemap)
- Each with sidecar, sample payload, golden outputs

#### 5.5 Template registry
- `vizli-templates.yaml` at library root
- Lists all templates with path, title, tags

#### 5.6 Claude Code rendering skill
- Skill file wrapping VIZLI_README.md workflow
- Sidecar search → data inspection → param resolution → vizli render →
  verify output → deliver

#### 5.7 Explainer skill migration evaluation
- Assess: can the existing explainer skill produce vizli html-type
  templates instead of raw HTML?
- Decision: migrate if clear value, keep independent otherwise

#### 5.8 End-to-end validation
- Manual test: agent receives "chart my revenue data" → correct output
- Test across: chart, explainer, diagram request types
- Validate skill + templates + CLI work together

#### 5.9 Documentation
- README.md: installation, quickstart, examples
- Clean up: remove v0 spec files once FINAL confirmed authoritative

### Definition of done
- 14+ templates pass `vizli validate` and `vizli golden --check`
- Every template has complete sidecar per SIDECAR_SPEC
- Templates render in both light and dark themes
- Template registry valid and complete
- Rendering skill invokes `vizli render` successfully
- End-to-end agent workflow produces correct output
- Documentation complete

---

## 11. Cross-cutting concerns

### Exit codes (all phases)

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Template, validation, or render error |
| 2 | CLI usage error |
| 3 | Missing dependency |
| 4 | Timeout |

### Security (all phases)

- Network isolation for browser rendering (Playwright route.abort)
- Path safety validation for all file access
- XML safety via defusedxml for SVG
- Constrained imports for Altair (allowlist, not blocklist)
- No `exec()` anywhere
- Browser contexts: ephemeral, isolated, sandboxed, timeout-bounded

### Logging (all phases)

- Default: `✓ revenue.vizli.json → revenue.png (800×500, 1.2s)`
- `--verbose`: full diagnostics block with all resolved values
- `--quiet`: errors only
- `--trace`: browser trace files, intermediate JSON/SVG, console logs

### Performance (all phases)

Priority order: correctness > predictability > speed.

Acceptable caching: manifest cache within process, Jinja2 compiled
templates, browser reuse within command, memoized asset reads.

NOT in v1: daemon mode, persistent browser pools, cross-command caching,
speculative rendering.

---

## 12. Open questions

| # | Question | Affects | Decision point |
|---|---|---|---|
| Q1 | hatchling vs setuptools for build? | Phase 0 | During 0.1 |
| Q2 | Should vega-lite HTML output go in Phase 1 or Phase 4 (bundling)? | Phase 1/4 | During 1.7 |
| Q3 | Should browser tests run in a separate CI job? | Phase 2 | During 2.8 |
| Q4 | Altair import guard: allowlist vs blocklist? | Phase 3 | During 3.1 |
| Q5 | How to bundle D3 — package_data or importlib.resources? | Phase 2 | During 2.6 |
| Q6 | vizli convert: single LLM call or multi-turn agent? | Phase 4 | During 4.6 |
| Q7 | Should the explainer skill migrate to vizli templates? | Phase 5 | During 5.7 |

---

## 13. Issue structure

GitHub issues follow this hierarchy:

```
Milestone (M0-M5)
  └─ Epic issue (one per phase, tracks all work in phase)
       └─ Task issues (one per work package, implementable in 1-2 sessions)
```

Labels:
- `phase:0` through `phase:5` — which phase
- `type:infrastructure` — project setup, CI, tooling
- `type:engine` — rendering engine implementation
- `type:cli` — CLI command implementation
- `type:validation` — validation and schema work
- `type:testing` — test infrastructure and test suites
- `type:templates` — template library content
- `type:integration` — skill and agent integration
- `type:docs` — documentation
- `priority:critical` — blocks other work
- `priority:normal` — standard priority

---

## 14. Summary

| Phase | Milestone | Delivers | Key risk | Mitigation |
|---|---|---|---|---|
| 0 | M0: Bootstrap | Package, CLI, CI, doctor | None | — |
| 1 | M1: Core Rendering | VL + SVG rendering, validation | vl-convert API | Test first |
| 2 | M2: Browser Engines | HTML + JS via Playwright | CI flakiness | Separate job, retry |
| 3 | M3: Full Engine Suite | Altair, golden testing | Import guards | Allowlist approach |
| 4 | M4: Workflow Tools | Convert, serve, PDF, watch | LLM integration | Mock tests |
| 5 | M5: Template Library | Templates, skill, E2E | Quality at scale | Incremental validation |

The first useful tool emerges at M1. By M3, vizli is a complete
rendering pipeline for all 5 engine types with regression testing.
M4 and M5 add workflow convenience and the content that makes vizli
valuable to agents.
