# clipli — Clipboard Intelligence CLI

**Version:** 1.0.0-spec
**Status:** Draft
**Author:** Brian (spec), Claude (co-author)
**Language:** Rust
**License:** MIT

---

## 1. Vision

clipli is a Rust CLI that turns the macOS pasteboard into a programmable, template-driven interface for agents and power users. It captures rich clipboard content (HTML, RTF, images) from any application, converts it to reusable Jinja2 templates, and pastes rendered templates back with full formatting preserved.

**The core loop:**

```
Copy from App → clipli capture → templatize → agent fills data → clipli paste → Paste into App
```

**Design principles:**

- **Agent-native.** JSON-over-stdin/stdout. Every subcommand is composable in a pipeline.
- **Template-first.** Captured content becomes a reusable asset, not a one-shot clipboard event.
- **Lossless round-trip.** What you copy out of PowerPoint should paste back into PowerPoint with identical formatting.
- **Zero-runtime dependencies.** Pure Rust + macOS system frameworks. No Python, no Node, no web engine.

---

## 2. Architecture

### 2.1 Crate Layout

Single crate, binary target, six modules:

```
clipli/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entrypoint (clap)
│   ├── pb.rs                # Pasteboard FFI (NSPasteboard via objc2)
│   ├── clean.rs             # HTML sanitizer pipeline
│   ├── render.rs            # Template engine (minijinja)
│   ├── templatize.rs        # Literal → variable extraction
│   ├── store.rs             # Template storage manager
│   └── model.rs             # Shared data types
├── templates/
│   ├── _base.html.j2        # Base template with common boilerplate
│   ├── table_default.html.j2
│   ├── table_striped.html.j2
│   └── slide_default.html.j2
└── tests/
    ├── fixtures/             # Sample pasteboard HTML from Excel, PPT, etc.
    ├── pb_tests.rs
    ├── clean_tests.rs
    ├── render_tests.rs
    └── templatize_tests.rs
```

### 2.2 Module Dependency Graph

```
main.rs
  ├── pb         (reads/writes pasteboard)
  ├── clean      (sanitizes captured HTML)
  ├── render     (fills templates via minijinja)
  ├── templatize (extracts variables from HTML)
  ├── store      (manages template filesystem)
  └── model      (shared types, used by all)
```

No circular dependencies. `model` is a leaf. `pb` has no internal dependencies.

---

## 3. Data Model (`model.rs`)

### 3.1 Pasteboard Types

```rust
/// Recognized pasteboard UTI types, in priority order for capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PbType {
    Html,          // public.html
    Rtf,           // public.rtf
    PlainText,     // public.utf8-plain-text
    Png,           // public.png
    Tiff,          // public.tiff
    Pdf,           // com.adobe.pdf
    Unknown,       // anything else (logged but not processed)
}

impl PbType {
    pub fn uti(&self) -> &'static str {
        match self {
            Self::Html      => "public.html",
            Self::Rtf       => "public.rtf",
            Self::PlainText => "public.utf8-plain-text",
            Self::Png       => "public.png",
            Self::Tiff      => "public.tiff",
            Self::Pdf       => "com.adobe.pdf",
            Self::Unknown   => "unknown",
        }
    }
}

/// Raw pasteboard snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbSnapshot {
    pub types: Vec<PbTypeEntry>,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub source_app: Option<String>,   // from NSPasteboard owner, if available
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbTypeEntry {
    pub pb_type: PbType,
    pub uti: String,
    pub size_bytes: usize,
    pub data: Vec<u8>,
}
```

### 3.2 Template Metadata

```rust
/// Stored alongside every captured template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMeta {
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub source_app: Option<String>,
    pub source_pb_types: Vec<String>,       // UTIs present at capture time
    pub templatized: bool,                  // was variable extraction performed?
    pub variables: Vec<TemplateVariable>,   // extracted template variables
    pub tags: Vec<String>,                  // user-defined tags for search
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub var_type: VarType,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarType {
    String,
    Number,
    Currency,
    Percentage,
    Date,
    Boolean,
    List,
}
```

### 3.3 Table Model (for structured input)

```rust
/// Agent-friendly table input format for `clipli paste --from-table`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInput {
    pub headers: Option<Vec<Cell>>,
    pub rows: Vec<Vec<Cell>>,
    pub style: Option<TableStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub value: String,
    #[serde(default)]
    pub style: CellStyle,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellStyle {
    pub font_family: Option<String>,
    pub font_size_pt: Option<f32>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    pub fg_color: Option<String>,     // hex, e.g. "#1A3E6F"
    pub bg_color: Option<String>,
    pub alignment: Option<Align>,
    pub border: Option<BorderStyle>,
    pub colspan: Option<u32>,
    pub rowspan: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align { Left, Center, Right }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderStyle {
    pub color: Option<String>,
    pub width_px: Option<f32>,
    pub style: Option<String>,        // solid, dashed, dotted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStyle {
    pub default_font: Option<String>,
    pub default_font_size_pt: Option<f32>,
    pub header_bg: Option<String>,
    pub header_fg: Option<String>,
    pub stripe_even_bg: Option<String>,
    pub border_collapse: Option<bool>,
}
```

---

## 4. CLI Surface (`main.rs`)

