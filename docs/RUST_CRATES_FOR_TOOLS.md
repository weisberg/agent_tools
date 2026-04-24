# agent_tools Crate Research

Targeted crate recommendations for each tool in the agent_tools toolkit, plus shared dependencies across the workspace.

---

## Shared Foundation (all five tools)

These crates form the common substrate. Every tool in agent_tools should depend on the same versions via workspace-level `[dependencies]`.

| Crate | Role |
|---|---|
| `clap` (derive) | Argument parsing, subcommands, shell completions |
| `serde` + `serde_json` | JSON stdin/stdout agent protocol, config files |
| `toml` | Config file format |
| `thiserror` | Typed error enums per tool |
| `color-eyre` | Pretty error reports with backtraces for the binary entrypoints |
| `tracing` + `tracing-subscriber` | Structured logging, `RUST_LOG` env filter |
| `dirs` | Platform-correct `~/.config/`, `~/.cache/`, `~/.local/share/` paths |
| `chrono` | Timestamps in metadata, logs, file headers |
| `strum` | Enum ↔ string derives for subcommand variants, output formats, status values |

---

## docli — Word Documents CLI

### Core Document Engine

| Crate | Purpose | Notes |
|---|---|---|
| `quick-xml` | Read/write/edit OOXML (the XML inside .docx) | The docx format is a ZIP of XML files. This is your low-level workhorse for direct XML manipulation — tracked changes, comments, style editing |
| `zip` | Read/write the .docx ZIP container | Unpack to edit XML, repack to produce .docx |
| `docx-rs` | Higher-level docx creation API | Pure Rust, generates docx from Rust structs. Good for new document creation but limited for editing existing docs |
| `pandoc` (binary) | Format conversion (md → docx, docx → md, docx → pdf) | Shell out to pandoc for heavy-lift conversions. Not a crate — a system dependency |

### Content Processing

| Crate | Purpose | Notes |
|---|---|---|
| `minijinja` | Jinja2-compatible template rendering | Already proven in clipli spec. Use for docx templates with variable substitution |
| `pulldown-cmark` | Markdown parsing | If docli reads markdown input to produce docx, this is the standard parser |
| `regex` | Pattern matching in document content | Find-and-replace, content extraction |
| `similar` | Diff generation | Compare document versions, show what changed between revisions |

### Supporting

| Crate | Purpose | Notes |
|---|---|---|
| `base64` | Embed images in documents | Inline image data for self-contained docx |
| `image` | Image format detection/conversion | Resize, convert PNG/JPEG for document embedding |
| `sha2` or `blake3` | Content hashing | Deduplicate images, track document identity |
| `walkdir` | Scan template directories | Find templates in `~/.config/docli/templates/` |

---

## xli — Excel Documents CLI

### Core Spreadsheet Engine

| Crate | Purpose | Notes |
|---|---|---|
| `calamine` | Fast xlsx/xls/ods reading | Pure Rust, 2.5x faster than alternatives for bulk reads. Read-only — no write support. Serde integration for deserializing rows directly into structs |
| `rust_xlsxwriter` | Fast xlsx creation | Used by `xli create`, including CSV/Markdown/JSON imports and report-table creation options. Strong for new workbooks, charts, conditional formatting, data validation, sparklines, and images. Write-only — cannot modify existing files |
| `umya-spreadsheet` | Read + write + modify xlsx | Current mutation fallback for `write`, `format`, `sheet`, `batch`, and `apply`. Practical for MVP editing, but `xli` intentionally warns because unrelated workbook artifacts may be rewritten |
| `csv` | CSV read/write with serde | Import/export CSV. Handles quoting, escaping, flexible delimiters. Used by `xli create --from-csv` and column-name report options |
| `schemars` | JSON Schema generation | Used for structured command/result schema output. Some command schemas are still maintained manually until CLI-type-derived schemas are complete |
| `quick-xml` + `zip` | OOXML package inspection/patching | Existing helper crates for artifact-preserving OOXML work. Full mutation coverage is still active work |

### Strategy Note

xli currently composes the spreadsheet crates directly: `calamine` for reads and inspection, `rust_xlsxwriter` for new workbook creation, and `umya-spreadsheet` as the mutation fallback. The long-term direction is to move common mutations onto artifact-preserving OOXML patch paths using `zip` and `quick-xml`, keeping fallback usage explicit in response warnings.

