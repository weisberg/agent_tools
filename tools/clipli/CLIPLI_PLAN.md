# clipli Development Plan

**Spec:** `CLIPLI_SPEC.md` v1.0.0-spec  
**Current crate version:** `0.1.0`  
**Plan updated:** 2026-03-26

---

## Overview

This plan replaces the original bootstrap-oriented phase checklist with a roadmap that matches the current codebase.

clipli is no longer an MVP scaffold. The project already has:

- [x] working macOS pasteboard read/write support
- [x] a mature HTML cleaning pipeline
- [x] template rendering and built-in table/slide templates
- [x] heuristic templatization
- [x] filesystem-backed template storage
- [x] rich Excel HTML generation and clipboard editing
- [x] a passing automated test baseline

The next stage is to turn that solid core into a more complete product: close the highest-value correctness gaps, harden workflows, add missing features promised by the spec, and then expand into automation, sharing, and agent-native integrations.

---

## Current State Snapshot

### What is already implemented

- [x] `inspect`, `read`, `write`, `capture`, `paste`, `list`, `show`, `edit`, `delete`, `excel`, `excel-edit`, and `convert` commands
- [x] pasteboard support for HTML, RTF, plain text, PNG, TIFF, and PDF payloads
- [x] HTML cleaning with target-aware CSS filtering for Excel, PowerPoint, Google Sheets, and generic HTML
- [x] Jinja2-compatible rendering with custom filters and HTML-to-plain-text conversion
- [x] heuristic templatization for dates, currency, percentages, emails, large numbers, quarters, and cell text
- [x] template storage under `~/.config/clipli/templates/`
- [x] rich CSV-to-Excel HTML generation and A1-style Excel cell editing

### Verified baseline

`cargo test` currently passes, including the unit and integration suites, with GUI-dependent pasteboard tests still ignored as expected.

### Completed so far

- [x] core clipboard I/O is implemented for the primary macOS pasteboard formats used by clipli
- [x] the HTML cleaning pipeline is implemented and covered by fixture-oriented tests
- [x] rendering, built-in templates, and HTML-to-plain-text fallback are implemented
- [x] heuristic templatization is implemented and round-trip tested
- [x] template capture, storage, listing, editing, showing, and deletion are implemented
- [x] Excel HTML generation and clipboard editing workflows are implemented beyond the original MVP scope
- [x] the current automated baseline is green across unit and integration tests, with only GUI-dependent clipboard tests ignored

### Highest-confidence gaps from the current implementation

- [ ] `convert --from rtf --to html` is still explicitly unimplemented
- [ ] config is loaded, but defaults are not consistently honored across commands
- [ ] agent templatization uses a stdin/stdout protocol only; clipli does not yet invoke an external agent command itself
- [ ] template storage has no versioning, rollback, locking, or import/export story
- [ ] `capture` does not yet provide the preview workflow described in the spec
- [ ] shell completions, richer diagnostics, and release/distribution work are still missing

### Strategic implication

The roadmap should optimize for four outcomes:

1. complete the core spec where the product still has sharp edges
2. make templates safer and easier to manage over time
3. make clipli a stronger agent workflow primitive
4. expand into automation and collaboration once the core is dependable

---

## Product Direction

clipli should evolve into a clipboard automation platform with four layers:

1. **Core clipboard correctness**  
   Lossless-ish capture, render, and paste across common macOS productivity apps.

2. **Reusable template workflows**  
   Safe capture, editing, versioning, search, linting, export/import, and team reuse.

3. **Agent-native execution**  
   Clean machine-readable interfaces for external agents, batch jobs, and eventually MCP.

4. **Automation and operations**  
   Clipboard history, watch mode, previews, UI helpers, and packaging/distribution.

The version roadmap below is sequenced around those layers.

---

## Version Roadmap

## v0.2 — Core Completion and Correctness

**Goal:** Close the most important spec gaps and make the existing command set reliable enough for daily use.

### Primary deliverables