```
clipli <SUBCOMMAND> [OPTIONS]
```

### 4.1 Subcommands

#### `clipli inspect`

Show all pasteboard types currently on the clipboard.

```
clipli inspect [--json]
```

**Output (default):**
```
Pasteboard contents (3 types):
  public.html             12,847 bytes
  public.rtf               8,203 bytes
  public.utf8-plain-text     412 bytes
Source app: com.microsoft.PowerPoint
```

**Output (--json):**
```json
{
  "types": [
    {"uti": "public.html", "size_bytes": 12847},
    {"uti": "public.rtf", "size_bytes": 8203},
    {"uti": "public.utf8-plain-text", "size_bytes": 412}
  ],
  "source_app": "com.microsoft.PowerPoint"
}
```

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | false | JSON output for agent consumption |

---

#### `clipli capture`

Read clipboard, clean HTML, optionally templatize, and save to template store.

```
clipli capture --name <NAME> [OPTIONS]
```

**Pipeline:**
1. Read `public.html` from pasteboard (fall back to `public.rtf` → plain text)
2. Run HTML through `clean` pipeline
3. If `--templatize`: run through `templatize` module
4. Save to store: `~/.config/clipli/templates/<NAME>/`

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--name` / `-n` | string | **required** | Template name (slug-friendly) |
| `--templatize` / `-t` | bool | false | Extract variables from literal values |
| `--strategy` | enum | `heuristic` | Templatization strategy: `heuristic`, `agent`, `manual` |
| `--description` / `-d` | string | none | Human-readable description |
| `--tags` | string[] | [] | Comma-separated tags |
| `--force` / `-f` | bool | false | Overwrite existing template |
| `--raw` | bool | false | Skip HTML cleaning (save as-is) |
| `--keep-classes` | bool | false | Preserve CSS class attributes during cleaning |
| `--preview` | bool | false | Open cleaned HTML in browser before saving |
| `--json` | bool | false | Output capture result as JSON |

**Templatization strategies:**

- **`heuristic`** — Rule-based extraction. Detects dates, currency, percentages, email addresses, proper nouns, and numeric values. Replaces with `{{var_N}}` and writes a `.schema.json` with inferred types. Fast, deterministic, but limited.

- **`agent`** — Pipes cleaned HTML to stdout with a structured prompt. The calling agent (or a piped LLM CLI) returns templatized HTML + schema. clipli parses the response and saves both. This is the high-quality path.

  Agent protocol (stdout when `--strategy agent`):
  ```json
  {
    "action": "templatize",
    "html": "<cleaned HTML content>",
    "prompt": "Identify dynamic content in this HTML captured from {source_app}. Replace dynamic values with Jinja2 variables using descriptive names. Keep all inline CSS intact. Return JSON with keys: template (the templatized HTML string), variables (array of {name, type, default_value, description})."
  }
  ```

  Expected response (stdin):
  ```json
  {
    "template": "<td style=\"...\">{{title}}</td>...",
    "variables": [
      {"name": "title", "type": "string", "default_value": "Q3 Results", "description": "Slide title"}
    ]
  }
  ```

- **`manual`** — Saves cleaned HTML as `.html` (not `.html.j2`). User edits it into a template by hand.

---

#### `clipli paste`

Render a template with data and write the result to the pasteboard.

```
clipli paste <NAME> [OPTIONS]
```

**Pipeline:**
1. Load template from store
2. Merge data from `--data` flag, `--data-file`, or stdin
3. Render with minijinja
4. Write `public.html` + `public.utf8-plain-text` (plain text fallback) to pasteboard

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `<NAME>` | positional | **required** | Template name |
| `--data` / `-D` | string | none | Inline JSON data, e.g. `-D '{"title":"Q4"}'` |
| `--data-file` | path | none | Path to JSON file with template data |
| `--stdin` | bool | false | Read JSON data from stdin |
| `--dry-run` | bool | false | Print rendered HTML to stdout instead of writing to pasteboard |
| `--plain-text` | enum | `auto` | Plain text strategy: `auto` (strip tags), `tab-delimited` (table-aware), `none` (skip) |
| `--open` | bool | false | Also open rendered HTML in default browser for preview |

**Data merge precedence:** `--data` overrides `--data-file` overrides stdin. Missing variables use defaults from `.schema.json`. Undefined variables with no defaults produce a minijinja error (fail-fast, not silent).

---

#### `clipli paste --from-table`

Render a table directly from structured JSON input (no named template needed).

```
clipli paste --from-table [OPTIONS]
```

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--from-table` | bool | false | Read TableInput JSON from stdin |
| `--template` / `-t` | string | `table_default` | Built-in table template to use |

**Stdin format:** A `TableInput` JSON object (see §3.3).

---

#### `clipli list`

List all saved templates.

```
clipli list [OPTIONS]
```

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--tag` | string | none | Filter by tag |
| `--json` | bool | false | JSON output |
| `--verbose` / `-v` | bool | false | Include variable names and descriptions |

**Output (default):**
```
Templates (4):
  quarterly_report_header   templatized  3 vars   [vanguard, slides]
  kpi_table                 templatized  8 vars   [vanguard, tables]
  email_banner              raw          0 vars   [email]
  team_roster               templatized  2 vars   [tables]
