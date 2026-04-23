---
title: "docli — DOCX CLI for Agile Agentic Analytics"
version: 0.1.0-draft
status: proposed
author: Brian
agents: [athena-analyst, experiment-analyst, scrum-master]
language: rust
tags: [cli, docx, agent-native, sdd, knowledge-base, rust]
created: 2026-03-15
---

# docli — DOCX CLI Specification

> A single-binary, agent-native CLI that wraps the full complexity of OOXML
> document manipulation behind structured commands with JSON output.
> Built in Rust for sub-millisecond cold starts that make every invocation
> truly atomic. Designed for Claude Code sub-agents in the Agile Agentic
> Analytics ecosystem.

---

## 1. Problem Statement

The current `docx` skill requires agents to:

1. **Context-switch between Python and Node.js** — creation uses `docx-js` (npm),
   editing uses Python scripts, conversion uses LibreOffice. Neither is fast to start.
2. **Manage intermediate state** — the unpack → edit XML → pack lifecycle leaks
   implementation details into the agent's working memory.
3. **Understand OOXML internals** — agents must produce valid WordprocessingML XML,
   know about `<w:r>`, `<w:pPr>`, DXA units, RSIDs, `ShadingType.CLEAR` vs `SOLID`,
   and dozens of other schema-level concerns.
4. **Parse unstructured text output** — scripts return human-readable messages,
   not machine-parseable structures.
5. **Carry a ~600-line skill document in context** — the SKILL.md is large because
   it must teach XML patterns that the agent applies by hand.

This burns context tokens on plumbing instead of the user's actual document task.

### Design Goal

Replace the entire skill surface with a **single static binary** that:

- Accepts high-level, declarative intent (YAML specs, named operations)
- Returns structured JSON an agent can parse without regex
- Manages all intermediate state internally (no leaked temp dirs)
- Integrates with the knowledge base for templates, styles, and rules
- Starts in sub-millisecond time, making every edit invocation fully atomic
- Ships as a zero-dependency binary — no Python, no Node.js, no runtime
- Follows the agent-native CLI patterns already in the ecosystem (`jira-cli`, `confluence-cli`, `agentcli`)

---

## 2. Architecture

`docli` is a **Rust CLI plus Rust library** for inspecting, creating, patching,
reviewing, validating, rendering, and finalizing Word `.docx` documents for
agent workflows. The CLI is the primary interface for agents. The library is
the primary implementation surface for engineering. A future MCP wrapper is
allowed but should be thin — it translates requests into the same internal
job AST that the CLI uses.

### Core Insight: The Transaction Boundary is the Package

A `.docx` file is a ZIP archive of XML parts. The atomic unit is not
`word/document.xml` — it is the finished package on disk. The current skill
already points in this direction with its unpack/edit/repack workflow. Rust's
fast startup makes it cheap enough to treat **every edit as a full package
transaction** without ceremony.

```
┌──────────────────────────────────────────────┐
│  Agent (Claude Code / Sub-agent)             │
│  e.g. athena-analyst, experiment-analyst     │
└────────────────┬─────────────────────────────┘
                 │  stdin: YAML jobs / JSON ops
                 │  stdout: structured JSON
                 │  stderr: progress / diagnostics
┌────────────────▼─────────────────────────────┐
│              docli CLI (clap)                 │
│                                              │
│  Verbs                                       │
│  ┌──────────────────────────────────────────┐│
│  │ inspect   read   create   run            ││
│  │ edit.*    review.*    finalize.*          ││
│  │ validate  diff   convert  extract        ││
│  │ template  ooxml.*   kb.*   doctor        ││
│  └──────────────────────────────────────────┘│
│                    │                          │
│         ┌─────────▼──────────┐               │
│         │   Job AST          │  All verbs    │
│         │   (unified IR)     │  compile here │
│         └─────────┬──────────┘               │
│                   │                          │
│  Workspace Crates │                          │
│  ┌────────────────▼─────────────────────────┐│
│  │ docli-core     │ Package model, job AST, ││
│  │                │ commit journal, hashing, ││
│  │                │ transaction pipeline     ││
│  ├────────────────┼─────────────────────────┤│
│  │ docli-query    │ roxmltree read-only     ││
│  │                │ index, selector engine,  ││
│  │                │ part hashing, story enum ││
│  ├────────────────┼─────────────────────────┤│
│  │ docli-patch    │ Custom OOXML patcher:   ││
│  │                │ run-split, tracked edits,││
│  │                │ comment builders, ID     ││
│  │                │ allocator, rel updates   ││
│  ├────────────────┼─────────────────────────┤│
│  │ docli-create   │ Pluggable backend for   ││
│  │                │ new docs (docx-rs v1)   ││
│  ├────────────────┼─────────────────────────┤│
│  │ docli-schema   │ Typed OOXML parts,      ││
│  │                │ structural validators   ││
│  ├────────────────┼─────────────────────────┤│
│  │ docli-render   │ Adapters: Pandoc (MD),  ││
│  │                │ soffice (PDF), Poppler  ││
│  ├────────────────┼─────────────────────────┤│
│  │ docli-kb       │ Template/rule resolver, ││
│  │                │ minijinja rendering     ││
│  └────────────────┴─────────────────────────┘│
└──────────────────────────────────────────────┘
         │                    │
    ┌────▼────┐         ┌────▼────────────┐
    │ KB /    │         │ Optional Deps   │
    │templates│         │ pandoc, soffice │
    └─────────┘         └─────────────────┘
```

### Key Architectural Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | Sub-ms cold start makes every invocation a real package transaction; single static binary; zero runtime deps |
| **Edit engine** | **Custom OOXML patcher** (`docli-patch`) | High-level DOCX crates lose fidelity on round-trip. Edits to existing docs must work at the part-graph level: run-splitting, tracked change builders, relationship updates. This is the heart of the system. |
| Create engine | `docx-rs` behind a `CreateBackend` trait | Good enough for greenfield generation; hidden behind a trait so the backend is swappable if the crate has sharp edges |
| Read-only XML | `roxmltree` | Fast, zero-alloc read-only DOM for structural indexing and selector resolution |
| Streaming XML | `quick-xml` | Low-allocation streaming read/write for transforms on large parts |
| Package I/O | `zip` crate | DOCX archive read/write; stream unchanged entries through without re-processing |
| Durable writes | `atomicfile` or equivalent | Same-filesystem temp → fsync → rename → dir-sync. Not just "rename and hope." |
| CLI framework | `clap` (derive) | Industry standard; auto-generated help and shell completions |
| Serialization | `serde` + `serde_json` + `serde_yaml` | Typed envelopes, typed ops, typed specs |
| Template rendering | `minijinja` | Jinja2-compatible, compiles into the binary |
| Semantic extraction | Pandoc (adapter) | Pandoc's docx reader handles `--track-changes=all` and `reference.docx` well; its intermediate model is less expressive than OOXML so it stays as an adapter, not the edit engine |
| Render QA | LibreOffice + Poppler (adapters) | DOCX→PDF→page-image for visual diff; same approach as the current skill |
| Typed OOXML parts | `ooxmlsdk` (selective) | Generated schemas for docx/xlsx/pptx parts; validators are WIP so it's a helper, not a complete story |

### Atomic Commit Model

This is where Rust earns its lunch. The earlier draft said "atomic" without
defining what that means for a ZIP-based file format. There are three
distinct properties:

**Semantic atomicity:** All requested edits in one command succeed together
or fail together. If operation 4 of 6 fails validation, operations 1–3 are
not applied either.

**Visibility atomicity:** The destination file is swapped in one step. No
reader ever sees a half-written document. This requires writing a complete
shadow package to the same filesystem, then performing an atomic rename.

**Crash durability:** After success returns, the new file survives power
loss as well as the platform allows. This is **not** free — `rename(2)` alone
does not guarantee the file contents have been flushed to disk. Stronger
guarantees require fsync-before-rename plus directory-sync, which has
measurable cost.

`docli` exposes this as an explicit `--durability` flag rather than pretending
all atomic writes are equal:

| Mode | Behavior | Use Case |
|------|----------|----------|
| `fast` | Temp file → write → rename. No fsync. | Ephemeral agent sandboxes, CI pipelines |
| `durable` (default) | Temp file → write → fsync temp → rename → fsync parent dir | Production workflows, shared filesystems |
| `paranoid` | Build temp package → reopen → revalidate → optionally render-test → durable commit | Compliance-critical documents, audit trails |

**Write policy (all mutating commands):**

1. Never modify the source archive in place.
2. Always write a complete shadow package.
3. Place the temp output on the same filesystem as the destination.
4. Replace the target only after validation succeeds.
5. Leave the source untouched on failure.

### Shadow-Package Write Pipeline

Every mutating command follows this pipeline:

```
 1. Open source archive (read-only)
 2. Build part inventory and content hashes
 3. Build selector index (roxmltree)
 4. Apply all ops to in-memory part graph (docli-patch)
 5. Validate structure + hard invariants + KB rules
 6. Serialize only the touched XML parts
 7. Stream unchanged entries (images, media, fonts) through without re-processing
 8. Write full shadow .docx to same-filesystem temp path
 9. Reopen and validate the shadow package (paranoid mode: always; durable mode: optional)
10. Optionally render/diff (paranoid mode)
11. Commit with selected durability mode
12. Emit JSON envelope + commit journal to stdout
```