- [ ] Implement `rtf -> html` conversion in `convert`
- [ ] Make config defaults actually influence command behavior end-to-end
- [ ] Add `capture --preview`
- [ ] Tighten plain-text output behavior for `paste` and `convert`
- [ ] Improve structured error output consistency
- [ ] Expand regression coverage around missing and partial behaviors

### Detailed scope

#### 0.2.1 RTF conversion

- [ ] Add an internal `rtf_to_html()` path used by `convert` and optionally by `capture` fallback workflows
- [ ] Preserve at least bold, italic, underline, font family, font size, foreground color, paragraph breaks, and table-ish structures where possible
- [ ] Define explicit failure behavior for unsupported RTF constructs instead of silently degrading
- [ ] Add tests using realistic RTF fixtures from common macOS apps

**Acceptance:**

- [ ] `clipli convert --from rtf --to html` works on representative RTF samples
- [ ] `capture` can produce useful output when HTML is absent but RTF is present

#### 0.2.2 Config cascade cleanup

- [ ] Audit every command that should honor config defaults
- [ ] Ensure `defaults.font`, `defaults.font_size_pt`, `defaults.plain_text_strategy`, `clean.keep_classes`, `clean.target_app`, and `templatize.default_strategy` are applied consistently
- [ ] Define clear precedence: CLI flags > command defaults from config > built-in defaults
- [ ] Add tests for config-on / config-off behavior

**Acceptance:**

- [ ] Changing config materially changes behavior in tested commands without requiring equivalent CLI flags

#### 0.2.3 Capture and paste polish

- [ ] Add `capture --preview` by writing the cleaned or templatized output to a temp file and opening it
- [ ] Audit `paste --plain-text auto|tab-delimited|none` so behavior is explicit and stable
- [ ] Improve `show --open` and `paste --open` temp-file handling
- [ ] Make validation errors more actionable for invalid names, invalid JSON, and missing template data

**Acceptance:**

- [ ] preview flows work for capture, show, and paste
- [ ] plain-text modes behave deterministically and are documented

#### 0.2.4 JSON and error surface hardening

- [ ] Standardize error envelopes where `--json` is supported
- [ ] Add command-level error codes where only string errors exist today
- [ ] Reduce `Box<dyn std::error::Error>` escape hatches in command paths where typed errors are practical
- [ ] Add tests for error JSON output, not just happy paths

**Acceptance:**

- [ ] machine-readable consumers can reliably inspect failure codes across core commands

#### 0.2.5 Core regression coverage

- [ ] Add tests for config usage
- [ ] Add tests for `capture --preview` and preview temp-file generation where feasible
- [ ] Add more conversion tests for malformed input and fallback flows
- [ ] Add at least one real-world RTF fixture suite

### Risks

- [ ] RTF fidelity may be materially worse than HTML fidelity; document known limits rather than overpromising
- [ ] config cleanup can accidentally change existing defaults; preserve behavior where practical and call out intentional changes

### Exit criteria

- [ ] The spec-promised core conversion and preview workflows are implemented
- [ ] Config defaults behave consistently
- [ ] JSON output is more uniform
- [ ] The project remains green on `cargo test`

---

## v0.3 — Template Safety, Search, and Lifecycle

**Goal:** Make templates durable, discoverable, and safer to evolve over time.

### Primary deliverables

- [ ] template versioning and rollback
- [ ] template linting and validation improvements
- [ ] content-based template search
- [ ] import/export bundles
- [ ] safer store writes and recovery paths

### Detailed scope

#### 0.3.1 Template versioning

- [ ] Store historical snapshots under each template directory
- [ ] Track enough metadata to show when a version was created and from what change path
- [ ] Add commands such as:
  - [ ] `clipli versions <NAME>`
  - [ ] `clipli show <NAME> --version <ID>`
  - [ ] `clipli restore <NAME> --version <ID>`
- [ ] Decide whether versions are full copies or delta-free snapshots; favor simplicity first

**Acceptance:**

