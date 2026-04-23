# Python Companion To `docli`

## Purpose

`docli` should stay optimized for fast startup, atomic package operations, and
structured CLI output. A Python companion should handle the expensive work that
benefits from Python's broader ecosystem and where startup cost is acceptable.

The split is simple:

- `docli` is the fast path for inspect, patch, package transactions, and
  machine-readable summaries.
- Python is the slow path for heavyweight validation, semantic checks, final
  QA, and integration with rich analysis libraries.

This keeps the Rust tool small, predictable, and agent-friendly while still
letting us use the best available libraries for deep verification.

## Design Rule

Python should complement `docli`, not replace it.

- `docli` remains the source of truth for reading and mutating `.docx` packages.
- Python should operate after `docli` has produced a candidate document or a
  structured report.
- Python should be invoked for boundary steps:
  preflight, final check, CI verification, release gating, or batch audit.
- Python should avoid editing OOXML directly unless there is a compelling,
  isolated reason. The primary edit engine should remain in `docli`.

## Why This Split Works

Rust and Python have different strengths.

`docli` is better for:

- sub-second cold starts
- atomic single-shot CLI invocations
- package-safe read/modify/write flows
- deterministic JSON envelopes for agents and automation
- shipping as a single binary with minimal environment assumptions

Python is better for:

- richer validation libraries
- heavy parsing and reporting pipelines
- visual QA and document comparison workflows
- data-science-style heuristics over extracted structure
- integration with existing internal scripts and notebooks

The startup penalty is acceptable because Python should only run when we need a
deeper answer than `docli` alone should provide.

## Recommended Responsibilities

### `docli`

- open, inspect, and mutate `.docx` packages
- perform fast structural validation
- emit normalized JSON and YAML envelopes
- expose stable commands for extract, validate, render, diff, and finalize
- provide the low-latency primitive that agents can call repeatedly

### Python Companion

- run heavyweight validation passes over the candidate output
- aggregate multiple checks into a single final report
- perform semantic and policy validation using the wider Python ecosystem
- generate rich HTML or Markdown QA artifacts
- act as a CI gate or "final check" runner

## Best Use Cases For The Python Companion

### 1. Final Validation Gate

Run Python once after all `docli` edits complete.

Examples:

- compare `docli inspect` output against project-specific rules
- verify required sections, heading order, references, and appendix presence
- check table captions, figure numbering, and citation formatting
- confirm that tracked changes or comments are absent before release

### 2. Deep Content Policy Checks

Python is a good place for rules that are expensive or domain-specific.

Examples:

- style-guide enforcement
- glossary consistency
- prohibited language detection
- metadata completeness checks
- cross-document consistency across a batch of files

### 3. Visual and Rendering QA

Python can orchestrate tools and compare their outputs more effectively than a
thin CLI alone.

Examples:

- render DOCX to PDF, then compare page images
- detect layout drift using image diff or OCR
- inspect generated PDFs for missing fonts, blank pages, or broken tables

### 4. Batch Audits And Reporting

Python is a natural fit for repository-wide analysis.

Examples:

- validate hundreds of reports overnight
- build a dashboard of failures by category
- summarize recurring template defects
- export CSV, JSON, or HTML reports for humans

## Recommended Interface Between `docli` And Python

Keep the interface narrow and stable.

### Contract

- Python consumes document paths and `docli` JSON output.
- `docli` does not need to know Python internals.
- Python should treat `docli` as an external command with a stable schema.
- The handoff format should be JSON files or stdout, not ad hoc text parsing.

### Suggested Flow

1. `docli` produces or validates a candidate `.docx`.
2. `docli inspect`, `docli validate`, `docli read`, or `docli diff` emits JSON.
3. Python consumes those envelopes plus the file path.
4. Python runs deeper checks and produces a consolidated report.
5. CI or the agent decides whether the document is acceptable.

### Good Boundary Objects

- source document path
- candidate output path
- `docli inspect` JSON envelope
- `docli validate` JSON envelope
- render artifacts such as PDF or PNG pages
- a final Python report in JSON and Markdown