```

---

#### `clipli show`

Display details of a specific template.

```
clipli show <NAME> [OPTIONS]
```

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `<NAME>` | positional | **required** | Template name |
| `--html` | bool | false | Print raw template HTML to stdout |
| `--schema` | bool | false | Print variable schema as JSON |
| `--meta` | bool | false | Print metadata as JSON |
| `--open` | bool | false | Render with defaults and open in browser |

---

#### `clipli edit`

Open a template in `$EDITOR` for manual editing.

```
clipli edit <NAME>
```

Opens the `.html.j2` (or `.html`) file. On save, clipli validates Jinja2 syntax and updates `updated_at` in metadata. If the user added new `{{variables}}`, clipli detects them and prompts to update the schema (or auto-updates with `--auto-schema`).

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `<NAME>` | positional | **required** | Template name |
| `--auto-schema` | bool | false | Auto-detect and add new variables to schema |

---

#### `clipli delete`

Remove a template from the store.

```
clipli delete <NAME> [--force]
```

---

#### `clipli read`

Read the current clipboard and output the HTML content to stdout (without saving).

```
clipli read [OPTIONS]
```

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--type` / `-t` | enum | `html` | Pasteboard type to read: `html`, `rtf`, `plain`, `png`, `pdf` |
| `--clean` | bool | false | Run through HTML cleaning pipeline before output |
| `--output` / `-o` | path | stdout | Write to file instead of stdout |

For binary types (`png`, `pdf`, `tiff`), output goes to the file specified by `--output` (required for binary types).

---

#### `clipli write`

Write content from stdin or a file to the pasteboard.

```
clipli write [OPTIONS]
```

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--type` / `-t` | enum | `html` | Pasteboard type: `html`, `rtf`, `plain` |
| `--input` / `-i` | path | stdin | Read from file instead of stdin |
| `--with-plain` | bool | true | Also write plain-text fallback (auto-stripped from HTML) |

---

#### `clipli convert`

Convert between formats.

```
clipli convert --from <FORMAT> --to <FORMAT> [OPTIONS]
```

**Supported conversions:**
| From | To | Method |
|------|----|--------|
| `html` | `j2` | Heuristic or agent templatization |
| `j2` | `html` | Render with provided data |
| `rtf` | `html` | RTF → HTML conversion |
| `html` | `plain` | Strip tags, table-aware tab-delimited |

**Flags:**
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--from` | enum | **required** | Source format |
| `--to` | enum | **required** | Target format |
| `--input` / `-i` | path | stdin | Input file |
| `--output` / `-o` | path | stdout | Output file |
| `--data` / `-D` | string | none | JSON data (for j2 → html) |
| `--strategy` | enum | `heuristic` | For html → j2: `heuristic` or `agent` |

---

## 5. Module Specifications

### 5.1 Pasteboard FFI (`pb.rs`)

**Responsibilities:**
- Read all types from `NSPasteboard.generalPasteboard`
- Write one or more types atomically (single `clearContents` + N `setData` calls)
- Query pasteboard owner (source application bundle ID)
- Return structured `PbSnapshot`

**FFI approach:** `objc2` + `objc2-app-kit` + `objc2-foundation`. No raw `objc_msgSend`. The `objc2` ecosystem provides safe(r) wrappers with compile-time selector checks.

**Key implementation details:**

```rust
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeHTML, NSPasteboardTypeString, NSPasteboardTypeRTF};
use objc2_foundation::{NSArray, NSString, NSData};

pub fn read_all() -> PbSnapshot {
    let pb = unsafe { NSPasteboard::generalPasteboard() };
    let types = unsafe { pb.types() }.unwrap_or_default();
    // iterate types, read data for each, build PbSnapshot
}

pub fn write(entries: &[(PbType, &[u8])]) -> Result<(), PbError> {
    let pb = unsafe { NSPasteboard::generalPasteboard() };
    unsafe { pb.clearContents() };
    for (pb_type, data) in entries {
        // setData:forType: for each entry
    }
    Ok(())
}

pub fn source_app() -> Option<String> {
    // NSPasteboard doesn't directly expose source app reliably.
    // Use NSWorkspace.frontmostApplication as a heuristic captured
    // at read time, or parse com.apple.pasteboard.source if available.
    None // best-effort
}
```

**Error type:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum PbError {
    #[error("pasteboard is empty")]
    Empty,
    #[error("requested type '{0}' not available on pasteboard")]
    TypeNotFound(String),
    #[error("failed to write to pasteboard: {0}")]
    WriteFailed(String),
    #[error("objc runtime error: {0}")]
    ObjcError(String),
}
```

---

### 5.2 HTML Cleaner (`clean.rs`)

**Responsibilities:**
- Strip Microsoft Office cruft from captured HTML
- Normalize inline CSS to the subset that target apps (Excel, Sheets, PowerPoint) respect
- Produce minimal, valid HTML

**Pipeline stages (executed in order):**

| Stage | Description |
|-------|-------------|
| 1. Decode | Handle UTF-8, UTF-16, and Windows-1252 encodings (Office is inconsistent) |
| 2. Strip meta | Remove `<meta>`, `<link>`, `<style>` blocks, `<xml>` islands, `<!--[if ...]>` conditionals |
| 3. Strip mso-* | Remove all `mso-*` CSS properties from inline styles |
| 4. Normalize fonts | Map Office font aliases to web-safe equivalents (e.g., `Calibri` stays, `+mj-lt` → `Calibri`) |
| 5. Collapse empty | Remove empty `<span>`, `<p>`, `<div>` elements with no content or only whitespace |
| 6. Normalize colors | Convert `rgb(R,G,B)` to `#RRGGBB`, strip `windowtext` → `#000000` |
| 7. Strip classes | Remove `class` attributes (unless `--keep-classes`). All styling is inline. |
| 8. Strip IDs | Remove `id` attributes except those needed for internal links |
| 9. Normalize whitespace | Collapse runs of whitespace in text nodes, trim attribute values |
| 10. Validate | Ensure result is well-formed HTML fragment. Repair if needed. |