### Data Processing

| Crate | Purpose | Notes |
|---|---|---|
| `jaq-core` + `jaq-std` + `jaq-json` | jq filter engine | Already specced for bashli. Reuse in xli for `xli query --filter '.rows[] | select(.revenue > 1000)'` on JSON-exported sheet data |
| `polars` | DataFrame operations | Heavy dependency but extremely powerful for transforms, aggregations, pivots on large datasets. Optional feature gate |
| `comfy-table` | Terminal table rendering | Display spreadsheet data in the terminal with auto-wrapping columns |
| `tabled` | Alternative table rendering with derive | `#[derive(Tabled)]` on row structs for quick display |

### Formatting & Output

| Crate | Purpose | Notes |
|---|---|---|
| `chrono` | Date/time cell formatting | Parse Excel serial dates, format for display |
| `rust_decimal` | Precise decimal arithmetic | Avoid float rounding in financial data. `rust_xlsxwriter` has a `rust_decimal` feature |
| `num-format` | Locale-aware number formatting | Format numbers with thousand separators, currency symbols for terminal display |
| `humantime` | Duration formatting | Display time-based cell values in human-readable form |

---

## vizli — Visualization CLI

### Chart Generation

| Crate | Purpose | Notes |
|---|---|---|
| `plotters` | Primary chart rendering library | Pure Rust. Line, bar, scatter, histogram, heatmap, area, box plots. Renders to SVG, PNG (via bitmap backend), and even terminal. The most mature charting crate in Rust |
| `plotters-svg` | SVG backend for plotters | Cleanest output for agent consumption — SVG is text, diffable, embeddable |
| `plotters-bitmap` | PNG/BMP backend for plotters | For raster output when SVG isn't suitable |

### Diagram Generation

| Crate | Purpose | Notes |
|---|---|---|
| `mermaid` (shell out) | Generate diagrams from Mermaid DSL | Mermaid CLI (`mmdc`) renders flowcharts, sequence diagrams, Gantt charts. Not a Rust crate — shell out to the binary or use the JS engine |
| `d2` (shell out) | D2 declarative diagrams | Your flowchart conversation identified D2 as the best fit for swim lanes and process docs. Shell out to the `d2` binary |
| `svgbob` | ASCII art → SVG conversion | Pure Rust. Converts ASCII box drawings into clean SVG diagrams. Good for agent-generated diagrams that start as text |

### SVG & Image Processing

| Crate | Purpose | Notes |
|---|---|---|
| `svg` | SVG DOM construction | Build SVGs programmatically when plotters is too opinionated. Direct control over elements, attributes, transforms |
| `resvg` | SVG rendering to raster | Convert SVG → PNG with high fidelity. Uses `tiny-skia` as its 2D rendering engine. Useful for `vizli export --format png` |
| `tiny-skia` | 2D rendering engine | Pure Rust pixel-level rendering. The backend behind resvg. Use directly for custom drawing operations |
| `image` | Image format encode/decode | PNG, JPEG, GIF, WebP read/write. Needed for raster output formats |

### Data Input

| Crate | Purpose | Notes |
|---|---|---|
| `csv` | Read CSV data for charting | Same crate as xli — shared dependency |
| `serde_json` | Read JSON data for charting | Already in shared foundation |
| `calamine` | Read xlsx data for charting | Pull chart data directly from spreadsheets |

---

## clipli — Clipboard CLI

### Pasteboard Access (macOS)

| Crate | Purpose | Notes |
|---|---|---|
| `objc2` | Objective-C runtime bindings | Foundation for macOS pasteboard access |
| `objc2-app-kit` | AppKit bindings (NSPasteboard) | Read/write `public.html`, `public.utf8-plain-text`, and custom pasteboard types. This is the core of clipli's platform layer |
| `objc2-foundation` | Foundation type bindings | NSString, NSData, NSArray conversions |

### HTML Processing

| Crate | Purpose | Notes |
|---|---|---|
| `lol_html` | Streaming HTML rewriter | Cloudflare's low-output-latency HTML rewriter. Specified in the clipli spec for the 10-stage HTML cleaning pipeline. Handles tag removal, attribute stripping, CSS inlining without building a full DOM |
| `html5ever` | Full HTML5 parser | When lol_html's streaming model isn't enough — e.g., for template extraction where you need to traverse the full DOM tree |
| `markup5ever` | Shared HTML/XML types | Underlying type system for html5ever. You'll need it for node manipulation |
| `scraper` | CSS selector-based HTML querying | Built on html5ever. Use for extracting specific elements by CSS selector during the templatize workflow |