## Symbiotic Architecture

The most important design choice is that the Python layer should not behave
like a second document engine. It should behave like an orchestration and
assurance layer around `docli`.

### Division Of Labor

- `docli` owns package truth
- `docli` owns OOXML mutation
- `docli` owns atomic commit semantics
- Python owns multi-pass interpretation
- Python owns heavy policy, QA, and corpus-level analysis
- Python can propose actions, but `docli` should execute the actual edits

This means the relationship is symbiotic:

- `docli` provides fast structured facts
- Python turns those facts into higher-order judgments
- Python can feed a report, a risk score, or an action plan back into `docli`

### Three Operating Loops

#### 1. Inner Loop: Fast Authoring

This loop should stay almost entirely inside `docli`.

- inspect
- patch
- validate
- read

The goal is low-latency iteration.

#### 2. Outer Loop: Deep Final Check

This is where Python should run.

- consume one or more `docli` envelopes
- run semantic, policy, and rendering checks
- emit a combined report and release decision

The goal is assurance, not speed.

#### 3. Fleet Loop: Corpus Audit

This is the most Python-native loop.

- scan many documents
- normalize results into a table or warehouse
- compute recurring defects and trends
- produce dashboards, scorecards, and upgrade priorities

The goal is visibility across many documents, not one edit session.

## Example Workflow

### Fast Iteration Loop

Use only `docli` during editing:

```text
docli inspect report.docx
docli edit.replace report.docx ...
docli validate report.docx
```

This keeps agent loops fast.

### Slow Final Check

Run Python only when the document is ready for review:

```text
docli inspect report.docx --format json > /tmp/report.inspect.json
docli validate report.docx --format json > /tmp/report.validate.json
python tools/docli_companion.py final-check \
  --doc report.docx \
  --inspect /tmp/report.inspect.json \
  --validate /tmp/report.validate.json
```

That gives us a cheap inner loop and an expensive outer gate.

## Mac-First Recommendation

If this companion must be reliable on macOS and remain multi-platform, the
default stack should avoid Windows-only automation and avoid assuming Microsoft
Word is installed.

### Approved Default Stack

- `zipfile` + `lxml` for package inspection, raw OOXML validation, and final
  part-level audits
- `docx2python` for structured extraction, including text, images, comments,
  footnotes, and other review-friendly content
- `python-docx` for light document generation and simple edits
- `docxtpl` for template-driven generation from human-authored `.docx`
  templates
- `mammoth` for semantic HTML conversion and human-reviewable output
- `pypandoc-binary` for conversion smoke tests on macOS without adding a
  separate Pandoc install burden

This is the best default because it stays cross-platform, works well on modern
Mac hardware, and keeps us inside libraries that are practical to install in
CI and on developer laptops.

### What This Stack Is Good At

- final validation and policy checks
- structured extraction for QA and analysis
- template-based document generation
- semantic review and lightweight conversion checks
- repository-wide auditing and report generation

### What This Stack Is Not Good At

- Word-perfect tracked-revision workflows
- full-fidelity layout editing
- complete coverage of advanced WordprocessingML features
- exact parity with Microsoft Word behavior

Open-source Python on macOS is good enough for validation and extraction. It is
not the right place to promise perfect Word-native editing fidelity.

### Mac-Safe Escalation Path

If we decide we need higher-fidelity review or rendering behavior on macOS, the
best escalation path is a commercial SDK instead of Windows automation.

Recommended escalation:

- `Aspose.Words for Python via .NET`

Use that only when we need features such as:

- stronger tracked-changes handling
- richer comment/revision workflows
- higher-fidelity rendering and format conversion
- broader coverage of complex Word document features

### What To Avoid For A Mac-First Design

- `pywin32` and COM automation
- any workflow that requires Microsoft Word to be installed
- making LibreOffice the primary edit engine
- building the core architecture around Windows-only APIs

Those options either do not run on macOS or create an avoidable portability
problem.