Step 7 is a key optimization — a typical edit touches `document.xml` and
maybe `comments.xml`, but the document may contain megabytes of embedded
images. Streaming those through unchanged avoids unnecessary I/O.

### Hard Invariants vs KB Rules

The alternative spec correctly identifies that the current docx skill teaches
critical XML truths as "tribal memory in markdown notes" — things like
`w:id` collision between bookmarks/comments/revisions, illegal nesting of
tracked changes inside runs, missing required package parts that trigger
Word repair dialogs. These should be **compile-time or validation-time
guarantees**, not documentation.

**Hard invariants (enforced by `docli-patch` and `docli-schema`):**

These are baked into the binary. Violating them is a bug, not a configuration choice.

- **Unified `w:id` allocator.** One shared allocator for all OOXML ID spaces that can collide (bookmarks, comments, revisions, footnotes, endnotes). No duplicate IDs, ever.
- **Structural validity of tracked changes.** Insertions and deletions emitted only at valid structural levels — never nested inside `<w:r>` or `<w:t>`.
- **Comment range sibling enforcement.** `<w:commentRangeStart>` and `<w:commentRangeEnd>` are always siblings of `<w:r>`, never children.
- **Relationship + Content-Type consistency.** Adding an image, comment, or part always updates `_rels/*.rels` and `[Content_Types].xml` atomically.
- **Required package parts.** `endnotes.xml`, `webSettings.xml`, and other parts Word expects are present when needed. Missing them triggers repair dialogs.
- **Table structural integrity.** `w:tblLook` present, border attributes complete (`w:space="0"`), cell widths sum correctly.
- **`xml:space="preserve"` on whitespace `<w:t>`.** Auto-added during serialization.
- **`durableId` / `paraId` range validity.** Values < `0x7FFFFFFF`, auto-repaired if violated.
- **Package reopen validation.** After every write, reopen and verify the ZIP is well-formed.
- **Deterministic package normalization.** Part ordering, XML formatting, and ZIP entry ordering are canonical so that byte-identical inputs produce byte-identical outputs.

**KB rules (loaded from knowledge base, configurable per organization):**

These are taste, branding, and convention. They belong outside the binary.

- Default page size by region (Letter vs A4)
- Heading style hierarchy and numbering
- Real numbering instead of Unicode bullets
- Analytics report table heuristics (striped rows, DXA widths, dual-width tables)
- Branding (fonts, colors, logos, header/footer templates)
- Typography (smart quotes, em-dashes)
- Accessibility (alt text on images, heading structure, reading order)
- House style and review checklists

### Crate Strategy

There is a tempting trap: pick one Rust DOCX crate and declare victory.
The right split is:

| Role | Crate | Notes |
|------|-------|-------|
| Primary edit engine | Custom (`docli-patch`) | Part-level OOXML patching; this is the heart of docli |
| Primary query engine | `roxmltree` + `quick-xml` | `roxmltree` for read-only indexing; `quick-xml` for streaming transforms |
| Primary package I/O | `zip` | Archive read/write with streaming passthrough |
| Primary commit layer | `atomicfile` | Durable rename with fsync semantics |
| Primary create backend | `docx-rs` (behind `CreateBackend` trait) | Swappable if the crate has gaps |
| Experimental round-trip | `linch-docx-rs` (feature-flagged) | Promising python-docx-like API with round-trip emphasis, but docs say "not yet production"; comments/tracked changes still on roadmap |
| Typed OOXML helper | `ooxmlsdk` (selective) | Generated schemas for typed parts; validators still WIP |

```toml
[workspace]
members = [
    "docli-cli",       # The binary: clap, subcommand dispatch, envelope formatting
    "docli-core",      # Package model, job AST, commit pipeline, hashing
    "docli-query",     # Read-only index, selector resolution, part hashing
    "docli-patch",     # OOXML patching: run-split, tracked changes, comments, IDs
    "docli-create",    # CreateBackend trait + docx-rs implementation
    "docli-schema",    # Typed parts, structural validators, hard invariants
    "docli-render",    # Pandoc adapter, soffice adapter, visual diff
    "docli-kb",        # KB resolver, minijinja rendering, template filling
]

[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# XML
roxmltree = "0.20"           # Fast read-only DOM for indexing
quick-xml = { version = "0.36", features = ["serialize"] }  # Streaming read/write

# ZIP (DOCX is a ZIP archive)
zip = "2"

# OOXML document generation (create-only backend)
docx-rs = "0.4"

# Typed OOXML schemas (selective use)
ooxmlsdk = "0.2"

# Atomic file writes with proper fsync
atomicfile = "0.1"

# Template rendering (Jinja2-compatible)
minijinja = { version = "2", features = ["loader"] }

# Temp files with RAII cleanup
tempfile = "3"

# Diff algorithm
similar = "2"

# Error handling
anyhow = "1"
thiserror = "2"
```

### Workspace Layout

```
docli/
├── Cargo.toml                         # Workspace root
│
├── docli-cli/                         # The binary
│   └── src/
│       ├── main.rs                    # clap dispatch → job AST → pipeline
│       ├── envelope.rs                # JSON envelope types, timing
│       └── commands/                  # Thin orchestrators per subcommand
│           ├── inspect.rs
│           ├── read.rs
│           ├── create.rs
│           ├── edit.rs                # edit.replace, edit.insert, etc.
│           ├── review.rs              # review.comment, review.track-replace
│           ├── finalize.rs            # finalize.accept, finalize.reject
│           ├── run.rs                 # Batch job execution
│           ├── validate.rs
│           ├── diff.rs
│           ├── convert.rs
│           ├── template.rs
│           ├── ooxml.rs               # ooxml.unpack, ooxml.pack, ooxml.query
│           ├── kb.rs                  # kb.list, kb.show, kb.resolve
│           └── doctor.rs              # Environment health check
│
├── docli-core/                        # Package model + transaction engine
│   └── src/
│       ├── package.rs                 # Part inventory, content hashing
│       ├── job.rs                     # Job AST: the unified IR all verbs compile to
│       ├── pipeline.rs                # Shadow-package write pipeline (12 steps)
│       ├── commit.rs                  # Durability modes: fast / durable / paranoid
│       ├── journal.rs                 # Commit journal for audit trails
│       └── units.rs                   # "1in" → 1440 DXA, "12pt" → 240 half-points
│
├── docli-query/                       # Read-only structural indexing
│   └── src/
│       ├── index.rs                   # Paragraph, table, image, comment indices
│       ├── selector.rs                # Selector enum + resolution
│       ├── heading.rs                 # heading_path + offset
│       ├── story.rs                   # body, header.default, footer.first, etc.
│       └── hash.rs                    # Part-level content hashing for diff
│
├── docli-patch/                       # The heart: OOXML part-level patching
│   └── src/
│       ├── run_split.rs               # Split runs at text offsets
│       ├── tracked_changes.rs         # <w:ins>, <w:del> builders
│       ├── comments.rs                # Comment range + reply builders
│       ├── id_alloc.rs                # Unified w:id allocator (no collisions)
│       ├── relationships.rs           # _rels + Content_Types updates
│       ├── tables.rs                  # Cell update, row append
│       ├── images.rs                  # media/ + relationship + content type
│       ├── runs.rs                    # Run merging, formatting preservation
│       ├── numbering.rs               # Bullet/number list config
│       └── normalize.rs               # Package canonicalization
│
├── docli-create/                      # Greenfield document generation
│   └── src/
│       ├── backend.rs                 # CreateBackend trait
│       ├── docx_rs.rs                 # docx-rs implementation
│       └── spec.rs                    # YAML spec → CreateJob
│
├── docli-schema/                      # Validation + hard invariants
│   └── src/
│       ├── invariants.rs              # Hard invariants (compiled in)
│       ├── structural.rs              # XSD-like structural checks
│       ├── redline.rs                 # Tracked change integrity
│       └── repair.rs                  # Auto-fix (durableId, xml:space, etc.)
│
├── docli-render/                      # External adapter layer
│   └── src/
│       ├── pandoc.rs                  # Semantic extraction, reference.docx
│       ├── soffice.rs                 # DOCX→PDF conversion
│       ├── poppler.rs                 # PDF→page images
│       └── markdown.rs                # Built-in OOXML→Markdown walker
│
└── docli-kb/                          # Knowledge base integration
    └── src/
        ├── resolver.rs                # kb:// URI → filesystem path
        ├── template.rs                # minijinja rendering + var interpolation
        └── rules.rs                   # KB rule loading + validation
```

Each `docli-cli/commands/*.rs` is a thin orchestrator: parse args → compile
to job AST → execute pipeline → format envelope → stdout. The heavy lifting
lives in the workspace crates.

---

## 3. Global Flags

```
--format json|yaml|text    Output format (default: json)
--pretty                   Pretty-print JSON output (default: compact)
--quiet                    Suppress stderr diagnostics
--kb-path <dir>            Knowledge base root (default: ./kb)
--author <name>            Author for tracked changes / comments (default: "Claude")
--verbose                  Verbose stderr diagnostics
--durability <mode>        Commit durability: fast | durable (default) | paranoid
```

Every mutating subcommand also accepts:

```
--in <file>                Source DOCX (never modified)
--out <file>               Destination DOCX (atomically created)
```

`--in` and `--out` may point to the same path — the shadow-package pipeline
guarantees the source is read completely before the destination is written.

### JSON Envelope

All commands return a consistent envelope:

```json
{
  "ok": true,
  "command": "inspect",
  "data": { ... },
  "warnings": [],
  "elapsed_ms": 142
}
```

On failure:

```json
{
  "ok": false,
  "command": "edit",
  "error": {
    "code": "INVALID_TARGET",
    "message": "Paragraph index 47 out of range (document has 32 paragraphs)",
    "context": { "max_index": 31 }
  },
  "warnings": [],
  "elapsed_ms": 38
}
```

---

## 4. Revised Command Surface

The earlier draft used batch-only subcommands (`docli edit ops.yaml`). The
alternative spec correctly identifies that Rust's startup cost makes **narrow
verbs** practical — agents can do `inspect → one edit → inspect` loops as
separate atomic commits without performance penalty. Internally, all verbs
compile into the same **job AST** that `docli run` consumes. One execution
engine, one validator, one commit pipeline.

```bash
# ── Read-only ──
docli inspect   <file> [--sections ...] [--depth shallow|full]
docli read      <file> [--range ...] [--section ...]
docli validate  <file> [--original <file>]

# ── Batch (job file or stdin) ──
docli run       <job.yaml|job.json|-> [--in <file>] [--out <file>]
docli create    <spec.yaml>           [--out <file>] [--template <name>]

# ── Narrow edit verbs (micro-commit mode) ──
docli edit replace       --in <file> --out <file> --target <sel> --content <text>
docli edit insert        --in <file> --out <file> --target <sel> --position before|after --content <text|yaml>
docli edit delete        --in <file> --out <file> --target <sel>
docli edit find-replace  --in <file> --out <file> --find <text> --replace <text> [--scope all|first]
docli edit update-table  --in <file> --out <file> --target <sel> --cell <row,col> --content <text>
docli edit set-style     --in <file> --out <file> --target <sel> --style <yaml>

# ── Review verbs ──
docli review comment         --in <file> --out <file> --target <sel> --text <text>
docli review track-replace   --in <file> --out <file> --target <sel> --content <text>
docli review track-insert    --in <file> --out <file> --target <sel> --content <text>
docli review track-delete    --in <file> --out <file> --target <sel>
docli review list            <file>

# ── Finalize verbs ──
docli finalize accept   --in <file> --out <file> [--ids <n,n,...>]
docli finalize reject   --in <file> --out <file> [--ids <n,n,...>]
docli finalize strip    --in <file> --out <file>

# ── Diff / convert / extract ──
docli diff      <left.docx> <right.docx> [--mode semantic|render|hybrid]
docli convert   <file> --to <format> [--out <file>]
docli extract   <tables|images|section|comments> <file> [--out <dir>]

# ── Templates + KB ──
docli template  list|show|apply [<name>] [--vars <k=v,...>]
docli kb        list|show|resolve|validate [<uri>]

# ── OOXML escape hatch (expert mode) ──
docli ooxml unpack  <file> <dir>
docli ooxml pack    <dir> <file> [--original <file>]
docli ooxml query   <file> <xpath>
docli ooxml patch   <file> <patch.yaml> [--out <file>]

# ── Diagnostics ──
docli schema    <job|patch|report>     # Print JSON schema for job/patch/report types
docli doctor                           # Environment health check (deps, KB, permissions)
```

This gives agents two modes:

- **Micro-commit mode:** Narrow verbs for single edits as separate atomic
  package transactions. Cheap because startup is sub-millisecond.
- **Transaction mode:** `docli run` with a job file for many related edits
  committed together as one package write.

Both compile to the same job AST internally.

---

## 5. Subcommand Detail

### 5.1 `docli inspect`

> "What am I working with?"

Returns a structural map of a DOCX file — the agent's first call before any edits.

```bash
docli inspect report.docx
docli inspect report.docx --sections headings,tables,images
docli inspect report.docx --depth shallow    # just counts, no content
```

#### Output Schema

```json
{
  "ok": true,
  "command": "inspect",
  "data": {
    "file": "report.docx",
    "file_size_bytes": 48230,
    "page_size": "letter",
    "orientation": "portrait",
    "sections": 3,
    "paragraphs": {
      "count": 47,
      "by_style": {
        "Heading1": [0, 12, 28],
        "Heading2": [1, 5, 13, 19, 29, 35],
        "Normal": [2, 3, 4, 6, 7, ...],
        "ListParagraph": [8, 9, 10, 11]
      }
    },
    "headings": [
      { "index": 0,  "level": 1, "text": "Executive Summary" },
      { "index": 1,  "level": 2, "text": "Key Findings" },
      { "index": 5,  "level": 2, "text": "Methodology" },
      { "index": 12, "level": 1, "text": "Experiment Design" },
      { "index": 13, "level": 2, "text": "CUPED Variance Reduction" }
    ],
    "tables": [
      { "index": 0, "location_paragraph": 4, "rows": 5, "cols": 3,
        "header_row": ["Metric", "Control", "Treatment"] }
    ],
    "images": [
      { "index": 0, "location_paragraph": 7, "type": "png",
        "width_inches": 6.5, "height_inches": 4.0, "alt_text": "Lift chart" }
    ],
    "comments": {
      "count": 3,
      "authors": ["Jane", "Claude"],
      "unresolved": 2
    },
    "tracked_changes": {
      "count": 8,
      "insertions": 5,
      "deletions": 3,
      "authors": ["Claude"]
    },
    "styles": ["Heading1", "Heading2", "Normal", "ListParagraph", "Caption"],
    "fonts": ["Arial", "Calibri"],
    "word_count": 2847
  }
}
```

The `paragraphs.by_style` mapping and `headings` array become the addressing
system for all subsequent commands. Agents anchor edits to these indices.

---

### 5.2 `docli read`

> "Give me the content."

Extracts document text as markdown, plain text, or a structured paragraph array.

```bash
docli read report.docx                         # full markdown
docli read report.docx --format json           # structured paragraphs
docli read report.docx --range 0:12            # paragraphs 0 through 12
docli read report.docx --section "Methodology" # heading + its content
docli read report.docx --tables-only           # extract tables as JSON
docli read report.docx --comments              # include comment annotations
docli read report.docx --tracked-changes all   # show insertions/deletions
```

#### Structured Output (JSON)

```json
{
  "ok": true,
  "command": "read",
  "data": {
    "paragraphs": [
      {
        "index": 0,
        "style": "Heading1",
        "text": "Executive Summary",
        "runs": [
          { "text": "Executive Summary", "bold": true, "font": "Arial", "size_pt": 16 }
        ]
      },
      {
        "index": 1,
        "style": "Normal",
        "text": "This report presents the results of the Q4 email holdout experiment...",
        "runs": [
          { "text": "This report presents the results of the Q4 email holdout experiment...",
            "font": "Arial", "size_pt": 12 }
        ]
      }
    ],
    "tables": [
      {
        "index": 0,
        "after_paragraph": 4,
        "headers": ["Metric", "Control", "Treatment"],
        "rows": [
          ["Open Rate", "12.3%", "15.7%"],
          ["Click Rate", "3.1%", "4.8%"]
        ]
      }
    ]
  }
}
```

---

### 5.3 `docli create`

> "Build me a document from this spec."

Creates a new DOCX from a declarative YAML specification. This is the
spec-driven development (SDD) equivalent for documents.

```bash
docli create spec.yaml -o report.docx
docli create spec.yaml -o report.docx --template vanguard-brand
cat spec.yaml | docli create --stdin -o report.docx
```

#### Spec Format

```yaml
# docli create spec
# Supports $ref for knowledge-base template composition

meta:
  page_size: letter          # letter | a4
  orientation: portrait      # portrait | landscape
  margins:                   # all values accept: Nin (inches), Nmm, Ncm, or raw DXA int
    top: 1in
    right: 1in
    bottom: 1in
    left: 1in

styles:
  $ref: kb://styles/vanguard-brand.yaml    # pull from knowledge base
  # — or inline: —
  # heading1:
  #   font: Arial
  #   size: 16pt
  #   bold: true
  #   color: "#2E75B6"
  #   spacing_before: 240
  #   spacing_after: 240
  # body:
  #   font: Arial
  #   size: 12pt

numbering:
  bullets:
    levels:
      - { format: bullet, text: "•", indent: 0.5in }
      - { format: bullet, text: "–", indent: 1.0in }
  numbers:
    levels:
      - { format: decimal, text: "%1.", indent: 0.5in }

header:
  default:
    - paragraph:
        text: "Vanguard Marketing Analytics"
        align: left
        font: { name: Arial, size: 9pt, color: "#666666" }
    - rule: { color: "#2E75B6", weight: 1pt }

footer:
  default:
    - columns:
        left: "Confidential"
        right: "Page {PAGE} of {NUMPAGES}"
      font: { name: Arial, size: 9pt, color: "#999999" }

content:
  # — Heading —
  - heading1: "Executive Summary"

  # — Body paragraph —
  - paragraph: "This report documents the Q4 2025 email holdout experiment."

  # — Paragraph with inline formatting —
  - paragraph:
      runs:
        - text: "The treatment showed a "
        - text: "statistically significant"
          bold: true
        - text: " lift of 15.7%."

  # — Bullet list —
  - bullets:
      - "Control group: 50,000 users (no email)"
      - "Treatment group: 50,000 users (weekly digest)"
      - "Duration: 90 days"

  # — Numbered list —
  - numbers:
      - "Define holdout population"
      - "Randomize assignment"
      - "Measure incremental lift"

  # — Table —
  - table:
      headers: ["Metric", "Control", "Treatment", "Lift", "p-value"]
      rows:
        - ["AUM Growth", "$1.2M", "$1.8M", "+$600K", "0.003"]
        - ["Account Opens", "142", "198", "+39.4%", "0.011"]
        - ["Engagement Score", "6.2", "7.8", "+25.8%", "0.001"]
      style: striped             # striped | bordered | minimal | none
      column_widths: [2in, 1.5in, 1.5in, 1in, 1in]

  # — Image —
  - image:
      path: ./charts/lift_chart.png
      width: 6in
      alt: "Bar chart showing incremental AUM lift by segment"
      caption: "Figure 1: Incremental AUM lift across wealth segments"

  # — Page break —
  - page_break: true

  # — Section reference from knowledge base —
  - $ref: kb://sections/cuped-methodology.yaml

  # — Table of contents (placed where declared) —
  - toc:
      heading_range: 1-3
      title: "Table of Contents"

  # — Footnote-bearing paragraph —
  - paragraph:
      runs:
        - text: "Revenue grew 15%"
        - footnote: "Source: Q4 2025 Annual Report, adjusted for seasonality"
        - text: " using CUPED-adjusted metrics."

  # — Multi-column section —
  - columns:
      count: 2
      gap: 0.5in
      content:
        - paragraph: "Left column content flows naturally..."
        - paragraph: "...into the right column."

  # — Hyperlink —
  - paragraph:
      runs:
        - text: "See the "
        - link:
            text: "full methodology"
            url: "https://internal.vanguard.com/methodology"
        - text: " for details."
```