- [ ] editing or recapturing a template no longer destroys prior working states

#### 0.3.2 Template linting

- [ ] Add `clipli lint <NAME>`
- [ ] Detect:
  - [ ] undefined variables
  - [ ] variables present in schema but unused in template
  - [ ] duplicate variable names
  - [ ] suspicious default values
  - [ ] unbalanced Jinja markers and invalid identifiers
- [ ] Surface warnings separately from hard failures

**Acceptance:**

- [ ] users can validate templates before a bad paste flow or broken batch run

#### 0.3.3 Search and discovery

- [ ] Add full-text search across template HTML, schema, and metadata
- [ ] Support search by name, tags, source app, and content snippets
- [ ] Add command:
  - [ ] `clipli search <QUERY>`
- [ ] Keep implementation simple at first, likely filesystem scan plus indexed metadata if needed later

**Acceptance:**

- [ ] users with a non-trivial template library can reliably find the right template

#### 0.3.4 Import/export bundles

- [ ] Export a template directory as a portable `.clipli` bundle
- [ ] Import a bundle into the store with collision handling
- [ ] Preserve `meta.json`, `schema.json`, template content, `original.html`, and `raw.html`
- [ ] Consider optional manifest versioning for future compatibility

**Acceptance:**

- [ ] a template can be transferred between machines without manual directory copying

#### 0.3.5 Store durability

- [ ] Add atomic writes where practical
- [ ] Add simple lock-file or temp-file strategy to reduce corruption risk
- [ ] Ensure `force` overwrites are safe and recoverable
- [ ] Decide behavior for malformed `meta.json` in partially corrupted template directories

### Risks

- [ ] versioning and bundle design become de facto public formats; keep them simple and documented
- [ ] search can drift into building a database too early; start with file-backed indexing only if needed

### Exit criteria

- [ ] templates are versioned and recoverable
- [ ] users can lint, search, export, and import templates
- [ ] store writes are materially safer than they are today

---

## v0.4 — Agent-Native Workflows

**Goal:** Make clipli a first-class primitive in AI and automation pipelines, not just a CLI a user manually chains.

### Primary deliverables

- [ ] external agent command execution for templatization
- [ ] stronger agent-response validation
- [ ] richer machine-readable command outputs
- [ ] batch rendering workflows
- [ ] better debugging and observability for automation use

### Detailed scope

#### 0.4.1 External agent command integration

- [ ] Extend `capture --strategy agent` to optionally invoke an external command directly
- [ ] Add flags such as:
  - [ ] `--agent-command`
  - [ ] `--agent-timeout`
  - [ ] `--agent-arg` if needed, or support a shell-free command/args shape
- [ ] Continue to support the current stdin/stdout protocol mode for advanced users
- [ ] Capture stderr and exit status cleanly

**Acceptance:**

- [ ] users can run a single clipli command that delegates templatization to an external LLM tool

#### 0.4.2 Agent validation hardening

- [ ] Validate variable names more strictly
- [ ] Validate returned template structure against the input in a practical way
- [ ] Detect suspicious changes such as removed tables, added scripts, or attribute rewrites outside expected bounds
- [ ] Add fallback behavior when the agent response is malformed

**Acceptance:**

- [ ] agent-powered capture is safe enough to trust in semi-automated workflows

#### 0.4.3 Batch rendering

- [ ] Add batch render/paste workflow, for example:
  - [ ] `clipli paste-batch <NAME> --data-file rows.json`
  - [ ] `clipli render <NAME> --output-dir ...`
- [ ] Support arrays of objects, newline-delimited JSON, and CSV-backed input as follow-on formats if needed
- [ ] Allow file output as well as clipboard output

**Acceptance:**

- [ ] clipli can generate many outputs from one template without forcing users to script the loop themselves

#### 0.4.4 Machine-readable command contracts

- [ ] Audit JSON output across `capture`, `paste`, `list`, `show`, `convert`, and future batch flows
- [ ] Make success outputs stable and explicit enough for programmatic callers
- [ ] Add example automation docs to the spec or README