## Research-Backed Option Map

The Python landscape is not one library. It is a stack of specialized tools.

| Option | Best Role | Why It Matters | Limitation |
|---|---|---|---|
| `python-docx` | light generation and editing | Good for common paragraphs, tables, styles, comments, and simple document assembly | not a full-fidelity Word engine |
| `docxtpl` | template-first generation | Lets humans author `.docx` templates and Python fill them | not a general editing system |
| `docx2python` | rich extraction and audit | Strong for pulling text, images, comments, footnotes, and document content into Python | extraction-oriented rather than mutation-oriented |
| `mammoth` | semantic HTML review | Good for turning Word structure into review-friendly HTML | not layout-faithful for complex docs |
| `pypandoc-binary` | conversion smoke tests | Useful when we want a portable Pandoc-backed conversion path on macOS | conversion model is not Word-native OOXML |
| `unstructured` | element partitioning | Useful when we want typed elements like titles, narrative text, and list items | more analysis-focused than package-faithful |
| `Docling` | experimental unified ingestion | Promising when we want one representation across DOCX, PDF, HTML, PPTX, and more | broader and heavier than a minimal companion |
| `Aspose.Words for Python via .NET` | premium high-fidelity path | Best Mac-safe escalation when we need stronger revision and rendering fidelity | commercial dependency |

## Recommended Dependency Tiers

Not every installation should pull every package.

### Tier 0: Always-On Core

These should be considered the baseline companion stack.

- `zipfile`
- `lxml`
- `pydantic`
- `jsonschema`

These support package inspection, validation, typed reports, and contract
checking without creating a heavy runtime burden.

### Tier 1: Standard Mac-First Companion

These are the best default additions for a serious local workflow.

- `docx2python`
- `mammoth`
- `pypandoc-binary`

This tier gives us strong extraction, semantic review, and practical
conversion checks on macOS.

### Tier 2: Authoring And Template Support

Add these only when the Python layer needs to create documents or fill
templates.

- `python-docx`
- `docxtpl`

### Tier 3: Advanced Analysis

These are worth using when the Python layer starts doing richer content
analysis or warehouse-style reporting.

- `duckdb`
- `opencv-python`
- `pytesseract`
- `unstructured`
- `Docling`

### Tier 4: Premium Fidelity

This tier should be optional and explicitly approved.

- `Aspose.Words for Python via .NET`

This is the clean escalation path when open-source Python is no longer enough.

## Python Ecosystem To Leverage

The companion should use Python where the ecosystem is clearly stronger.

### Document and XML Handling

- `lxml` for powerful XML and XPath inspection
- `zipfile` for package inspection when raw archive access is enough
- `python-docx` for high-level read-only convenience checks
- `docx2python` for text and table extraction shortcuts
- `mammoth` for semantic DOCX-to-HTML inspection

### Validation and Data Modeling

- `pydantic` for typed reports and configuration
- `jsonschema` for validating `docli` output contracts
- `schemathesis` or similar tools if API-style schema testing becomes useful

### Data and Heuristic Analysis

- `pandas` for large batch reports
- `duckdb` for local analytics over many final-check artifacts
- `rapidfuzz` for approximate heading or phrase matching
- `regex` for advanced style or policy scanning
- `networkx` if document structure graphs become useful

### Document Intelligence And Partitioning

- `unstructured` for typed element extraction
- `Docling` for broader cross-format ingestion and unified representations

### Rendering and Final QA

- `pypdf` and `pdfplumber` for PDF inspection
- `Pillow` and `opencv-python` for page image comparison
- `pytesseract` if OCR-based checks are needed
- `matplotlib` or `plotly` for QA dashboards

### Premium Fidelity

- `Aspose.Words for Python via .NET` for the Mac-safe high-fidelity escalation
  path when open-source capabilities are no longer sufficient

## Suggested Python Companion Modes

A single script can expose a few high-value subcommands.

### `final-check`

Runs the full release gate for one document.

Checks could include:

- `docli` structural validity
- required sections
- policy rules
- citation or appendix conventions
- rendered PDF sanity checks
- report generation

### `batch-audit`

Runs the same logic across many documents and produces a summary table.

### `visual-check`

Focuses on PDF conversion, page rasterization, and visual drift detection.

### `policy-check`

Runs domain-specific rules that should not be embedded into the Rust binary.

### `propose-fix`

Produces a structured action plan for `docli` rather than mutating the package
directly. The Python layer diagnoses; `docli` executes.

### `corpus-audit`

Scans many documents, stores normalized facts, and computes failure patterns
across a whole body of work rather than one file at a time.

### `render-check`

Converts, rasterizes, compares, and summarizes layout or typography drift using
rendered artifacts instead of only OOXML-level structure.

## Bold Ideas Worth Adding

The strongest version of the companion is not just a validator. It is a Python
intelligence layer that can look at documents from several angles and then feed
decisions back into `docli`.

### 1. The Shadow Committee

Run multiple parsers over the same document and treat disagreement as signal.

Possible members:

- `docli inspect` for package-faithful structure
- `docx2python` for extraction-focused content recovery
- `mammoth` for semantic HTML interpretation
- `unstructured` for typed elements
- `Docling` for a broader unified document representation

This creates a powerful final-check pattern:

- if all parsers agree, confidence is high
- if only one parser sees a heading, table, or comment, confidence drops
- if semantic extraction diverges from OOXML facts, the report should flag it

This is bold because it turns library disagreement into a detection mechanism
instead of a bug.

### 2. Render And Read Back

Do not trust structure alone. Render the document and inspect the output that a
human would actually read.

Pipeline:

1. `docli` produces the candidate `.docx`
2. Python converts the document to PDF or HTML
3. Python rasterizes pages where needed
4. OCR and visual checks confirm that headings, footnotes, tables, and visible
   content match expectations

This catches classes of bugs that OOXML validation will miss:

- white-on-white text
- off-page content
- broken pagination
- missing glyphs or substituted fonts
- table overflow and clipping

### 3. Python As Planner, Rust As Actor

The companion should be allowed to recommend repairs without becoming the edit
engine.

Pattern:

- Python analyzes a document
- Python emits a typed remediation plan
- `docli` applies the plan atomically
- Python reruns validation

This is the cleanest symbiosis in the whole design.

The plan can contain:

- section insertions to request from templates
- replacement targets
- style normalization suggestions
- metadata repairs
- review comments to insert

### 4. A Document Warehouse

Treat document QA as data engineering.

Store normalized outputs from `docli` and the companion in Parquet and query
them with DuckDB. This makes cross-document analysis cheap and reproducible.

Examples:

- most common validation failures by template family
- heading drift by team or author
- average table density by report type
- comment volume before release
- tracked-change leakage rate over time

This is the right way to make the companion useful at organization scale rather
than only file-by-file.

### 5. Confidence-Weighted Decisions

The companion should not force every check into a binary pass/fail if the
signal is probabilistic.

Instead, produce:

- hard failures
- soft warnings
- confidence scores
- evidence snippets
- recommended next action

That lets us treat some checks as release blockers and others as triage inputs.

### 6. Domain Packs

The Python layer is the right home for heavyweight domain packs that would be
too large or too volatile to compile into `docli`.

Possible packs:

- regulated-report pack
- academic-manuscript pack
- legal-brief pack
- board-deck narrative pack
- accessibility pack

Each pack can add:

- policy rules
- extraction heuristics
- required section schemas
- report templates
- severity thresholds

### 7. Golden Corpus Regression

Maintain a set of canonical Word documents that represent the hard cases:

- dense tables
- tracked changes
- comments and replies
- floating images
- section breaks
- headers and footers
- footnotes and endnotes

Every upgrade to `docli` or the Python stack should rerun the same final-check
bundle on this corpus. This creates a practical guardrail against accidental
fidelity regressions.

### 8. Air-Gapped Heavy Mode

The companion should support a mode that runs fully offline for sensitive
documents.