#### Template Composition with `$ref`

The `$ref` key resolves paths from the knowledge base:

```
kb://styles/vanguard-brand.yaml   → {kb-path}/templates/docli/styles/vanguard-brand.yaml
kb://sections/cuped-methodology.yaml → {kb-path}/templates/docli/sections/cuped-methodology.yaml
```

This allows teams to maintain shared style guides, boilerplate sections,
and reusable content blocks that any agent can compose into documents.

#### Output

```json
{
  "ok": true,
  "command": "create",
  "data": {
    "output_file": "report.docx",
    "file_size_bytes": 52480,
    "paragraphs_created": 47,
    "tables_created": 1,
    "images_embedded": 1,
    "refs_resolved": 2,
    "validation": "passed"
  }
}
```

---

### 5.4 `docli edit`

> "Change this document."

Applies a batch of edit operations to an existing DOCX. The agent describes
*what* to change using the addressing system from `inspect`; docli handles
all XML manipulation internally.

```bash
docli edit report.docx ops.yaml -o report-v2.docx
docli edit report.docx ops.yaml --in-place
docli edit report.docx --op 'replace:p3:Updated text here'  # inline single op
cat ops.yaml | docli edit report.docx --stdin -o report-v2.docx
```

#### Operations Format

```yaml
# docli edit operations file
# All operations are applied atomically — if any fails, no changes are written.

operations:

  # ── Replace paragraph text ──
  - op: replace_text
    target: { paragraph: 3 }                # by index from inspect
    content: "Updated paragraph content with new findings."

  # ── Replace by heading selector ──
  - op: replace_text
    target: { heading: "Key Findings", offset: 1 }  # 1st paragraph after heading
    content: "The experiment yielded a statistically significant lift."

  # ── Find-and-replace (all occurrences) ──
  - op: find_replace
    find: "30 days"
    replace: "60 days"
    scope: all                               # all | first | section:"Methodology"

  # ── Find-and-replace with tracked changes ──
  - op: find_replace
    find: "preliminary"
    replace: "final"
    track_changes: true

  # ── Insert paragraph after ──
  - op: insert_after
    target: { paragraph: 12 }
    content:
      - paragraph: "New paragraph inserted after index 12."
      - paragraph:
          runs:
            - text: "With "
            - text: "formatting"
              bold: true
            - text: " preserved."

  # ── Insert before ──
  - op: insert_before
    target: { heading: "Methodology" }
    content:
      - heading2: "Results Overview"
      - paragraph: "Before diving into methodology, here are the key results."

  # ── Delete paragraphs ──
  - op: delete
    target: { paragraphs: [5, 6, 7] }       # multiple indices
    track_changes: true                      # mark as deletion, don't remove

  # ── Delete by range ──
  - op: delete
    target: { range: "14:18" }               # paragraphs 14 through 18
    track_changes: false                     # actually remove from document

  # ── Update table cell ──
  - op: update_table
    target: { table: 0 }                     # table index from inspect
    cell: { row: 2, col: 3 }                # 0-indexed
    content: "+42.1%"

  # ── Append row to table ──
  - op: append_table_row
    target: { table: 0 }
    row: ["Net Flows", "$890K", "$1.4M", "+57.3%", "0.008"]

  # ── Replace image ──
  - op: replace_image
    target: { image: 0 }
    path: ./charts/updated_lift_chart.png
    width: 6in

  # ── Update style on paragraphs ──
  - op: set_style
    target: { range: "3:5" }
    style:
      font: Arial
      size: 11pt
      bold: false
      color: "#333333"

  # ── Apply heading level ──
  - op: set_heading
    target: { paragraph: 14 }
    level: 2                                 # Heading2

  # ── Page break insertion ──
  - op: insert_page_break
    target: { before_paragraph: 28 }

  # ── Section from knowledge base ──
  - op: insert_after
    target: { heading: "Experiment Design" }
    content:
      - $ref: kb://sections/cuped-methodology.yaml
```

#### Selector Model

Structured selectors are the public contract. General XPath is **not** the
main interface — it lives behind `docli ooxml query` for experts.

The `target` field in ops files and `--target` on narrow verbs accepts these
selector types:

| Selector | Example | Description |
|----------|---------|-------------|
| `paragraph: N` | `paragraph: 3` | Direct paragraph index (from `inspect`) |
| `paragraphs: [N,M,...]` | `paragraphs: [5,6,7]` | Multiple paragraph indices |
| `range: "N:M"` | `range: "14:18"` | Inclusive paragraph range |
| `heading: "text"` | `heading: "Methodology"` | First heading matching text (substring) |
| `heading_path: "H1/H2"` | `heading_path: "Results/CUPED"` | Hierarchical heading path |
| `heading: "text", offset: N` | `heading: "Key Findings", offset: 2` | Nth paragraph after heading |
| `table: N` | `table: 0` | Table by index |
| `image: N` | `image: 0` | Image by index |
| `style: "name"` | `style: "Caption"` | All paragraphs with named style |
| `text: "substring"` | `text: "30 days"` | First paragraph containing text |
| `text: "pattern", regex: true` | `text: "\\d+ days"` | Regex match with occurrence control |
| `bookmark: "name"` | `bookmark: "tbl_results"` | Named bookmark |
| `node_id: "id"` | `node_id: "0A1B2C3D"` | Stable internal `w14:paraId` (survives reflows) |
| `contains: "text", occurrence: N` | `contains: "revenue", occurrence: 2` | Nth occurrence of substring |

All selectors support an optional **story scope** to restrict resolution to
a specific document story:

```yaml
target:
  heading: "Methodology"
  story: body                    # default
# Other story values: header.default, header.first, header.even,
#                     footer.default, footer.first, footer.even,
#                     footnotes, endnotes, comments
```

Selectors resolve to a `ResolvedTarget` that carries the paragraph index(es),
the containing XML part path, and a span of XML byte offsets for precise
run-splitting. This resolution is deterministic and cached within a session.

#### Output

```json
{
  "ok": true,
  "command": "edit",
  "data": {
    "input_file": "report.docx",
    "output_file": "report-v2.docx",
    "operations_applied": 6,
    "operations_total": 6,
    "tracked_changes_added": 2,
    "validation": "passed",
    "durability": "durable",
    "commit": {
      "source_hash": "sha256:a1b2c3...",
      "output_hash": "sha256:d4e5f6...",
      "parts_modified": ["word/document.xml", "word/comments.xml"],
      "parts_unchanged": 14
    },
    "ops_detail": [
      { "index": 0, "op": "replace_text", "target": "p:3", "status": "applied" },
      { "index": 1, "op": "find_replace", "target": "all", "matches": 4, "status": "applied" },
      { "index": 2, "op": "insert_after", "target": "p:12", "paragraphs_added": 2, "status": "applied" },
      { "index": 3, "op": "delete", "target": "p:5-7", "status": "applied" },
      { "index": 4, "op": "update_table", "target": "t:0[2,3]", "status": "applied" },
      { "index": 5, "op": "replace_image", "target": "img:0", "status": "applied" }
    ]
  }
}
```

---

### 5.5 `docli review`

> "Add feedback, make tracked changes."

The `review` namespace groups all operations that annotate or propose changes
to a document without silently modifying content. Every `review` verb
produces proper OOXML markup (comment ranges, `<w:ins>`, `<w:del>`) with
author attribution, timestamps, and collision-free IDs from the unified allocator.