#### 0.4.5 Debuggability

- [ ] Add `-v` / `-vv` style logging or an equivalent debug mode
- [ ] Log which source type was captured, which strategy was used, and which template was loaded
- [ ] Provide enough visibility for users diagnosing agent failures or data merge mistakes

### Risks

- [ ] external process execution can introduce quoting and security hazards; avoid shell-based execution
- [ ] stronger validation can reject useful agent output if rules are too strict; start with conservative structural checks

### Exit criteria

- [ ] agent templatization supports a one-command workflow
- [ ] batch rendering exists for automation-heavy use
- [ ] JSON output and logging are strong enough for scripted use

---

## v0.5 — Automation, History, and Power-User Flows

**Goal:** Expand clipli from a clipboard templating tool into a broader clipboard workflow system.

### Primary deliverables

- [ ] clipboard watch mode and history capture
- [ ] history search and replay
- [ ] improved preview and browser workflows
- [ ] deeper Excel and table automation

### Detailed scope

#### 0.5.1 Clipboard watch and history

- [ ] Add `clipli watch`
- [ ] Persist captures with timestamps, source app, and content fingerprints
- [ ] Deduplicate by hash where appropriate
- [ ] Define a storage structure that can scale without becoming opaque

**Acceptance:**

- [ ] users can build a searchable clipboard history instead of relying on one-off captures

#### 0.5.2 History query and replay

- [ ] Add commands such as:
  - [ ] `clipli history list`
  - [ ] `clipli history search <QUERY>`
  - [ ] `clipli history show <ID>`
  - [ ] `clipli history restore <ID>`
- [ ] Support filtering by source app, type, and date range

#### 0.5.3 Preview improvements

- [ ] Improve HTML preview ergonomics across capture, show, and paste
- [ ] Decide whether to keep temp files, overwrite a stable temp location, or add a small preview cache
- [ ] Consider a `clipli preview` command for explicit previewing without pasting

#### 0.5.4 Excel workflow extensions

- [ ] Build on `excel` and `excel-edit` rather than replacing them
- [ ] Candidate additions:
  - [ ] merged cell helpers beyond title rows
  - [ ] reusable formatting presets
  - [ ] richer number-format helpers
  - [ ] named style presets
  - [ ] table transforms from JSON as well as CSV

### Risks

- [ ] watch mode turns clipli into a longer-running tool with different operational concerns
- [ ] history can accumulate sensitive data; storage model and documentation must respect that reality

### Exit criteria

- [ ] clipli can passively collect and actively query clipboard history
- [ ] power users can automate more of their recurring Excel and preview workflows

---

## v0.6 — Distribution, Interfaces, and Ecosystem

**Goal:** Make clipli easier to install, integrate, and extend.

### Primary deliverables

- [ ] shell completions
- [ ] CI and release automation
- [ ] packaging and installation polish
- [ ] optional library extraction
- [ ] initial platform/interface expansion groundwork

### Detailed scope

#### 0.6.1 Shell completions and help polish

- [ ] Add `clap_complete`
- [ ] Generate bash, zsh, and fish completions
- [ ] Improve command help text with realistic examples
- [ ] Add template-name completion if practical

#### 0.6.2 CI and release pipeline

- [ ] Add GitHub Actions for build, test, clippy, and fmt
- [ ] Document expected handling for GUI-only tests
- [ ] Add release packaging for macOS

#### 0.6.3 Packaging

- [ ] Support `cargo install` cleanly
- [ ] Add Homebrew distribution if desired
- [ ] Ensure release artifacts are easy to download and verify

#### 0.6.4 Internal interfaces

- [ ] Evaluate extracting reusable internals into a `clipli-core` library crate once APIs stabilize
- [ ] Keep the binary UX first; only split once the boundaries are obvious

#### 0.6.5 Forward-looking integration groundwork

- [ ] Prepare for:
  - [ ] MCP server support
  - [ ] local preview server
  - [ ] future non-macOS abstraction layers