**Implementation:** Use `lol_html` (Cloudflare's streaming HTML rewriter). Each stage is a handler registered on the rewriter. This is memory-efficient for large clipboard contents and handles malformed markup gracefully.

```rust
use lol_html::{element, rewrite_str, RewriteStrSettings};

pub fn clean(html: &str, opts: &CleanOptions) -> Result<String, CleanError> {
    let result = rewrite_str(html, RewriteStrSettings {
        element_content_handlers: vec![
            element!("meta, link, style, xml", |el| { el.remove(); Ok(()) }),
            element!("*[class]", |el| {
                if !opts.keep_classes { el.remove_attribute("class"); }
                Ok(())
            }),
            element!("*[style]", |el| {
                if let Some(style) = el.get_attribute("style") {
                    el.set_attribute("style", &normalize_css(&style))?;
                }
                Ok(())
            }),
            // ... additional handlers per stage
        ],
        ..Default::default()
    })?;
    Ok(result)
}

pub struct CleanOptions {
    pub keep_classes: bool,
    pub target_app: TargetApp,  // affects which CSS properties to keep
}

#[derive(Debug, Clone, Copy)]
pub enum TargetApp {
    Excel,
    PowerPoint,
    GoogleSheets,
    Generic,
}
```

**CSS properties preserved per target:**

| Property | Excel | PPT | Sheets | Generic |
|----------|-------|-----|--------|---------|
| `font-family` | ✓ | ✓ | ✓ | ✓ |
| `font-size` | ✓ | ✓ | ✓ | ✓ |
| `font-weight` | ✓ | ✓ | ✓ | ✓ |
| `font-style` | ✓ | ✓ | ✓ | ✓ |
| `color` | ✓ | ✓ | ✓ | ✓ |
| `background-color` | ✓ | ✓ | ✓ | ✓ |
| `text-align` | ✓ | ✓ | ✓ | ✓ |
| `text-decoration` | ✓ | ✓ | ✓ | ✓ |
| `border` | ✓ | ✗ | ✓ | ✓ |
| `border-collapse` | ✓ | ✗ | ✓ | ✓ |
| `padding` | ✓ | ✗ | partial | ✓ |
| `width` | ✓ | ✗ | ✓ | ✓ |
| `height` | ✓ | ✗ | partial | ✓ |
| `vertical-align` | ✓ | ✗ | ✓ | ✓ |
| `white-space` | ✓ | ✗ | ✗ | ✓ |

---

### 5.3 Template Renderer (`render.rs`)

**Responsibilities:**
- Load templates from store or built-in defaults
- Render templates with provided data via minijinja
- Produce both HTML and plain-text representations
- Register custom filters

**Core API:**

```rust
use minijinja::{Environment, context, Value};

pub struct Renderer {
    env: Environment<'static>,
}

impl Renderer {
    pub fn new(template_dir: &Path) -> Result<Self, RenderError> {
        let mut env = Environment::new();

        // Load built-in templates (compiled into binary)
        env.add_template("table_default", include_str!("../templates/table_default.html.j2"))?;
        env.add_template("table_striped", include_str!("../templates/table_striped.html.j2"))?;
        env.add_template("slide_default", include_str!("../templates/slide_default.html.j2"))?;
        env.add_template("_base", include_str!("../templates/_base.html.j2"))?;

        // Load user templates from store
        for entry in fs::read_dir(template_dir)? {
            // load .html.j2 files from each template subdirectory
        }

        // Register custom filters
        env.add_filter("currency", filter_currency);
        env.add_filter("pct", filter_pct);
        env.add_filter("date_fmt", filter_date_fmt);
        env.add_filter("number_fmt", filter_number_fmt);
        env.add_filter("default_font", filter_default_font);

        Ok(Self { env })
    }

    pub fn render(
        &self,
        template_name: &str,
        data: &serde_json::Value,
    ) -> Result<RenderedOutput, RenderError> {
        let tmpl = self.env.get_template(template_name)?;
        let html = tmpl.render(data)?;
        let plain = html_to_plain_text(&html);
        Ok(RenderedOutput { html, plain })
    }
}

pub struct RenderedOutput {
    pub html: String,
    pub plain: String,
}
```

**Custom filters:**

| Filter | Input | Output | Example |
|--------|-------|--------|---------|
| `currency` | number | formatted string | `{{ revenue \| currency }}` → `$4,200,000` |
| `pct` | float | percentage string | `{{ rate \| pct }}` → `12.5%` |
| `pct(1)` | float | percentage with decimals | `{{ rate \| pct(1) }}` → `12.5%` |
| `date_fmt` | string | formatted date | `{{ d \| date_fmt("%b %d, %Y") }}` → `Mar 25, 2026` |
| `number_fmt` | number | comma-separated | `{{ n \| number_fmt }}` → `1,234,567` |
| `default_font` | string | fallback font | `{{ font \| default_font("Calibri") }}` |

**Plain text conversion (`html_to_plain_text`):**
- Tables → tab-delimited rows with `\n` separators
- `<br>` / `<p>` → newlines
- `<li>` → `• ` prefix
- Strip all other tags
- Decode HTML entities

---

### 5.4 Templatizer (`templatize.rs`)

**Responsibilities:**
- Analyze cleaned HTML to identify dynamic content
- Replace literal values with Jinja2 template variables
- Produce a variable schema

#### 5.4.1 Heuristic Strategy

Operates as a multi-pass scan over the HTML text content (not tags or attributes):

| Pass | Detection | Regex / Heuristic | Variable Name |
|------|-----------|-------------------|---------------|
| 1 | Dates | `\b\d{1,2}[/-]\d{1,2}[/-]\d{2,4}\b`, month names, ISO dates | `date_N` |
| 2 | Currency | `\$[\d,]+\.?\d*`, `€`, `£` patterns | `currency_N` |
| 3 | Percentages | `\d+\.?\d*%` | `pct_N` |
| 4 | Emails | standard email regex | `email_N` |
| 5 | Large numbers | `\b\d{1,3}(,\d{3})+\b` | `number_N` |
| 6 | Quarters | `Q[1-4]\s*\d{4}` | `quarter_N` |
| 7 | Cell text | Remaining text content in `<td>` / `<th>` elements > 2 chars, not purely structural (like "Total", "Name") | `field_N` |

Each pass produces a `TemplateVariable` with inferred type and the original value as `default_value`.

**Output:** Modified HTML with `{{var_name}}` replacements + a `Vec<TemplateVariable>`.

#### 5.4.2 Agent Strategy

Does not perform extraction itself. Instead:

1. Emits a JSON payload to stdout describing the task (see §4.1 `capture --strategy agent`)
2. Reads the agent's JSON response from stdin
3. Validates the response: checks that the returned template is valid Jinja2, that variable names are valid identifiers, and that the HTML structure is preserved
4. Saves the result

The calling agent is responsible for routing the request to an LLM. This keeps clipli LLM-agnostic.

#### 5.4.3 Manual Strategy

No extraction. Saves cleaned HTML as `.html`. User edits in `$EDITOR`.

---

### 5.5 Template Store (`store.rs`)

**Responsibilities:**
- Manage the filesystem layout under `~/.config/clipli/templates/`
- CRUD operations on templates
- Search and listing

**Filesystem layout:**

```
~/.config/clipli/
├── config.toml              # Global configuration
└── templates/
    ├── quarterly_report_header/
    │   ├── template.html.j2        # The Jinja2 template (or .html if raw)
    │   ├── meta.json               # TemplateMeta
    │   ├── schema.json             # Variable schema (if templatized)
    │   ├── original.html           # Cleaned HTML before templatization (reference)
    │   └── raw.html                # Uncleaned original from pasteboard (archival)
    ├── kpi_table/
    │   ├── template.html.j2
    │   ├── meta.json
    │   ├── schema.json
    │   ├── original.html
    │   └── raw.html
    └── ...
```

**API:**

```rust
pub struct Store {
    root: PathBuf,  // ~/.config/clipli/templates
}

impl Store {
    pub fn new() -> Result<Self, StoreError>;
    pub fn save(&self, name: &str, content: SaveContent) -> Result<(), StoreError>;
    pub fn load(&self, name: &str) -> Result<LoadedTemplate, StoreError>;
    pub fn list(&self, filter: Option<&ListFilter>) -> Result<Vec<TemplateMeta>, StoreError>;
    pub fn delete(&self, name: &str) -> Result<(), StoreError>;
    pub fn exists(&self, name: &str) -> bool;
    pub fn template_path(&self, name: &str) -> PathBuf;
}

pub struct SaveContent {
    pub template_html: String,        // .html.j2 or .html
    pub meta: TemplateMeta,
    pub schema: Option<Vec<TemplateVariable>>,
    pub original_html: Option<String>,
    pub raw_html: Option<String>,
}

pub struct LoadedTemplate {
    pub template_html: String,
    pub meta: TemplateMeta,
    pub schema: Vec<TemplateVariable>,
}

pub struct ListFilter {
    pub tag: Option<String>,
    pub templatized_only: bool,
}
```

---

## 6. Built-in Templates

### 6.1 `_base.html.j2`

```jinja
{# Base template — all table templates extend this #}
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body>
{% block content %}{% endblock %}
</body>
</html>
```

### 6.2 `table_default.html.j2`

```jinja
{% extends "_base.html.j2" %}
{% block content %}
<table style="border-collapse:collapse; font-family:{{default_font | default('Calibri')}}; font-size:{{default_font_size | default('11')}}pt;">
  {% if headers %}
  <tr>
    {% for cell in headers %}
    <td style="
      font-weight:{{cell.style.font_weight | default('bold')}};
      background-color:{{cell.style.bg_color | default('#4472C4')}};
      color:{{cell.style.fg_color | default('#FFFFFF')}};
      padding:6px 10px;
      border:1px solid #999;
      font-size:{{cell.style.font_size_pt | default(11)}}pt;
      text-align:{{cell.style.alignment | default('left')}};
      {% if cell.style.font_family %}font-family:{{cell.style.font_family}};{% endif %}
    ">{{cell.value}}</td>
    {% endfor %}
  </tr>
  {% endif %}
  {% for row in rows %}
  <tr>
    {% for cell in row %}
    <td style="
      {% if cell.style.bold %}font-weight:bold;{% endif %}
      {% if cell.style.italic %}font-style:italic;{% endif %}
      {% if cell.style.bg_color %}background-color:{{cell.style.bg_color}};{% endif %}
      {% if cell.style.fg_color %}color:{{cell.style.fg_color}};{% endif %}
      {% if cell.style.font_family %}font-family:{{cell.style.font_family}};{% endif %}
      font-size:{{cell.style.font_size_pt | default(11)}}pt;
      padding:4px 8px;
      border:1px solid #D9D9D9;
      text-align:{{cell.style.alignment | default('left')}};
    ">{{cell.value}}</td>
    {% endfor %}
  </tr>
  {% endfor %}
</table>
{% endblock %}
```

### 6.3 `table_striped.html.j2`

```jinja
{% extends "_base.html.j2" %}
{% block content %}
<table style="border-collapse:collapse; font-family:{{default_font | default('Calibri')}};">
  {% if headers %}
  <tr>
    {% for cell in headers %}
    <td style="
      font-weight:bold;
      background-color:{{header_bg | default('#4472C4')}};
      color:{{header_fg | default('#FFFFFF')}};
      padding:8px 12px;
      border:1px solid #999;
    ">{{cell.value}}</td>
    {% endfor %}
  </tr>
  {% endif %}
  {% for row in rows %}
  <tr>
    {% for cell in row %}
    <td style="
      background-color:{% if loop.parent.loop.index is odd %}{{stripe_odd | default('#FFFFFF')}}{% else %}{{stripe_even | default('#D6E4F0')}}{% endif %};
      padding:6px 12px;
      border:1px solid #D9D9D9;
    ">{{cell.value}}</td>
    {% endfor %}
  </tr>
  {% endfor %}
</table>
{% endblock %}
```

### 6.4 `table_excel.html.j2`

Excel-native template that generates HTML matching Excel 15/16's own clipboard format. Does NOT extend `_base.html.j2` — uses its own full HTML structure with Office XML namespaces. Reverse-engineered by copying formatted cells from Excel and reading the clipboard with `clipli read --type html`.

**Key differences from `table_default`:**
- Office XML namespaces (`xmlns:v`, `xmlns:o`, `xmlns:x`) and `<meta name=ProgId content=Excel.Sheet>`
- `<style>` block wrapped in `<!-- ... -->` with CSS classes using `mso-*` properties (not inline-only styles)
- `background:` (not `background-color:`) with `mso-pattern:black none` for fills
- Per-position border classes: `hdr_l/m/r` (header left/middle/right), `cl/cm/cr` (cell), `tl/tm/tr` (total row)
- Thick outer border (`1.0pt solid windowtext`), thin inner borders (`.5pt solid windowtext`)
- `<!--StartFragment-->` / `<!--EndFragment-->` markers around table rows
- `mso-number-format` for currency/percent/date formatting
- Explicit `height` on every `<tr>` (note: `<col>` column widths are ignored on paste — confirmed by testing)
- `&nbsp;` for empty cells (not empty strings)
- Every class explicitly sets `color:black` and `vertical-align:middle`

**Style object keys:**
| Key | Default | Description |
|-----|---------|-------------|
| `style.header_bg` | `#4472C4` | Header row background |
| `style.header_fg` | `#FFFFFF` | Header row text color |
| `style.total_bg` | `#F2F2F2` | Total (last) row background |
| `style.default_font` | `Calibri` | Font family for all cells |
| `style.default_font_size` | `11` | Font size in pt |

**Per-cell style fields:**
| Field | Values | Description |
|-------|--------|-------------|
| `alignment` | `left`, `center`, `right` | Text alignment (emits both `align=` attr and `text-align:` CSS) |
| `bold` | bool | Bold text (`font-weight:700`) |
| `fg_color` | hex string | Text color |
| `bg_color` | hex string | Background on any cell (`background:` + `mso-pattern:black none`). Last row with bg_color gets thick bottom border. |
| `number_format` | see below | Excel number format (`mso-number-format`) |
| `url` | string | Hyperlink URL — renders `<a href>` with styled span preserving cell colors |
| `wrap` | bool | Word wrapping (`white-space:normal`). Default: nowrap. |

**Number format values:**
| Value | Output | Example |
|-------|--------|---------|
| `currency` | `$#,##0` with red negatives | `$4,230,000` |
| `percent` | `Percent` (fractional input) | `15.60%` |
| `percent_int` | `0%` | `98%` |
| `percent_1dp` | `0.0%` | `15.6%` |
| `integer` | `#,##0` | `12,819` |
| `standard` | `Standard` | `1234.5678` |
| `text` | `@` (force text) | `B0BFBRL47B` |

**Usage:**
```bash
echo '{"headers":[...],"rows":[...],"style":{"header_bg":"#007873"}}' | clipli paste --from-table -t table_excel
```

**Merged title rows (colspan):**
The template does not support merged cells. For title rows or other layouts requiring `colspan`, use `clipli write --type html` with hand-crafted Excel-native HTML. Key details:
- Use `<td colspan=N>` with `border-right:1.0pt solid black` as inline style override
- Use `mso-number-format:"mmmm\\ yyyy"` for date-formatted title cells
- The header row below needs inline `border-top:none` to avoid doubled borders

**Font charset values:**
| Font | `mso-font-charset` |
|------|---------------------|
| Calibri | `0` (ANSI_CHARSET) |
| Aptos Display, Aptos Narrow | `1` (DEFAULT_CHARSET) |

Excel 365 default base font is "Aptos Narrow" in the base `td` style. Individual classes override with the applied font.

**Important implementation notes:**
1. `Option::None` fields must use `#[serde(skip_serializing_if = "Option::is_none")]` — otherwise serde serializes them as JSON `null`, and minijinja's `| default()` filter does not fire on `null` (it only fires on undefined).
2. Template-level config (`default_font`, `default_font_size`) must be inside the `style` object, not top-level JSON keys — `TableInput` deserialization drops unknown top-level keys.
3. Font names with spaces must be quoted in CSS: `font-family:"Aptos Display", sans-serif`.
4. Alignment requires BOTH `align=` HTML attribute AND `text-align:` in inline `style=` to override the class-level `text-align:general`.
5. Data cell classes define `border-top:none` in the class itself — only the header row uses inline `border-top:none` (when a title row sits above).
6. Empty cells must contain `&nbsp;` — empty strings cause sizing issues.

See `templates/table_excel.html.md` for the full sidecar reference document with detailed examples and the complete border position grid.

---

## 7. Configuration (`config.toml`)

```toml
# ~/.config/clipli/config.toml

[defaults]
font = "Calibri"
font_size_pt = 11
plain_text_strategy = "tab-delimited"   # auto | tab-delimited | none

[clean]
keep_classes = false
target_app = "generic"    # excel | powerpoint | google_sheets | generic

[templatize]
default_strategy = "heuristic"    # heuristic | agent | manual

[agent]
# For --strategy agent: how to invoke the external agent
# clipli pipes JSON to this command's stdin and reads JSON from stdout.
command = "claude-code"
args = ["--prompt-file", "/dev/stdin"]
# Alternatively, for direct API calls (future):
# endpoint = "http://localhost:8080/templatize"

[editor]
command = "$EDITOR"    # for `clipli edit`
```

---

## 8. Dependencies

```toml
[package]
name = "clipli"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# CLI
clap = { version = "4", features = ["derive", "env"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# macOS pasteboard FFI
objc2 = "0.5"
objc2-foundation = { version = "0.2", features = ["NSString", "NSData", "NSArray"] }
objc2-app-kit = { version = "0.2", features = ["NSPasteboard", "NSWorkspace"] }

# HTML processing
lol_html = "2"

# Template engine (Jinja2-compatible)
minijinja = { version = "2", features = ["loader", "builtins"] }

# Utilities
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
dirs = "5"               # XDG / macOS config dirs
toml = "0.8"             # config file parsing
regex = "1"              # heuristic templatizer patterns

[dev-dependencies]
assert_cmd = "2"         # CLI integration tests
predicates = "3"
tempfile = "3"
insta = "1"              # snapshot testing for HTML output
```

**Platform:** macOS only (aarch64-apple-darwin, x86_64-apple-darwin). The `objc2-app-kit` dependency is macOS-specific. Cross-platform pasteboard support (Linux via xclip/wl-copy, Windows via clipboard API) is a future extension but out of scope for v1.

---

## 9. Error Handling Strategy

All modules use `thiserror` for typed errors. The CLI catches all errors in `main.rs` and produces either:

- **Human-readable** (default): colored stderr output with actionable suggestions
- **JSON** (when `--json` is set on any subcommand): `{"error": "...", "code": "..."}` on stdout

Error codes:

| Code | Module | Description |
|------|--------|-------------|
| `PB_EMPTY` | pb | Pasteboard has no content |
| `PB_TYPE_NOT_FOUND` | pb | Requested type not on pasteboard |
| `PB_WRITE_FAILED` | pb | Failed to write to pasteboard |
| `CLEAN_INVALID_HTML` | clean | HTML couldn't be parsed even with repair |
| `RENDER_TEMPLATE_NOT_FOUND` | render | Named template doesn't exist |
| `RENDER_MISSING_VARIABLE` | render | Required variable not provided and no default |
| `RENDER_SYNTAX_ERROR` | render | Jinja2 syntax error in template |
| `STORE_NOT_FOUND` | store | Template name not in store |
| `STORE_ALREADY_EXISTS` | store | Template exists (use --force) |
| `STORE_IO_ERROR` | store | Filesystem error |
| `TEMPLATIZE_AGENT_TIMEOUT` | templatize | Agent didn't respond |
| `TEMPLATIZE_INVALID_RESPONSE` | templatize | Agent response didn't parse |
| `CONFIG_PARSE_ERROR` | config | Invalid config.toml |

---

## 10. Testing Strategy

### 10.1 Unit Tests

Each module has co-located tests using captured HTML fixtures from real applications:

- **`clean_tests.rs`** — Snapshot tests (insta) comparing raw Office HTML → cleaned output. Fixtures captured from Excel 2024, PowerPoint 2024, Google Sheets, Numbers.
- **`render_tests.rs`** — Round-trip tests: render template with known data, verify HTML output.
- **`templatize_tests.rs`** — Verify heuristic extraction on sample HTML. Assert correct variable names, types, and that re-rendering with default values produces original content.
- **`store_tests.rs`** — CRUD operations on a tempdir store.

### 10.2 Integration Tests

- **Full round-trip:** Capture fixture HTML → clean → templatize → save → load → render with new data → verify HTML is valid and formatted.
- **CLI smoke tests:** `assert_cmd` tests for each subcommand with expected exit codes and output patterns.
- **Pasteboard tests:** Marked `#[ignore]` by default (require macOS GUI session). Run manually with `cargo test -- --ignored`.

### 10.3 Fixtures

```
tests/fixtures/
├── excel_table_basic.html          # Simple table from Excel
├── excel_table_formatted.html      # Table with colors, fonts, borders
├── ppt_two_slides.html             # Two slides from PowerPoint
├── ppt_chart_slide.html            # Slide with chart (degrades to image)
├── sheets_pivot.html               # Google Sheets pivot table
├── numbers_table.html              # Apple Numbers table
└── word_formatted_text.html        # Rich text from Word
```

---

## 11. Agent Integration Patterns

### 11.1 Direct Pipeline

```bash
# Agent captures, fills, and pastes in one pipeline:
clipli capture --name temp_report --templatize --strategy heuristic --json | \
  jq '.variables' | \
  my-agent fill --template temp_report | \
  clipli paste temp_report --stdin
```

### 11.2 Agent-Assisted Templatization

```bash
# Capture raw, send to agent for smart templatization:
clipli capture --name slide_deck --strategy agent 2>&1 | \
  claude-code --stdin | \
  clipli capture --name slide_deck --force --stdin-template
```

### 11.3 Programmatic Use (as a Library)

While clipli is CLI-first, the modules are structured for optional library extraction:

```rust
// Future: clipli-core crate
use clipli_core::{pb, clean, render, store};

let snapshot = pb::read_all()?;
let html = snapshot.get_html()?;
let cleaned = clean::clean(html, &CleanOptions::default())?;
let rendered = render::Renderer::new(store_path)?.render("my_template", &data)?;
pb::write(&[(PbType::Html, rendered.html.as_bytes()), (PbType::PlainText, rendered.plain.as_bytes())])?;
```

---

## 12. Future Extensions (Out of Scope for v1)

| Extension | Description |
|-----------|-------------|
| **Cross-platform pasteboard** | Linux (xclip/wl-copy/xsel), Windows (clipboard API) |
| **Image template capture** | Capture `public.png`/`public.tiff`, use as template backgrounds |
| **RTF round-trip** | Some apps prefer RTF over HTML; add RTF rendering |
| **Clipboard watch mode** | `clipli watch` — daemon that auto-captures every clipboard change to a log |
| **MCP server** | Expose clipli as an MCP tool server for direct agent integration |
| **Template sharing** | Import/export templates as `.clipli` bundles (zip of template dir) |
| **clipli-core crate** | Extract library crate for programmatic use without CLI overhead |
| **Embedded preview server** | `clipli serve` — local HTTP server to preview templates in browser with hot reload |

---

## 13. Development Roadmap

### Phase 1: Foundation (MVP)
- [ ] `pb.rs` — read/write pasteboard (HTML + plain text)
- [ ] `model.rs` — all data types
- [ ] `clipli inspect`
- [ ] `clipli read`
- [ ] `clipli write`
- [ ] Test fixtures from Excel and PowerPoint

### Phase 2: Capture & Clean
- [ ] `clean.rs` — full 10-stage pipeline
- [ ] `store.rs` — filesystem template store
- [ ] `clipli capture` (with `--raw` and `--strategy manual`)
- [ ] `clipli list`, `clipli show`, `clipli delete`
- [ ] Snapshot tests for cleaner

### Phase 3: Templates & Rendering
- [ ] `render.rs` — minijinja integration + custom filters
- [ ] Built-in templates (table_default, table_striped, slide_default)
- [ ] `clipli paste` (named template + data)
- [ ] `clipli paste --from-table`
- [ ] `clipli edit`
- [ ] Config file support

### Phase 4: Templatization
- [ ] `templatize.rs` — heuristic strategy
- [ ] `clipli capture --templatize --strategy heuristic`
- [ ] Agent strategy protocol (stdout/stdin JSON)
- [ ] `clipli capture --templatize --strategy agent`
- [ ] `clipli convert`

### Phase 5: Polish
- [ ] Colored CLI output (human-friendly errors)
- [ ] Shell completions (clap_complete)
- [ ] `clipli --version`, `clipli help` polish
- [ ] README.md, man page
- [ ] CI (GitHub Actions, macOS runner)
- [ ] Homebrew formula