```bash
# ── Comments ──
docli review list report.docx                                        # list all comments + tracked changes
docli review comment --in report.docx --out out.docx --target 'heading:Methodology' --text "Needs CUPED citation"
docli review comment --in report.docx --out out.docx --target 'p:14' --text "Check sample size"
docli review comment --in report.docx --out out.docx --parent 0 --text "Added citation in v2"  # reply
docli review resolve --in report.docx --out out.docx --id 0

# ── Tracked changes (same as edit verbs but with track_changes: true) ──
docli review track-replace --in report.docx --out out.docx --target 'text:30 days' --content "60 days"
docli review track-insert  --in report.docx --out out.docx --target 'p:12' --position after --content "New paragraph."
docli review track-delete  --in report.docx --out out.docx --target 'range:5:7'
```

Each `review` verb is a micro-commit — one atomic package transaction per call.
For batch review operations, use `docli run` with a job file.

#### `review list` Output

```json
{
  "ok": true,
  "command": "review.list",
  "data": {
    "comments": [
      {
        "id": 0,
        "author": "Jane",
        "date": "2025-12-01T14:30:00Z",
        "text": "Please verify the sample size calculation.",
        "target_paragraph": 14,
        "target_text_snippet": "...using a sample of 50,000 users...",
        "resolved": false,
        "replies": [
          {
            "id": 1,
            "author": "Claude",
            "date": "2025-12-02T09:00:00Z",
            "text": "Sample size validated — power analysis confirms 80% power at α=0.05."
          }
        ]
      }
    ],
    "tracked_changes": [
      {
        "id": 2,
        "type": "insertion",
        "author": "Claude",
        "date": "2026-03-15T10:00:00Z",
        "paragraph": 3,
        "text": "statistically significant",
        "context": "The result was ___statistically significant___ at the 95% level."
      },
      {
        "id": 3,
        "type": "deletion",
        "author": "Claude",
        "date": "2026-03-15T10:00:00Z",
        "paragraph": 7,
        "text": "preliminary",
        "context": "This ~~preliminary~~ analysis shows..."
      }
    ],
    "summary": {
      "comments": 1,
      "tracked_changes": { "total": 8, "insertions": 5, "deletions": 3 },
      "authors": { "Jane": 1, "Claude": 9 }
    }
  }
}
```

---

### 5.6 `docli finalize`

> "Accept, reject, or strip review markup."

The `finalize` namespace resolves tracked changes and comments, producing
a clean document. Each verb is an atomic package transaction.

```bash
# Accept all tracked changes (produce clean document)
docli finalize accept --in report.docx --out report-clean.docx

# Reject all tracked changes (restore original text)
docli finalize reject --in report.docx --out report-original.docx

# Accept/reject specific changes by ID
docli finalize accept --in report.docx --out out.docx --ids 1,2,3
docli finalize reject --in report.docx --out out.docx --ids 4,5

# Strip all tracked changes and comments (produce a "final" document)
docli finalize strip --in report.docx --out report-final.docx
```

---

### 5.7 `docli validate`

> "Is this document well-formed?"

Runs schema validation and produces a structured report.

```bash
docli validate report.docx
docli validate report.docx --original input.docx   # compare against original
docli validate report.docx --repair                 # auto-fix what's fixable
```

#### Output

```json
{
  "ok": true,
  "command": "validate",
  "data": {
    "valid": true,
    "repairs": 2,
    "repairs_detail": [
      { "type": "durable_id_overflow", "file": "commentsIds.xml", "action": "regenerated" },
      { "type": "missing_xml_space", "file": "document.xml", "element": "w:t", "action": "added" }
    ],
    "warnings": [
      { "type": "mixed_fonts", "message": "Document uses 3 font families (Arial, Calibri, Times New Roman)" }
    ],
    "schema_errors": []
  }
}
```

---

### 5.8 `docli convert`

> "Transform this file."

Format conversion between document types. Most conversions are pure Rust;
PDF and legacy `.doc` require an optional `soffice` (LibreOffice) subprocess.

```bash
# Native (pure Rust, no external deps)
docli convert report.docx --to md -o report.md       # built-in OOXML→MD walker
docli convert report.docx --to html -o report.html   # built-in OOXML→HTML walker
docli convert report.docx --to json -o report.json   # structured paragraph array

# Requires soffice (optional external dependency)
docli convert report.docx --to pdf -o report.pdf
docli convert report.docx --to images -o pages/       # one image per page (via PDF)
docli convert legacy.doc --to docx -o modern.docx     # .doc → .docx
```

If `soffice` is not available, the command returns a structured error
(`DEPENDENCY_MISSING`) rather than silently failing.

---

### 5.9 `docli template`

> "What templates are available? Use one."

Lists and applies templates from the knowledge base.

```bash
docli template list                            # list available templates
docli template show experiment-brief           # show template spec
docli template apply experiment-brief -o brief.docx  # create from template
docli template apply experiment-brief -o brief.docx \
  --vars 'title=Q4 Email Holdout,author=Brian,date=2026-03-15'
```

Templates are YAML specs (same format as `docli create`) stored in the
knowledge base with front matter metadata:

```yaml
---
name: experiment-brief
description: "Standard A/B test experiment brief for Vanguard marketing analytics"
category: analytics
vars:
  title: { type: string, required: true }
  author: { type: string, default: "Marketing Analytics" }
  date: { type: date, default: today }
  hypothesis: { type: string, required: true }
  primary_metric: { type: string, required: true }
tags: [experiment, ab-test, brief]
---

meta:
  page_size: letter
  margins: { top: 1in, right: 1in, bottom: 1in, left: 1in }

styles:
  $ref: kb://styles/vanguard-brand.yaml

content:
  - heading1: "Experiment Brief: {{ title }}"
  - table:
      style: minimal
      rows:
        - ["Author", "{{ author }}"]
        - ["Date", "{{ date }}"]
        - ["Status", "Proposed"]
  - heading2: "Hypothesis"
  - paragraph: "{{ hypothesis }}"
  - heading2: "Primary Metric"
  - paragraph: "{{ primary_metric }}"
  - heading2: "Design"
  - $ref: kb://sections/standard-experiment-design.yaml
  - heading2: "Statistical Methodology"
  - $ref: kb://sections/cuped-methodology.yaml
```

---

### 5.10 `docli diff`

> "What's different between these two documents?"

Structural and content diff between two DOCX files. Supports multiple diff
strategies because semantic similarity and visual similarity are different things.

```bash
docli diff original.docx revised.docx                       # default: semantic
docli diff original.docx revised.docx --mode semantic        # XML part-level content diff
docli diff original.docx revised.docx --mode render          # visual pixel diff via soffice+poppler
docli diff original.docx revised.docx --mode hybrid          # both semantic + render
docli diff original.docx revised.docx --summary              # high-level counts only
```

#### Output

```json
{
  "ok": true,
  "command": "diff",
  "data": {
    "summary": {
      "paragraphs_added": 3,
      "paragraphs_deleted": 1,
      "paragraphs_modified": 5,
      "tables_modified": 1,
      "images_added": 1
    },
    "changes": [
      {
        "type": "modified",
        "paragraph": 3,
        "original": "The term is 30 days.",
        "revised": "The term is 60 days.",
        "diff": "The term is [-30-]{+60+} days."
      },
      {
        "type": "added",
        "paragraph": 15,
        "content": "New paragraph about CUPED methodology."
      }
    ]
  }
}
```

---

### 5.11 `docli merge`

> "Combine these documents."

Merges multiple DOCX files into one.

```bash
docli merge part1.docx part2.docx part3.docx -o combined.docx
docli merge part1.docx part2.docx --separator page_break -o combined.docx
docli merge part1.docx part2.docx --separator heading:"Part 2" -o combined.docx
```

---

### 5.12 `docli extract`

> "Pull specific content out of this document."

Targeted extraction of tables, images, or sections.

```bash
docli extract tables report.docx -o tables/          # all tables as CSV/JSON
docli extract images report.docx -o images/           # all embedded images
docli extract section report.docx "Methodology" -o methodology.md
docli extract comments report.docx -o comments.json
```

---

### 5.13 `docli run`

> "Execute a batch job."

The batch interface. All narrow verbs (`edit.*`, `review.*`, `finalize.*`)
compile into the same **job AST** that `docli run` consumes. A job file
is a YAML or JSON document describing multiple operations committed as one
package transaction.

```bash
docli run job.yaml --in report.docx --out report-v2.docx
docli run job.json --in report.docx --out report-v2.docx
cat job.yaml | docli run - --in report.docx --out report-v2.docx
```

```yaml
# job.yaml — multiple ops, one atomic commit
operations:
  - op: edit.replace
    target: { paragraph: 3 }
    content: "Updated paragraph."

  - op: review.track-replace
    target: { text: "30 days" }
    content: "60 days"

  - op: review.comment
    target: { heading: "Methodology" }
    text: "Added CUPED citation per Jane's review."

  - op: edit.insert
    target: { heading: "Results" }
    position: after
    content:
      - $ref: kb://sections/cuped-methodology.yaml
```

All operations apply to the same in-memory part graph and commit together.
If any operation fails validation, zero changes are written.

---

### 5.14 `docli ooxml`

> "I know what I'm doing. Let me at the XML."

The escape hatch for experts. Direct access to the OOXML package internals
when structured selectors aren't enough. Agents should prefer the high-level
verbs; `ooxml` is for edge cases and debugging.

```bash
# Unpack to a directory (XML pretty-printed, runs merged)
docli ooxml unpack report.docx unpacked/

# Repack from a directory (validates, canonicalizes, atomic commit)
docli ooxml pack unpacked/ report-v2.docx --original report.docx

# Query XML parts with XPath
docli ooxml query report.docx '//w:p[w:pPr/w:pStyle[@w:val="Heading1"]]'

# Apply a raw XML patch (for when structured ops don't cover your case)
docli ooxml patch report.docx patch.yaml --out report-v2.docx
```