- [ ] Do not commit to full cross-platform support until the macOS product is clearly stable and ergonomic

### Exit criteria

- [ ] clipli is easier to install and use from a shell
- [ ] CI protects the mainline
- [ ] the codebase is structured for broader integrations without premature abstraction

---

## v1.0 — Stable, Trusted Core Product

**Goal:** Declare the macOS core product stable.

### Requirements for v1.0

- [ ] core capture, render, and paste workflows are dependable on representative Excel, PowerPoint, Google Sheets, browser, and text-editor inputs
- [ ] RTF fallback is implemented and documented
- [ ] config behavior is consistent and tested
- [ ] template versioning, linting, and search exist
- [ ] agent integration is production-usable
- [ ] CI, packaging, completions, and docs are in place
- [ ] the release notes clearly distinguish stable core features from experimental ones

### Features that may remain post-1.0 or experimental

- [ ] clipboard watch/history
- [ ] MCP server
- [ ] preview server
- [ ] image templates
- [ ] cross-platform support

---

## Cross-Cutting Workstreams

These workstreams should progress alongside the version milestones rather than being deferred to the end.

### 1. Test Strategy

- [ ] keep `cargo test` green at every milestone
- [ ] grow fixture coverage with real clipboard HTML and RTF samples
- [ ] add targeted regression tests before fixing parser or store bugs
- [ ] keep GUI-dependent pasteboard tests available and documented even if CI cannot run them

### 2. Error Model and Diagnostics

- [ ] move toward typed command errors where it improves clarity
- [ ] keep human-readable output friendly
- [ ] keep JSON error codes stable once published

### 3. Template Data Model Evolution

- [ ] evolve `TemplateMeta` carefully if adding version IDs, provenance, bundle metadata, or draft/published states
- [ ] avoid unnecessary format churn in `meta.json` and `schema.json`

### 4. Store Safety and Backward Compatibility

- [ ] preserve compatibility with existing on-disk templates whenever possible
- [ ] if migrations become necessary, make them explicit and reversible

### 5. Documentation

- [ ] keep `CLIPLI_SPEC.md` aligned with actual behavior
- [ ] add practical examples for the commands that matter most in agent workflows
- [ ] document known fidelity limits instead of implying perfect round-trip behavior

---

## Recommended Sequencing

### Immediate order

1. `v0.2` core completion and correctness
2. `v0.3` template lifecycle and safety
3. `v0.4` agent-native workflows

### Next wave

4. `v0.5` automation and history
5. `v0.6` distribution, interfaces, and ecosystem

### Why this order

- [ ] It fixes the most obvious user-facing correctness gaps first.
- [ ] It protects user-generated assets before adding more automation on top.
- [ ] It makes agent integration more trustworthy by building on a safer core.
- [ ] It postpones broader operational surface area until the fundamentals are stable.

---

## Deferred / Optional Expansion Areas

These are strong ideas, but they should not displace the roadmap above unless user demand clearly pulls them forward.

- [ ] image templates with OCR-backed region replacement
- [ ] template marketplace or central registry
- [ ] collaborative/team template workflows
- [ ] cross-platform clipboard backends
- [ ] embedded preview server with hot reload
- [ ] advanced domain-specific templatization passes for finance, legal, or healthcare

---

## Definition of Done

For every milestone:

- [ ] `cargo test` passes
- [ ] `cargo clippy` is clean for the touched scope
- [ ] `cargo fmt` is clean
- [ ] new user-facing commands have help text and at least one integration test where practical
- [ ] changes to HTML generation or cleaning have fixture- or snapshot-based coverage where practical
- [ ] on-disk format changes are documented and, if needed, migrated safely

For roadmap completion through v1.0:

- [ ] clipli is a dependable macOS clipboard templating tool for both humans and automation workflows
- [ ] the project has a clear public contract for templates, CLI output, and error behavior
- [ ] experimental features are clearly labeled rather than mixed into the stable core
