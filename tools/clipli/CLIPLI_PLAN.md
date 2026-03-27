# clipli Development Plan

**Spec:** `CLIPLI_SPEC.md` v1.0.0-spec  
**Current crate version:** `0.3.0`
**Plan updated:** 2026-03-27

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

- [x] `inspect`, `read`, `write`, `capture`, `paste`, `list`, `show`, `edit`, `delete`, `versions`, `restore`, `lint`, `search`, `export`, `import`, `excel`, `excel-edit`, and `convert` commands (18 total)
- [x] pasteboard support for HTML, RTF, plain text, PNG, TIFF, and PDF payloads
- [x] HTML cleaning with target-aware CSS filtering for Excel, PowerPoint, Google Sheets, and generic HTML
- [x] Jinja2-compatible rendering with custom filters and HTML-to-plain-text conversion
- [x] heuristic templatization for dates, currency, percentages, emails, large numbers, quarters, and cell text
- [x] template storage under `~/.config/clipli/templates/`
- [x] rich CSV-to-Excel HTML generation and A1-style Excel cell editing

- [x] RTF-to-HTML conversion via `textutil` in `convert` and `capture` fallback
- [x] config cascade with all 6 fields wired end-to-end
- [x] `capture --preview` workflow
- [x] JSON error output with typed error codes across all error types
- [x] template versioning with snapshot, list, load, restore, and prune (max 20)
- [x] template linting with 5 checks (undefined vars, unused vars, duplicates, suspicious defaults, invalid identifiers)
- [x] full-text template search across name, description, tags, and content
- [x] template import/export as ZIP bundles with `manifest.json`
- [x] store durability with atomic writes (temp dir + rename) and `delete --keep-versions`

### Verified baseline

`cargo test` passes 197 tests with 0 failures and 0 warnings, including the unit and integration suites, with GUI-dependent pasteboard tests still ignored as expected.

### Completed so far

- [x] core clipboard I/O is implemented for the primary macOS pasteboard formats used by clipli
- [x] the HTML cleaning pipeline is implemented and covered by fixture-oriented tests
- [x] rendering, built-in templates, and HTML-to-plain-text fallback are implemented
- [x] heuristic templatization is implemented and round-trip tested
- [x] template capture, storage, listing, editing, showing, and deletion are implemented
- [x] Excel HTML generation and clipboard editing workflows are implemented beyond the original MVP scope
- [x] the current automated baseline is green across unit and integration tests, with only GUI-dependent clipboard tests ignored
- [x] **v0.2 complete:** RTF conversion via `textutil` (`src/rtf.rs`), config cascade with all 6 fields wired, `capture --preview`, JSON error output with `code()` on all error types and `--json` detection in `main()`, 186 tests passing
- [x] **v0.3 complete:** template versioning (snapshot/list/load/restore, prune to 20 max), template linting (`src/lint.rs`, 5 checks), full-text search, import/export ZIP bundles with `manifest.json`, store durability (atomic writes via temp dir + rename, `delete --keep-versions`), auto-snapshots on `edit`, 197 tests passing

### Highest-confidence gaps from the current implementation

- [x] ~~`convert --from rtf --to html` is still explicitly unimplemented~~ (done in v0.2)
- [x] ~~config is loaded, but defaults are not consistently honored across commands~~ (done in v0.2)
- [ ] agent templatization uses a stdin/stdout protocol only; clipli does not yet invoke an external agent command itself
- [x] ~~template storage has no versioning, rollback, locking, or import/export story~~ (done in v0.3)
- [x] ~~`capture` does not yet provide the preview workflow described in the spec~~ (done in v0.2)
- [ ] shell completions, richer diagnostics, and release/distribution work are still missing
- [ ] batch rendering workflows do not exist yet
- [ ] external agent command execution (beyond stdin/stdout protocol) is not yet supported
- [ ] `-v` / `-vv` debug logging is not yet available

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

## v0.2 — Core Completion and Correctness ✅ COMPLETE

**Goal:** Close the most important spec gaps and make the existing command set reliable enough for daily use.

### Primary deliverables

- [x] Implement `rtf -> html` conversion in `convert`
- [x] Make config defaults actually influence command behavior end-to-end
- [x] Add `capture --preview`
- [x] Tighten plain-text output behavior for `paste` and `convert`
- [x] Improve structured error output consistency
- [x] Expand regression coverage around missing and partial behaviors

### Detailed scope

#### 0.2.1 RTF conversion

- [x] Add an internal `rtf_to_html()` path used by `convert` and optionally by `capture` fallback workflows
- [x] Preserve at least bold, italic, underline, font family, font size, foreground color, paragraph breaks, and table-ish structures where possible
- [x] Define explicit failure behavior for unsupported RTF constructs instead of silently degrading
- [x] Add tests using realistic RTF fixtures from common macOS apps

**Acceptance:**

- [x] `clipli convert --from rtf --to html` works on representative RTF samples
- [x] `capture` can produce useful output when HTML is absent but RTF is present

#### 0.2.2 Config cascade cleanup