The `ooxml pack` command runs through the same shadow-package pipeline as
all other mutating commands — validation, hard invariant checks, durable commit.

---

### 5.15 `docli kb`

> "What's in the knowledge base?"

CRUD for the knowledge base. Browse, resolve, and validate templates, styles,
rules, and content blocks.

```bash
docli kb list                              # list all KB entries
docli kb list --category styles            # filter by category
docli kb show vanguard-brand               # print a template/style
docli kb resolve 'kb://sections/cuped-methodology.yaml'  # resolve URI to content
docli kb validate                          # check all KB entries for schema errors
```

---

### 5.16 `docli schema`

> "What's the shape of a job file?"

Prints JSON schemas for docli's typed inputs and outputs. Useful for agents
that want to validate their own job files before submitting them.

```bash
docli schema job         # JSON schema for job.yaml format
docli schema patch       # JSON schema for ooxml patch format
docli schema report      # JSON schema for the output envelope
docli schema spec        # JSON schema for create spec format
```

---

### 5.17 `docli doctor`

> "Is my environment healthy?"

Environment health check. Reports on binary version, available external
dependencies, KB path validity, filesystem permissions, and platform info.

```bash
docli doctor
docli doctor --json
```

```json
{
  "ok": true,
  "command": "doctor",
  "data": {
    "version": "0.1.0",
    "target": "aarch64-apple-darwin",
    "kb_path": "./kb",
    "kb_valid": true,
    "kb_entries": 14,
    "adapters": {
      "soffice": { "available": true, "version": "25.2.1", "path": "/usr/bin/soffice" },
      "pandoc": { "available": true, "version": "3.6", "path": "/usr/bin/pandoc" },
      "poppler": { "available": false, "message": "pdftoppm not found in PATH" }
    },
    "filesystem": {
      "output_writable": true,
      "temp_same_filesystem": true
    }
  }
}
```

---

## 6. Knowledge Base Structure

Templates, styles, rules, and examples live in the knowledge base under a
`templates/docli/` namespace. The librarian agent indexes these via YAML
front matter. The KB cleanly separates **engine invariants** (baked into
the binary) from **organization taste** (loaded at runtime).

Template packages can include both **semantic templates** (YAML specs,
markdown snippets) and **OOXML-native assets** (`.dotx` reference documents,
style fragments, section fragments, canonical table/header/footer parts).

```
{kb-path}/templates/docli/
├── README.md                          # Index / overview
│
├── styles/
│   ├── vanguard-brand.yaml            # Corporate style guide
│   ├── minimal.yaml                   # Clean minimal styles
│   └── academic.yaml                  # Paper / research styles
│
├── specs/
│   ├── experiment-brief.yaml          # A/B test documentation
│   ├── campaign-report.yaml           # Campaign measurement report
│   ├── executive-summary.yaml         # One-page exec summary
│   ├── quarterly-review.yaml          # QBR deck companion doc
│   └── methodology-appendix.yaml      # Statistical methodology doc
│
├── sections/                          # Reusable content blocks ($ref targets)
│   ├── cuped-methodology.yaml         # CUPED explanation boilerplate
│   ├── standard-experiment-design.yaml
│   ├── statistical-significance.yaml
│   ├── data-sources.yaml
│   └── disclaimer-footer.yaml
│
├── assets/                            # OOXML-native artifacts
│   ├── vanguard-reference.dotx        # Reference doc (styles + page props + headers/footers)
│   ├── header-logo.png                # Brand assets for embedding
│   ├── table-styles/                  # Canonical table part fragments
│   │   ├── analytics-striped.xml
│   │   └── executive-bordered.xml
│   └── section-fragments/             # Reusable OOXML section XML
│       ├── two-column-layout.xml
│       └── landscape-appendix.xml
│
├── rules/
│   ├── formatting.md                  # Rules agents must follow when editing
│   ├── accessibility.md               # WCAG compliance rules for docs
│   ├── brand-compliance.md            # Vanguard brand guidelines
│   └── review-checklist.md            # Pre-submission quality checks
│
└── examples/
    ├── cuped-report-spec.yaml         # Complete example spec
    ├── cuped-report-job.yaml          # Complete example job file
    ├── holdout-experiment-spec.yaml
    └── campaign-results-spec.yaml
```

### Style File Format

```yaml
---
name: vanguard-brand
description: "Vanguard Marketing Analytics brand styles"
version: 1.0
---

default:
  font: Arial
  size: 12pt
  color: "#333333"
  line_spacing: 1.15

heading1:
  font: Arial
  size: 16pt
  bold: true
  color: "#1A1A2E"
  spacing_before: 18pt
  spacing_after: 12pt

heading2:
  font: Arial
  size: 14pt
  bold: true
  color: "#2E75B6"
  spacing_before: 12pt
  spacing_after: 8pt

heading3:
  font: Arial
  size: 12pt
  bold: true
  color: "#2E75B6"
  spacing_before: 8pt
  spacing_after: 4pt

table:
  header_background: "#2E75B6"
  header_font_color: "#FFFFFF"
  stripe_color: "#F2F7FB"
  border_color: "#CCCCCC"
  cell_padding: { top: 4pt, bottom: 4pt, left: 6pt, right: 6pt }

caption:
  font: Arial
  size: 10pt
  italic: true
  color: "#666666"
```

---

## 7. Agent Integration Patterns

### Pattern 1: Micro-Commit Loop

Because Rust's cold start is sub-millisecond, agents can treat each narrow
verb as a separate atomic package transaction. Inspect → edit → inspect →
edit, each a real file commit. No batching required for simple workflows.

```bash
# 1. Inspect the document
docli inspect report.docx --pretty

# 2. Make a single targeted edit (one atomic package write)
docli edit replace --in report.docx --out report.docx \
  --target 'heading:Methodology,offset:1' \
  --content "Updated methodology with CUPED variance reduction details."

# 3. Add a review comment (another atomic package write)
docli review comment --in report.docx --out report.docx \
  --target 'heading:Results' \
  --text "Verified lift calculations against Athena query."

# 4. Verify the result
docli inspect report.docx --sections comments,changes
```

Each command reads the source completely, writes a shadow package, validates,
and atomically swaps. `--in` and `--out` can safely be the same file.

### Pattern 2: Batch Transaction

For many related edits that should commit together, use `docli run` with a
job file. All operations apply to the same in-memory part graph and commit
as one package write.

```bash
docli run job.yaml --in report.docx --out report-v2.docx
```

### Pattern 3: Template-Based Generation

For standard deliverables, skip spec authoring entirely.

```bash
docli template apply experiment-brief --out brief.docx \
  --vars 'title=Q4 Email Holdout,hypothesis=Weekly digest increases AUM,primary_metric=Incremental AUM'
```

### Pattern 4: Sub-Agent Delegation

In the Agile Agentic Analytics framework, the `scrum-master` agent can
delegate document tasks to specialized sub-agents:

```
scrum-master → experiment-analyst:
  "Generate the Q4 holdout report using docli template 'campaign-report'
   with the data from the Athena query results."

experiment-analyst:
  1. docli template apply campaign-report --out q4-report.docx --vars ...
  2. docli run edits.yaml --in q4-report.docx --out q4-report.docx
  3. docli validate q4-report.docx
  4. Return: q4-report.docx
```

---

## 8. Implementation Roadmap

Sequenced by dependency chain, not feature-request priority. Each phase
delivers a usable CLI increment.

### Phase 1: Package Core

The foundation everything else builds on. Ship `inspect`, `validate`,
`ooxml` escape hatch, durable commit pipeline, KB resolution, and `doctor`.

| Crate | Scope |
|-------|-------|
| `docli-core` | Package model, part inventory, content hashing, job AST, shadow-package pipeline, durability modes, commit journal |
| `docli-query` | `roxmltree` read-only index, selector resolution, story enum |
| `docli-schema` | Hard invariants, structural validators, auto-repair |
| `docli-kb` | `kb://` URI resolution, template listing, KB validation |
| `docli-cli` | `inspect`, `validate`, `ooxml unpack/pack/query`, `kb`, `schema`, `doctor` |

**Agent unblock:** Agents can inspect documents, validate them, and browse the KB.

### Phase 2: Structured Edits

The heart: `docli-patch` and the narrow edit verbs. This is where the
custom OOXML patcher earns its keep.

| Crate | Scope |
|-------|-------|
| `docli-patch` | Run-splitting, text-range mapping, insert/delete/replace, table cell updates, image replacement, relationship + content-type updates, unified `w:id` allocator, package canonicalization |
| `docli-cli` | `edit.*` narrow verbs, `run` batch executor |

**Agent unblock:** Agents can modify existing documents with micro-commits or batch jobs.

### Phase 3: Review Flows

Tracked changes and comments, built on top of the patch engine.

| Crate | Scope |
|-------|-------|
| `docli-patch` | Tracked insert/delete/replace builders, comment range builders, `<w:ins>`/`<w:del>` generation with proper structural nesting |
| `docli-cli` | `review.*` verbs (comment, track-replace, track-insert, track-delete, list), `finalize.*` verbs (accept, reject, strip) |

**Agent unblock:** Full review workflow — propose changes with tracked changes,
add comments, accept/reject.

