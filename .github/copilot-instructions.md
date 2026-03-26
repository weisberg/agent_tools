# Copilot Instructions

This repository contains agent tooling — primarily the `clipli` Rust CLI and Python utilities under `tools/`.

## Repository Layout

- `tools/clipli/` — Main Rust CLI project (clipboard intelligence)
- `tools/` — Python utilities: `md_search.py`, `img_manipulate.py`, `md_clean.py`
- `skills/` — Claude skill definitions

---

## clipli (Rust CLI)

### Build, Test, Lint

```bash
# From tools/clipli/
cargo build               # debug build
cargo build --release     # release build
cargo test                # run all tests
cargo test --test clean_tests          # single test file
cargo test test_name_here              # single test by name
cargo test --test clean_tests -- --nocapture  # with stdout
cargo clippy              # lint
cargo fmt                 # format
```

Tests use `assert_cmd` (spawns the binary as subprocess) plus `insta` for HTML snapshot tests. Never touch `~/.config` — store tests always use `tempfile` temp directories.

### Architecture

**Purpose**: Transforms the macOS clipboard into a template-driven interface. Core loop: copy from app → `clipli capture` → templatize → agent/user fills variables → `clipli paste` → paste back with original formatting preserved.

**Module dependency graph** (no circular deps; `model` is a leaf):
```
main.rs  ──► pb.rs        (NSPasteboard FFI via objc2)
         ──► clean.rs     (10-stage HTML sanitizer pipeline)
         ──► render.rs    (minijinja Jinja2 renderer + custom filters)
         ──► templatize.rs (variable extraction: heuristic / agent / manual)
         ──► store.rs     (template CRUD at ~/.config/clipli/templates/)
         ──► excel.rs     (Excel-native HTML generator from CSV)
         ──► model.rs     (shared types: PbType, TemplateMeta, TableInput, etc.)
```

**Template store layout** (`~/.config/clipli/templates/NAME/`):
```
template.html.j2   (templatized) or template.html (raw)
meta.json          (TemplateMeta)
schema.json        (Vec<TemplateVariable>, optional)
original.html      (cleaned HTML before templatization)
raw.html           (uncleaned clipboard HTML)
```

Built-in template files live in `templates/*.html.j2` and are embedded via `include_str!`.

### Key Conventions

**Error handling**: Every module defines its own `Error` enum with `thiserror`. Variants include a `.code()` method returning a `&'static str` (e.g., `"CLEAN_REWRITER"`) used in structured JSON errors.

```rust
#[derive(Debug, thiserror::Error)]
pub enum FooError {
    #[error("descriptive message: {0}")]
    Variant(String),
}
impl FooError {
    pub fn code(&self) -> &'static str { match self { Self::Variant(_) => "FOO_VARIANT" } }
}
```

**JSON output**: All commands support `--json`. Success returns structured data; errors use `{"error": "...", "code": "..."}` via `print_json_error()` in `main.rs`.

**Config**: Two-tier — `~/.config/clipli/config.toml` (TOML, optional) overridden by CLI flags. Config structs use `#[serde(default)]` so missing fields fall back to defaults.

**Command handlers**: Functions in `main.rs` are named `cmd_*` (e.g., `cmd_capture`, `cmd_paste`).

**Module public API**: Each module exposes a minimal surface. Cross-module data exchange uses types from `model.rs`. Options are passed as `&Config` refs (immutable).

**Code organization within a module**:
1. Header comment linking to spec section
2. Public types (errors, config structs)
3. Private helpers
4. Public functions
5. `#[cfg(test)] mod tests { }` at the bottom

**HTML cleaning pipeline** (`clean.rs`): 10 sequential stages — encoding normalization, strip meta/link/style, strip conditional comments, normalize inline CSS (mso-\* denylist), normalize fonts, collapse empty elements, normalize colors, strip class/id attributes, collapse whitespace, validate output. Per-target CSS allowlists differ between Excel, PowerPoint, and Google Sheets.

**Templatization strategies**: `heuristic` (7 regex passes: dates, currency, %, emails, large numbers, quarters, fields), `agent` (JSON protocol over stdin/stdout), `manual` (no extraction). Strategy is an enum in `model.rs`.

**macOS FFI**: All `unsafe` objc2 calls are isolated in `pb.rs`. Pin exact objc2 versions — the crate is pre-1.0 with unstable APIs.

**Naming**: error types are `*Error`; config structs are `Config*`; enum variants are CamelCase; module section headers use `// ── Section Name ──`.

---

## Python Tools (tools/)

Run via `uv run` from the repo root. All return structured JSON with `--json` or when called non-interactively.

```bash
uv run tools/md_search.py headers <file> [--level N] [--json]
uv run tools/md_search.py links <file> [--external-only] [--json]
uv run tools/md_search.py code-blocks <file> [--language python] [--json]

uv run tools/img_manipulate.py resize <file> [--width N] [--height N] [--scale F] [--json]
uv run tools/img_manipulate.py crop <file> [--x N] [--y N] [--width N] [--height N] [--json]
uv run tools/img_manipulate.py convert <file> --format webp [--json]
uv run tools/img_manipulate.py batch-convert <dir> --format webp [--json]

uv run tools/md_clean.py clean <file> [--in-place] [--out-file out.md] [--json]
```

Error envelope on failure: `{"ok": false, "error": {"code": "...", "message": "...", "suggestion": "..."}}`.

All tools support `--dry-run` on commands that write files.

---

## Reference Docs

- `tools/clipli/CLIPLI_SPEC.md` — Full spec (commands, data model, error codes, config format)
- `tools/clipli/CLIPLI_PLAN.md` — Phased development roadmap (5 phases)
- `TOOLI_DEV_GUIDE.md` — Framework guide for building new Python tools