- [x] Audit every command that should honor config defaults
- [x] Ensure `defaults.font`, `defaults.font_size_pt`, `defaults.plain_text_strategy`, `clean.keep_classes`, `clean.target_app`, and `templatize.default_strategy` are applied consistently
- [x] Define clear precedence: CLI flags > command defaults from config > built-in defaults
- [x] Add tests for config-on / config-off behavior

**Acceptance:**

- [x] Changing config materially changes behavior in tested commands without requiring equivalent CLI flags

#### 0.2.3 Capture and paste polish

- [x] Add `capture --preview` by writing the cleaned or templatized output to a temp file and opening it
- [x] Audit `paste --plain-text auto|tab-delimited|none` so behavior is explicit and stable
- [x] Improve `show --open` and `paste --open` temp-file handling
- [x] Make validation errors more actionable for invalid names, invalid JSON, and missing template data

**Acceptance:**

- [x] preview flows work for capture, show, and paste
- [x] plain-text modes behave deterministically and are documented

#### 0.2.4 JSON and error surface hardening

- [x] Standardize error envelopes where `--json` is supported
- [x] Add command-level error codes where only string errors exist today
- [x] Reduce `Box<dyn std::error::Error>` escape hatches in command paths where typed errors are practical
- [x] Add tests for error JSON output, not just happy paths

**Acceptance:**

- [x] machine-readable consumers can reliably inspect failure codes across core commands

#### 0.2.5 Core regression coverage

- [x] Add tests for config usage
- [x] Add tests for `capture --preview` and preview temp-file generation where feasible
- [x] Add more conversion tests for malformed input and fallback flows
- [x] Add at least one real-world RTF fixture suite

### Risks

- [x] RTF fidelity may be materially worse than HTML fidelity; document known limits rather than overpromising
- [x] config cleanup can accidentally change existing defaults; preserve behavior where practical and call out intentional changes

### Exit criteria

- [x] The spec-promised core conversion and preview workflows are implemented
- [x] Config defaults behave consistently
- [x] JSON output is more uniform
- [x] The project remains green on `cargo test`

---

## v0.3 — Template Safety, Search, and Lifecycle ✅ COMPLETE

**Goal:** Make templates durable, discoverable, and safer to evolve over time.

### Primary deliverables

- [x] template versioning and rollback
- [x] template linting and validation improvements
- [x] content-based template search
- [x] import/export bundles
- [x] safer store writes and recovery paths

### Detailed scope

#### 0.3.1 Template versioning

- [x] Store historical snapshots under each template directory
- [x] Track enough metadata to show when a version was created and from what change path
- [x] Add commands such as:
  - [x] `clipli versions <NAME>`
  - [x] `clipli show <NAME> --version <ID>`
  - [x] `clipli restore <NAME> --version <ID>`
- [x] Decide whether versions are full copies or delta-free snapshots; favor simplicity first

**Acceptance:**

- [x] editing or recapturing a template no longer destroys prior working states

#### 0.3.2 Template linting

- [x] Add `clipli lint <NAME>`
- [x] Detect:
  - [x] undefined variables
  - [x] variables present in schema but unused in template
  - [x] duplicate variable names
  - [x] suspicious default values
  - [x] unbalanced Jinja markers and invalid identifiers
- [x] Surface warnings separately from hard failures

**Acceptance:**

- [x] users can validate templates before a bad paste flow or broken batch run

#### 0.3.3 Search and discovery

- [x] Add full-text search across template HTML, schema, and metadata
- [x] Support search by name, tags, source app, and content snippets
- [x] Add command:
  - [x] `clipli search <QUERY>`
- [x] Keep implementation simple at first, likely filesystem scan plus indexed metadata if needed later

**Acceptance:**

- [x] users with a non-trivial template library can reliably find the right template

#### 0.3.4 Import/export bundles

- [x] Export a template directory as a portable `.clipli` bundle
- [x] Import a bundle into the store with collision handling
- [x] Preserve `meta.json`, `schema.json`, template content, `original.html`, and `raw.html`
- [x] Consider optional manifest versioning for future compatibility

**Acceptance:**

- [x] a template can be transferred between machines without manual directory copying

#### 0.3.5 Store durability

- [x] Add atomic writes where practical
- [x] Add simple lock-file or temp-file strategy to reduce corruption risk
- [x] Ensure `force` overwrites are safe and recoverable
- [x] Decide behavior for malformed `meta.json` in partially corrupted template directories

### Risks

- [x] versioning and bundle design become de facto public formats; keep them simple and documented
- [x] search can drift into building a database too early; start with file-backed indexing only if needed

### Exit criteria

- [x] templates are versioned and recoverable
- [x] users can lint, search, export, and import templates
- [x] store writes are materially safer than they are today

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
- [x] RTF fallback is implemented and documented
- [x] config behavior is consistent and tested
- [x] template versioning, linting, and search exist
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

1. ~~`v0.2` core completion and correctness~~ ✅ COMPLETE
2. ~~`v0.3` template lifecycle and safety~~ ✅ COMPLETE
3. `v0.4` agent-native workflows ← **current**

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