### Phase 4: Create Flows

Greenfield document generation via pluggable backend.

| Crate | Scope |
|-------|-------|
| `docli-create` | `CreateBackend` trait, `docx-rs` implementation, YAML spec parser, reference-docx support, KB snippet/asset embedding |
| `docli-cli` | `create`, `template apply` with variable interpolation |

**Agent unblock:** Agents can generate new documents from YAML specs and templates.

### Phase 5: Render QA + Diff

External adapters for semantic extraction and visual quality assurance.

| Crate | Scope |
|-------|-------|
| `docli-render` | Pandoc adapter (semantic extraction, `--track-changes=all`, `reference.docx` flows), soffice adapter (DOCX→PDF), Poppler adapter (PDF→page images), built-in OOXML→Markdown walker |
| `docli-cli` | `diff` (semantic/render/hybrid modes), `convert`, `extract`, `read` (enhanced with Pandoc adapter) |

**Agent unblock:** Agents can diff documents, convert formats, and validate
visual output against expectations.

### Phase 6: Experimental Backends

Evaluate newer Rust DOCX crates as the ecosystem matures.

| Candidate | Interest | Maturity |
|-----------|----------|----------|
| `linch-docx-rs` | python-docx-like API with round-trip emphasis | Docs say "not yet production"; comments/tracked changes on roadmap |
| Deeper `ooxmlsdk` integration | Generated schemas for typed validation | Validators WIP, no serde support yet |

These belong behind feature flags or in a lab branch, not at the center of
the pipeline on day one.

---

## 9. Skill Replacement Strategy

Once `docli` reaches Phase 2 maturity, the SKILL.md knowledge requirement
shrinks dramatically:

### Current Skill (SKILL.md): ~600 lines

Teaches agents XML patterns, DXA units, JavaScript APIs, multi-step workflows,
common pitfalls, schema compliance rules.

### Replacement Skill: ~60 lines

```markdown
# DOCX Documents

Use `docli` for all DOCX operations. Every command returns structured JSON.

## Workflow
1. `docli inspect <file>` — understand document structure
2. `docli read <file>` — extract content
3. `docli create <spec.yaml>` — create from YAML spec
4. `docli edit replace|insert|delete --in <f> --out <f>` — modify documents
5. `docli review comment|track-replace --in <f> --out <f>` — add review markup
6. `docli finalize accept|reject|strip --in <f> --out <f>` — resolve reviews
7. `docli run <job.yaml>` — batch multiple ops as one atomic commit
8. `docli template list` — see available templates

## Quick Reference
| Task | Command |
|------|---------|
| Inspect structure | `docli inspect report.docx` |
| Read as markdown | `docli read report.docx --format text` |
| Create from spec | `docli create spec.yaml --out report.docx` |
| Single edit | `docli edit replace --in r.docx --out r2.docx --target 'p:3' --content "New text"` |
| Batch edit | `docli run job.yaml --in report.docx --out report-v2.docx` |
| Add comment | `docli review comment --in r.docx --out r2.docx --target 'p:14' --text "Check this"` |
| Track changes | `docli review track-replace --in r.docx --out r2.docx --target 'text:30 days' --content "60 days"` |
| Accept changes | `docli finalize accept --in r.docx --out clean.docx` |
| Validate | `docli validate report.docx` |
| Convert to PDF | `docli convert report.docx --to pdf` |
| Diff documents | `docli diff original.docx revised.docx` |
| Environment check | `docli doctor` |

All mutating commands use `--in`/`--out` and never modify the source file.
`--durability durable` (default) ensures crash-safe writes.

See `docli --help` and `docli <command> --help` for full options.
Templates and styles: `{kb-path}/templates/docli/`
```

**Context savings: ~540 lines (~10x reduction)**. The complexity doesn't
disappear — it moves from the agent's context window into the CLI binary
where it belongs.

---

## 10. Design Principles Summary

1. **The transaction boundary is the package.** A `.docx` is a ZIP archive. The atomic unit is the finished file on disk, not an individual XML part. Every mutating command produces a complete, validated package or produces nothing.
2. **Structured over unstructured.** JSON envelopes with typed error codes, not printed messages to grep.
3. **Declarative over imperative.** YAML specs and job files, not API calls and XML surgery.
4. **Addressed over positional.** Paragraph indices, heading selectors, bookmark names, and story scopes — not raw XPath as the primary contract.
5. **Atomic is three things, not one.** Semantic atomicity (all-or-nothing ops), visibility atomicity (rename-based swap), and crash durability (fsync) are separate properties with separate costs. Name them explicitly.
6. **Custom patcher for edits, pluggable backend for creation.** High-level DOCX crates lose fidelity on round-trip. Edits work at the part-graph level. Creation goes through a swappable `CreateBackend` trait.
7. **Hard invariants in the binary, taste in the KB.** `w:id` collision prevention, structural nesting rules, and required package parts are baked in. Branding, typography, and house style are loaded from the knowledge base.
8. **Composable over monolithic.** `$ref` templates, piped commands, narrow verbs for micro-commits, `docli run` for batch transactions, sub-agent delegation.
9. **Progressive disclosure.** `docli edit replace` for simple changes, `docli run job.yaml` for complex batches, `docli ooxml query` for experts who need raw XML access.
10. **Context-efficient.** The 60-line skill replaces a 600-line one. Complexity lives in the binary.
11. **Zero-dependency.** Single static binary. No interpreters, no package managers, no runtimes. External adapters (pandoc, soffice) are optional and degrade gracefully.
12. **The MCP wrapper should be gloriously boring.** If a future MCP server wraps docli, it should translate requests into the same job AST that the CLI uses. No second execution engine, no second validator, no second commit pipeline.

---

## Appendix A: Unit Conventions

The spec format accepts human-readable units everywhere. docli converts internally.

| Input | DXA | EMU | Use Case |
|-------|-----|-----|----------|
| `1in` | 1440 | 914400 | Margins, column widths |
| `1cm` | 567 | 360000 | Margins, column widths |
| `1mm` | 57 | 36000 | Fine positioning |
| `1pt` | 20 | 12700 | Font sizes, spacing |
| `1px` | 15 | 9525 | Image dimensions |

Agents never need to know DXA or EMU values.

## Appendix B: Error Codes

| Code | Meaning |
|------|---------|
| `FILE_NOT_FOUND` | Input file does not exist |
| `INVALID_DOCX` | File is not a valid DOCX (bad ZIP or missing required XML) |
| `INVALID_SPEC` | YAML spec has schema errors |
| `INVALID_JOB` | Job file has schema errors or unknown operation types |
| `INVALID_TARGET` | Selector resolved to nothing (bad index, no matching heading, story mismatch) |
| `INVALID_OPERATION` | Unknown or malformed operation |
| `REF_NOT_FOUND` | `$ref` or `kb://` URI not found in knowledge base |
| `VALIDATION_FAILED` | Document fails schema validation after edits |
| `INVARIANT_VIOLATION` | Hard invariant violated (e.g. structural nesting, missing required parts) |
| `ID_COLLISION` | `w:id` collision detected between bookmarks/comments/revisions (should never happen if using the unified allocator — indicates a bug) |
| `DEPENDENCY_MISSING` | Required external adapter (`soffice`, `pandoc`) not installed |
| `TEMPLATE_NOT_FOUND` | Named template not in knowledge base |
| `TEMPLATE_VAR_MISSING` | Required template variable not provided |
| `COMMIT_FAILED` | Shadow-package write or atomic rename failed (filesystem error) |
| `REVALIDATION_FAILED` | Paranoid-mode reopen validation found corruption in the shadow package |

## Appendix C: Comparison with Current Skill

| Dimension | Current Skill | docli |
|-----------|--------------|-------|
| Languages | Python + Node.js | Rust (single static binary) |
| Agent context cost | ~600 lines (SKILL.md) | ~60 lines |
| Output format | Human-readable text | Structured JSON with typed envelopes |
| State management | Agent manages temp dirs | Internal, invisible |
| Addressing | Raw XML / paragraph guessing | Typed selectors with story scopes |
| Creation model | Imperative JS API | Declarative YAML specs + `CreateBackend` trait |
| Edit model | Manual XML manipulation | Custom OOXML patcher (`docli-patch`) |
| Edit granularity | Batch only (multi-step ceremony) | Narrow verbs (micro-commit) or batch (`docli run`) |
| Validation | Separate script, text output | Built-in hard invariants + KB rules, JSON report |
| ID management | Manual, collision-prone | Unified `w:id` allocator, collision-free by construction |
| Template support | None | Knowledge base with YAML specs + OOXML-native assets |
| Error handling | String matching on "Error" | Typed error codes, structured JSON |
| Composability | Manual pipeline | `$ref`, pipes, job files, sub-agent delegation |
| Cold start | ~200ms (Python interpreter) | <1ms (native binary) |
| Runtime deps | Python 3.x, Node.js, pandoc, LibreOffice | None (pandoc/soffice optional adapters) |
| Atomicity model | Multi-step, agent-managed | Package-level: semantic + visibility + crash durability |
| Crash safety | None (rename without fsync) | Explicit durability modes (fast/durable/paranoid) |
| Crate strategy | N/A | Custom patcher for edits, pluggable backend for creation |

## Appendix D: Core Rust Types

The typed structures that `serde` serializes/deserializes. These types are
the contract between `docli` and every agent that calls it. They live in
`docli-core` and are re-exported by `docli-cli`.