This is where local-capable tools matter:

- `docli`
- `lxml`
- `docx2python`
- `mammoth`
- `Docling`
- local OCR tools
- local DuckDB databases

The architecture should not require a cloud API to perform high-value checks.

### 9. Optional LLM Advisory Layer

If we ever add an LLM step, it should remain advisory and typed.

Rules:

- the model never edits OOXML directly
- the model never bypasses `docli`
- every model output must validate into a typed Python schema
- all hard release decisions still depend on deterministic evidence

Good uses:

- summarizing the report for humans
- suggesting likely causes for a failure cluster
- drafting remediation notes
- generating candidate `docli` action plans for review

Bad uses:

- silently rewriting documents
- making the release decision without evidence
- becoming the primary parser

## Companion Artifact Bundle

The Python layer should emit more than a pass/fail bit.

For each run, we should aim to capture a small artifact bundle:

- `report.json`
- `report.md`
- `inspect.json`
- `validate.json`
- optional `semantic.html`
- optional `render.pdf`
- optional page images
- optional `facts.parquet`

This makes every final-check run inspectable and reproducible.

### Suggested Report Sections

- document identity
- tool versions
- source file hashes
- hard failures
- warnings
- confidence summary
- parser disagreement summary
- rendering summary
- recommended next action

## Decision Model

The companion should make it obvious how a result should be interpreted.

### Release States

- `pass`
- `pass-with-warnings`
- `manual-review`
- `fail`

### Evidence Classes

- deterministic structural evidence
- deterministic policy evidence
- extraction disagreement
- rendering evidence
- OCR evidence
- advisory model evidence

Deterministic evidence should always outrank probabilistic evidence.

## What Should Stay Out Of Python

To avoid architectural drift, the companion should not become the real engine.

Avoid using Python for:

- the default edit path
- repeated small mutations during agent loops
- package commit semantics
- low-level OOXML rewrite logic that `docli` already owns
- core CLI contracts that should stay in one place

If Python becomes responsible for core document mutation, startup cost and
behavioral drift will undo the main point of `docli`.

## Implementation Guidance

If we build this, the implementation should deepen in layers instead of trying
to solve every problem at once.

### Milestone 1: Typed Final Check

- create `tools/docli_companion.py`
- shell out to `docli`
- read `docli` JSON envelopes
- validate all companion outputs with `pydantic`
- emit a combined JSON and Markdown report
- focus on `final-check` for one document

### Milestone 2: Rich Extraction And Semantic Review

- add `docx2python`
- add `mammoth`
- add parser disagreement checks
- add section-schema and content-policy packs
- emit review-friendly HTML snippets where useful

### Milestone 3: Rendering And Visual QA

- add PDF conversion checks
- add rasterized page comparisons
- add OCR-assisted verification for visible text
- promote render-based failures to first-class evidence

### Milestone 4: Corpus Intelligence

- store normalized facts in Parquet
- query audits with DuckDB
- add dashboards and scorecards
- track failure classes across time, team, and template

### Milestone 5: Action Plans And Assisted Remediation

- emit typed fix proposals
- map fix proposals to `docli` operations
- allow a human or agent to approve application
- rerun final-check automatically after repair

### Milestone 6: Premium Fidelity Option

- add an optional `Aspose` backend only if open-source fidelity proves
  insufficient
- keep that backend isolated behind a feature flag or install extra
- continue to prefer `docli` as the mutation layer even if premium inspection
  is enabled

## Recommended Operating Model

Use this rule of thumb:

- call `docli` many times during authoring and editing
- call Python once per milestone, review, or release boundary

That gives us:

- fast agent feedback loops
- richer final assurance
- better separation of concerns
- freedom to use the Python ecosystem without paying its startup cost on every
  small operation

## Bottom Line

The best complement is not "Python instead of Rust." It is "Python after
Rust."

`docli` should remain the fast transactional core. A Python companion should be
the heavyweight verifier, auditor, and final-check layer that runs less often
but goes much deeper.