### Templating

| Crate | Purpose | Notes |
|---|---|---|
| `minijinja` | Jinja2-compatible template engine | Already in the clipli spec. Custom filters (`currency`, `pct`, `date_fmt`, `number_fmt`). Shared with docli |

### Supporting

| Crate | Purpose | Notes |
|---|---|---|
| `base64` | Encode/decode embedded images in HTML | Clipboard HTML may contain base64 image data |
| `cssparser` | Parse inline CSS | Extract and manipulate `mso-*` properties from Excel HTML |
| `sha2` or `blake3` | Template/content hashing | Dedup templates in the store |

---

## bashli — Bash Runner CLI

### Embedded Text Processing Engines

| Crate | Purpose | Notes |
|---|---|---|
| `jaq-core` + `jaq-std` + `jaq-json` | jq filter engine (pure Rust) | Already fully specced. Audited by Radically Open Security. ~95% jq compatibility |
| `sedregex` | sed-style regex substitution | Pure Rust sed implementation for the bashli-sed crate |
| `awk` / custom | awk processing | Less mature crate options — may need a custom implementation or the `gawk` binary as a bridge |

### Process Execution

| Crate | Purpose | Notes |
|---|---|---|
| `tokio` | Async runtime | Process spawning, timeouts, I/O routing all need async. The bashli-runner crate's foundation |
| `tokio::process` | Async child process management | Spawn commands, capture stdout/stderr separately, apply timeouts |
| `nix` | POSIX signal handling, process groups | Send SIGTERM/SIGKILL, manage process groups for cleanup |
| `signal-hook` | Signal handlers | Register handlers for SIGHUP, SIGTERM during long-running bashli pipelines |

### I/O & Data

| Crate | Purpose | Notes |
|---|---|---|
| `tempfile` | Temporary files for step I/O | Intermediate outputs between bashli steps |
| `which` | Find executables on PATH | Validate that commands exist before running them |
| `shell-words` | Shell word splitting/quoting | Parse and escape shell arguments correctly |

### Budget & Timing

| Crate | Purpose | Notes |
|---|---|---|
| `humantime` | Parse/display durations | "30s", "5m", "1h" timeout strings in bashli step definitions |
| `governor` | Rate limiting | If bashli steps need rate-limited API calls |

---

## Cross-Tool Integration Crates

These enable the tools to work together and with agents:

| Crate | Purpose | Which Tools |
|---|---|---|
| `minijinja` | Template engine | docli, clipli (shared templates) |
| `jaq-core` ecosystem | jq filtering | bashli, xli (JSON query on any output) |
| `csv` | Tabular data interchange | xli, vizli (shared data format) |
| `calamine` | Read xlsx | xli, vizli (chart from spreadsheet) |
| `comfy-table` | Terminal table display | xli, docli (preview content) |
| `similar` | Diffing | docli, clipli (compare versions) |
| `quick-xml` | XML manipulation | docli (OOXML), xli (xlsx internals share OOXML) |

---

## Crates That Should NOT Be in the List

For completeness — these were in the generic survey but are **not relevant** to agent_tools:

| Crate | Why Not |
|---|---|
| `ratatui` / `cursive` | Full TUI frameworks. agent_tools are pipe-oriented CLIs, not interactive TUIs |
| `dialoguer` / `inquire` | Interactive prompts. Agent-native tools use JSON stdin/stdout, not human prompts |
| `reqwest` | HTTP client. None of the five tools make HTTP calls (that's a concern for the agent/tooli layer above) |
| `rusqlite` / `redb` / `sled` | Embedded databases. agent_tools are stateless file processors, not daemons |
| `keyring` | Credential storage. No auth in these tools |
| `ring` / `rustls` | Cryptography/TLS. No network layer |
| `self_update` / `cargo-dist` | Distribution. Handled at the tooli workspace level, not per-tool |

---

*Targeted for the agent_tools toolkit: docli, xli, vizli, clipli, bashli. March 2026.*