```rust
// ═══════════════════════════════════════════════════════════════
// docli-core::envelope — JSON output contract
// ═══════════════════════════════════════════════════════════════

use serde::Serialize;

/// Every command returns this envelope on stdout.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Envelope<T: Serialize> {
    Ok(OkEnvelope<T>),
    Err(ErrEnvelope),
}

#[derive(Serialize)]
pub struct OkEnvelope<T: Serialize> {
    pub ok: bool,            // always true
    pub command: String,
    pub data: T,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct ErrEnvelope {
    pub ok: bool,            // always false
    pub command: String,
    pub error: ErrorDetail,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    FileNotFound,
    InvalidDocx,
    InvalidSpec,
    InvalidTarget,
    InvalidOperation,
    RefNotFound,
    ValidationFailed,
    InvariantViolation,
    DependencyMissing,
    TemplateNotFound,
    TemplateVarMissing,
    IdCollision,
}

// ═══════════════════════════════════════════════════════════════
// docli-core::commit — Durability modes and commit journal
// ═══════════════════════════════════════════════════════════════

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Durability {
    /// Temp file → write → rename. No fsync. For ephemeral sandboxes.
    Fast,
    /// Temp file → write → fsync temp → rename → fsync parent dir. Default.
    Durable,
    /// Build → reopen → revalidate → optionally render-test → durable commit.
    Paranoid,
}

#[derive(Serialize)]
pub struct CommitJournal {
    pub source_hash: String,          // sha256 of input package
    pub output_hash: String,          // sha256 of output package
    pub parts_modified: Vec<String>,  // e.g. ["word/document.xml", "word/comments.xml"]
    pub parts_unchanged: usize,
    pub durability: String,
    pub revalidated: bool,            // true in paranoid mode
}

// ═══════════════════════════════════════════════════════════════
// docli-query::selector — Target resolution
// ═══════════════════════════════════════════════════════════════

/// Story scope — which part of the document to search.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Story {
    #[default]
    Body,
    #[serde(rename = "header.default")]
    HeaderDefault,
    #[serde(rename = "header.first")]
    HeaderFirst,
    #[serde(rename = "header.even")]
    HeaderEven,
    #[serde(rename = "footer.default")]
    FooterDefault,
    #[serde(rename = "footer.first")]
    FooterFirst,
    #[serde(rename = "footer.even")]
    FooterEven,
    Footnotes,
    Endnotes,
    Comments,
}

/// Target selectors — deserialized from YAML ops files and --target CLI args.
#[derive(serde::Deserialize)]
#[serde(untagged)]
pub enum Target {
    Paragraph    { paragraph: usize, #[serde(default)] story: Story },
    Paragraphs   { paragraphs: Vec<usize>, #[serde(default)] story: Story },
    Range        { range: String, #[serde(default)] story: Story },          // "14:18"
    Heading      { heading: String, #[serde(default)] offset: usize,
                   #[serde(default)] story: Story },
    HeadingPath  { heading_path: String, #[serde(default)] offset: usize },  // "Results/CUPED"
    Table        { table: usize },
    Image        { image: usize },
    Style        { style: String, #[serde(default)] story: Story },
    Text         { text: String, #[serde(default)] regex: bool,
                   #[serde(default)] occurrence: Option<usize>,
                   #[serde(default)] story: Story },
    Bookmark     { bookmark: String },
    NodeId       { node_id: String },                                        // w14:paraId
    Contains     { contains: String, occurrence: usize,
                   #[serde(default)] story: Story },
}

/// Result of resolving a Target against a document index.
pub struct ResolvedTarget {
    pub paragraph_indices: Vec<usize>,
    pub part_path: String,             // e.g. "word/document.xml"
    pub byte_spans: Vec<(usize, usize)>,  // XML byte offsets for run-splitting
}

// ═══════════════════════════════════════════════════════════════
// docli-core::job — Unified IR (all verbs compile to this)
// ═══════════════════════════════════════════════════════════════

/// A job is a list of operations committed as one package transaction.
#[derive(serde::Deserialize)]
pub struct Job {
    pub operations: Vec<Operation>,
}

/// Every narrow verb and every entry in a job.yaml compiles to this enum.
#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Operation {
    // ── edit.* ──
    #[serde(rename = "edit.replace")]
    EditReplace      { target: Target, content: String },
    #[serde(rename = "edit.insert")]
    EditInsert       { target: Target, position: Position, content: Vec<ContentBlock> },
    #[serde(rename = "edit.delete")]
    EditDelete       { target: Target },
    #[serde(rename = "edit.find-replace")]
    EditFindReplace  { find: String, replace: String,
                       #[serde(default)] scope: Scope },
    #[serde(rename = "edit.update-table")]
    EditUpdateTable  { target: Target, cell: CellRef, content: String },
    #[serde(rename = "edit.append-row")]
    EditAppendRow    { target: Target, row: Vec<String> },
    #[serde(rename = "edit.replace-image")]
    EditReplaceImage { target: Target, path: String, width: Option<String> },
    #[serde(rename = "edit.set-style")]
    EditSetStyle     { target: Target, style: StyleOverride },
    #[serde(rename = "edit.set-heading")]
    EditSetHeading   { target: Target, level: u8 },

    // ── review.* ──
    #[serde(rename = "review.comment")]
    ReviewComment       { target: Target, text: String,
                          #[serde(default)] parent: Option<u64> },
    #[serde(rename = "review.track-replace")]
    ReviewTrackReplace  { target: Target, content: String },
    #[serde(rename = "review.track-insert")]
    ReviewTrackInsert   { target: Target, position: Position, content: Vec<ContentBlock> },
    #[serde(rename = "review.track-delete")]
    ReviewTrackDelete   { target: Target },

    // ── finalize.* ──
    #[serde(rename = "finalize.accept")]
    FinalizeAccept  { #[serde(default)] ids: Option<Vec<u64>> },
    #[serde(rename = "finalize.reject")]
    FinalizeReject  { #[serde(default)] ids: Option<Vec<u64>> },
    #[serde(rename = "finalize.strip")]
    FinalizeStrip   {},
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position { Before, After }

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope { All, First, Section(String) }

// ═══════════════════════════════════════════════════════════════
// docli-patch::id_alloc — Unified w:id allocator
// ═══════════════════════════════════════════════════════════════

/// One allocator for all OOXML ID spaces that can collide.
/// Initialized from the source document's existing IDs.
pub struct IdAllocator {
    next_id: u64,
    used: std::collections::HashSet<u64>,
}

impl IdAllocator {
    /// Scan all parts for existing w:id values (bookmarks, comments,
    /// revisions, footnotes, endnotes) and seed the allocator.
    pub fn from_package(package: &Package) -> Self { /* ... */ }

    /// Allocate a fresh ID guaranteed not to collide with any existing ID.
    pub fn next(&mut self) -> u64 { /* ... */ }
}

// ═══════════════════════════════════════════════════════════════
// docli-create::backend — Pluggable creation backend
// ═══════════════════════════════════════════════════════════════

/// Trait for greenfield document generation backends.
/// docx-rs is the v1 implementation; the trait lets us swap backends
/// if a crate turns out to have sharp edges.
pub trait CreateBackend {
    fn create(&self, job: &CreateJob) -> Result<PackageGraph>;
}
```

These types enforce the spec at compile time. An agent cannot construct an
invalid operation — `serde` deserialization fails with a typed error before
any document is touched. The `IdAllocator` and hard invariants in
`docli-schema` ensure that even valid-looking operations cannot produce
documents that trigger Word repair dialogs.

## Appendix E: Build & Distribution

### Building

```bash
# Development
cargo build

# Release (optimized, stripped)
cargo build --release
strip target/release/docli       # ~4-8 MB final binary

# Static linking (for maximum portability)
RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-musl
```

### Cross-Compilation Targets

| Target | Platform | Use Case |
|--------|----------|----------|
| `x86_64-unknown-linux-musl` | Linux x86_64 (static) | CI/CD, containers, agent sandboxes |
| `aarch64-unknown-linux-musl` | Linux ARM64 (static) | AWS Graviton, Apple Silicon VMs |
| `x86_64-apple-darwin` | macOS Intel | Developer laptops |
| `aarch64-apple-darwin` | macOS Apple Silicon | M-series Macs (M5 Max target) |
| `x86_64-pc-windows-msvc` | Windows x86_64 | Enterprise Windows environments |

The musl targets produce fully static binaries with no libc dependency —
they run on any Linux kernel without shared library concerns. This is
ideal for agent sandbox environments where installing system packages
is restricted or impossible.

### CI Pipeline

```yaml
# GitHub Actions matrix
strategy:
  matrix:
    include:
      - target: x86_64-unknown-linux-musl
        os: ubuntu-latest
      - target: aarch64-unknown-linux-musl
        os: ubuntu-latest
      - target: aarch64-apple-darwin
        os: macos-latest
      - target: x86_64-apple-darwin
        os: macos-latest
```

### Installation

```bash
# From release binary (recommended for agents)
curl -L https://github.com/{org}/docli/releases/latest/download/docli-$(uname -m)-linux -o docli
chmod +x docli
mv docli /usr/local/bin/

# From source
cargo install --path .

# Verify
docli --version
docli inspect --help
```

No Python. No Node.js. No package managers. One binary, zero runtime dependencies.
