---
title: mdli — Product Requirements Document
status: final-draft
version: 0.3
owner: Brian
last_updated: 2026-04-27
---

# mdli — Markdown Operations CLI for Agents

## 1. TL;DR

`mdli` is a Rust-based CLI and library that treats Markdown as an editable document tree rather than a string. It exists to stop AI agents from generating ad-hoc Python, shell, or JavaScript that hand-builds `.md` files line by line.

Agents should declare document intent:

```bash
mdli section ensure report.md --id cashplus.analytics --path "4. Campaign & Product Analytics" --level 2 --after cashplus.touchpoints --write

mdli table replace report.md \
  --section cashplus.analytics \
  --name analytics-tickets \
  --columns Ticket=key,Summary=summary,Assignee=assignee,Status=status,Priority=priority \
  --from-rows /tmp/analytics.ndjson \
  --key Ticket \
  --write
```

`mdli` performs the edit safely, idempotently, and with stable selectors that survive renames, duplicate headings, and section reordering.

The product is deliberately scoped to Markdown editing primitives plus a small recipe/template layer. It is not a Jira client, not a data-normalization framework, not a report scheduler, and not a general-purpose query engine. Those capabilities compose through stdin/stdout and prepared row datasets, preferably NDJSON.

## 2. Background and motivating failure mode

Agents asked to maintain structured Markdown reports often produce code that:

- builds Markdown by appending strings;
- recreates sections that already exist;
- breaks on duplicate headings because selection is by substring or line number;
- re-implements table rendering, pipe escaping, link formatting, sort order, and truncation;
- conflates data extraction with document editing;
- cannot distinguish generated content from human-authored content;
- cannot dry-run structurally;
- cannot emit a reliable edit plan;
- produces large, noisy diffs for small semantic changes.

The resulting workflow is brittle and unreviewable. A 200-line script may regenerate a 30-line table. The agent burns tokens, the user cannot easily audit the edit, and the document drifts.

`mdli` turns this into document operations.

## 3. Users

### 3.1 Primary users

AI coding agents and assistant tools acting on behalf of analysts, product managers, engineers, and operators.

Examples:

- Claude Code
- Cursor agents
- GitHub Copilot CLI
- terminal-native agent tools
- internal automation agents

These users need deterministic, JSON-friendly tools with bounded blast radius.

### 3.2 Secondary users

Human authors of long-lived Markdown reports, runbooks, status docs, OKR rollups, knowledge-base pages, and project plans who want generated updates without losing manual edits.

### 3.3 Tertiary users

Tooling authors who want a Rust crate for AST-level Markdown editing.

`mdli-core` should be usable directly by sibling tools such as `docli`, `vaultli`, and future document-operation utilities.

## 4. Goals

1. Agents can edit Markdown via explicit document operations, not line-oriented string manipulation.
2. Every mutating operation is idempotent, dry-runnable, and capable of emitting machine-readable output.
3. Generated content and human-authored content coexist in the same file without silent overwrites.
4. Selectors survive renames, duplicates, and reorderings.
5. JSON/NDJSON-to-table creation is first-class.
6. Round-trip fidelity is specified precisely, including what `mdli` preserves and what it canonicalizes.
7. In-place writes are atomic and safe under common concurrent-edit scenarios.
8. The core library is reusable from other Rust tools.
9. The command surface is small enough for agents to learn and reliable enough for CI.
10. The project resists scope creep into data fetching, data normalization, or arbitrary computation.

## 5. Non-goals

`mdli` core will not include:

- built-in data-source connectors such as `mdli jira fetch`, `mdli github issues`, or `mdli linear issues`;
- a general-purpose data pipeline;
- filtering, grouping, joining, deduping, classification, or derived-field computation for source datasets;
- a full programming language in templates;
- arbitrary shell execution from recipes or templates;
- network access;
- a package/plugin system in v1;
- editor integrations in v1;
- prose-style linting;
- Word, PDF, HTML, AsciiDoc, MDX, or Pandoc output.

Data preparation happens upstream with tools such as `jq`, `miller`, `jira-cli`, shell scripts, or bespoke scripts. `mdli` consumes prepared row data and edits Markdown.

## 6. Relationship to adjacent tools

| Tool | Role | Boundary with `mdli` |
|---|---|---|
| `mdx` | Section-addressable Markdown/container format | Reuse selector grammar or AST primitives if they exist. Decision required before Phase 1 implementation. |
| `docli` | OOXML document editor | Different format, same philosophy: stable selectors, managed blocks, dry-run JSON, atomic writes. |
| `vaultli` | Knowledge-base CLI | May invoke `mdli` for Markdown note edits. `mdli-core` should be importable. |
| `jira-cli` | Jira fetch and normalization | Produces local NDJSON that `mdli apply` or `mdli table replace` consumes. |
| `jq` / `miller` | Data prep | Filter, group, classify, sort, join, and derive data before passing rows to `mdli`. |

## 7. Success metrics

1. An agent can regenerate the Cash Plus report end-to-end with one `mdli apply` call after upstream data prep.
2. Re-running any mutating command produces zero diff.
3. A run on a 5,000-line document with 40 tables completes in under 200 ms, excluding external data-file I/O.
4. The 95th-percentile edit produces a structural diff under 200 lines of unified output.
5. Agents in dry-run mode receive a JSON edit plan that is sufficient to decide whether to commit.
6. `mdli lint` catches duplicate headings, invalid tables, duplicate stable IDs, malformed managed blocks, and tampered managed content.
7. `mdli table replace --from-rows` produces byte-identical table output across repeated runs with the same input.
8. A fixture corpus of at least 100 Markdown edge cases round-trips under canonicalization with expected output.

## 8. Design principles

1. **Idempotency by default.** Re-running a successful mutation must not duplicate or drift content.
2. **Stable selectors over visible paths.** Hidden IDs are canonical. Path selectors are useful for first-touch and bootstrap only.
3. **Tables are data.** Agents pass rows. The renderer owns escaping, alignment, link formatting, truncation, missing values, and stable output.
4. **Generated blocks are explicit.** Generated content is fenced with begin/end markers and checksums. Human content outside generated blocks is sacred.
5. **No line-number arithmetic.** Edits are AST operations, not regex edits against body text.
6. **Dry-run is first-class.** Every mutating command can return a structured edit plan before writing.
7. **Ambiguity is an error.** `mdli` never guesses between duplicate selectors.
8. **Round-trip fidelity is a contract.** Whitespace, fences, links, tables, and frontmatter behavior are documented and stable.
9. **Library first, CLI thin.** Every CLI command wraps a `mdli-core` operation.
10. **Compose, do not bundle.** Data fetch, data normalization, and data classification happen upstream.
11. **No hidden execution.** Recipes and templates describe rendering only. They do not execute commands or fetch resources.
12. **Clean stdout for agents.** Machine-readable stdout is never mixed with diagnostics.

## 9. Product scope by release

### 9.1 MVP: document operations and tables

MVP includes:

- Markdown parsing and structural inspection;
- stable ID assignment and listing;
- section get, ensure, replace, delete, move, and rename;
- table list, get, replace, upsert, delete-row, sort, and format;
- JSON/NDJSON-to-table rendering;
- managed block list, get, replace, ensure, lock, and unlock;
- dry-run and JSON output;
- atomic writes;
- selector ambiguity handling;
- basic linting;
- fixture-driven round-trip tests.

### 9.2 Post-MVP

Post-MVP includes:

- templates;
- recipes;
- semantic diffs;
- validation schemas;
- plan/apply-plan workflow;
- patch application;
- Git integration;
- public Rust API stabilization.

## 10. Architecture

### 10.1 Workspace layout

```text
mdli/
├── Cargo.toml
├── crates/
│   ├── mdli-core/       # AST, selectors, sections, markers, edit primitives
│   ├── mdli-table/      # GFM table model, row input, render, upsert, sort
│   ├── mdli-template/   # minimal template renderer, no expressions
│   ├── mdli-recipe/     # recipe schema, validation, apply orchestration
│   ├── mdli-lint/       # validators and repair suggestions
│   ├── mdli-diff/       # semantic diff and edit-plan rendering
│   └── mdli-cli/        # clap entry point, JSON output, exit codes
├── tests/
│   ├── fixtures/
│   ├── golden/
│   └── fuzz/
└── docs/
    ├── adr/
    ├── recipe-philosophy.md
    ├── wire-format.md
    └── migrations/
```

### 10.2 Parser choice

Decision: use `comrak` as the primary parser for v1, behind an internal abstraction.

Rationale:

- `comrak` supports CommonMark and GitHub-Flavored Markdown features needed by `mdli`, including tables, autolinks, task lists, strikethrough, and footnotes.
- It exposes an editable AST suitable for document operations.
- `pulldown-cmark` is excellent for rendering but event-based; reconstructing an editable tree would duplicate work.
- `markdown-rs` has an attractive AST but a less mature round-trip serializer story.

The parser is accessed through a facade:

```rust
mdli_core::ast::Document
```

Downstream crates should not depend directly on `comrak` types.

### 10.3 Supported Markdown dialect

Supported:

- CommonMark;
- GitHub-Flavored Markdown tables;
- task lists;
- strikethrough;
- autolinks;
- footnotes where the parser supports them;
- YAML and TOML frontmatter.

Unsupported in v1:

- MDX JSX;
- Pandoc attributes;
- nested Markdown tables;
- multiline table cells as first-class table cells;
- arbitrary raw HTML parsing;
- AsciiDoc;
- reStructuredText.

Unsupported constructs are preserved where possible, but commands that would need to edit inside unsupported constructs must fail with a clear error.

## 11. Round-trip fidelity contract

`mdli` must specify what it preserves, what it canonicalizes, and when canonicalization can change.

### 11.1 Preserved by default

The following are preserved unless the user directly edits the affected structure:

- heading text, level, and trailing `#` style;
- code fence language and content, byte-for-byte inside the fence;
- inline HTML;
- list bullet style per list (`-`, `*`, `+`);
- ordered-list numbering style where possible;
- link reference definitions and their ordering;
- frontmatter byte-for-byte when frontmatter is not edited;
- blank-line counts between top-level blocks, except where a specific edit creates or removes a block;
- unknown `mdli` marker fields on round-trip;
- unknown higher-version markers on unrelated edits.

### 11.2 Canonicalized on write

The following are canonicalized predictably:

- generated table alignment rows use `|---|` style with one space of padding in rendered cells;
- trailing whitespace outside code fences is stripped;
- final newline is enforced;
- tabs in table cells become spaces;
- tabs in code fences are preserved;
- backslash hard breaks may be normalized to two trailing spaces outside code fences;
- `mdli` markers written by `mdli` use canonical field order.

### 11.3 Canonicalization versioning

Any change to canonicalization rules after v1.0 is breaking unless it is gated behind an explicit migration command.

Required for every canonicalization change:

- `MIGRATIONS.md` entry;
- fixture updates;
- before/after examples;
- compatibility impact statement;
- `mdli migrate` support if existing files need marker or format changes.

### 11.4 Zero-diff definition

“Zero diff” means zero diff after applying `mdli` canonicalization once.

The first run of `mdli fmt` or a mutating command may produce a canonicalization diff on an unmanaged legacy file. Subsequent identical runs must produce zero diff.

## 12. Data input contract

`mdli` consumes prepared row datasets for table rendering and recipes.

### 12.1 Row input formats

Primary format: NDJSON, one JSON object per line.

Convenience format: JSON array of objects.

Command flags:

```bash
--from-rows PATH
--rows-format auto|ndjson|json-array
```

Rules:

- default `--rows-format` is `auto`;
- auto-detection uses the first non-whitespace byte;
- `[` means JSON array;
- `{` means NDJSON;
- each row must be a JSON object;
- arrays of scalars are invalid;
- top-level objects that are not NDJSON lines are invalid;
- files must be UTF-8;
- empty row files are valid and produce an empty dataset.

The old name `--from-json` is not part of the v1 command surface because it obscures the NDJSON-first contract. A compatibility alias may be added later, but docs and examples should use `--from-rows`.

### 12.2 Projection into table columns

Tables map row fields to column labels.

Example:

```bash
--columns Ticket=key,Summary=summary,Assignee=assignee,Status=status,Priority=priority
```

If `=` is omitted, the column label is also the row field name:

```bash
--columns Ticket,Summary,Status
```

Field-path rules:

- simple dotted paths are allowed, e.g. `fields.status.name`;
- paths traverse JSON objects only;
- array indexing, wildcards, filters, and functions are not supported;
- keys containing literal dots are not addressable in v1 and should be prepared upstream;
- missing fields render according to `--missing`, defaulting to empty cells.

This projection is not a data pipeline. It is a table rendering convenience.

### 12.3 Stdin rules

Only one input stream may consume stdin in a single invocation.

Invalid:

```bash
cat report.md | mdli table replace - --from-rows - --section analytics
```

The document and the rows cannot both be read from standard input.

Valid alternatives:

```bash
cat report.md \
  | mdli table replace - \
      --section analytics \
      --from-rows /tmp/rows.ndjson \
      --emit document
```

or, where supported:

```bash
cat report.md \
  | mdli table replace - \
      --section analytics \
      --from-rows-fd 3 \
      --emit document \
      3</tmp/rows.ndjson
```

`--from-rows-fd` is optional in MVP. If omitted from MVP, docs must require a separate file path or process substitution for row input when the Markdown document is read from stdin.

## 13. Table rendering contract

Markdown tables are first-class document objects.

### 13.1 Table commands

```bash
mdli table list FILE [--json]
mdli table get FILE (--section SEL | --name NAME) [--json]
mdli table replace FILE --section SEL [--name NAME] --columns SPEC --from-rows PATH [--key COL] [--sort SPEC] [--write | --emit document | --json]
mdli table upsert FILE --name NAME --key COL (--row K=V ... | --from-rows PATH) [--write | --emit document | --json]
mdli table delete-row FILE --name NAME --key COL --value VALUE [--write | --emit document | --json]
mdli table sort FILE --name NAME --by SPEC [--write | --emit document | --json]
mdli table fmt FILE [--all | --name NAME] [--write | --emit document | --json]
```

There is no `--where EXPR` in v1. Row deletion is key/value based to avoid adding an expression language.

### 13.2 Named table marker

Tables may be named by a marker immediately preceding the table:

```md
<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->
| Ticket | Summary | Assignee | Status | Priority |
|---|---|---|---|---|
| CPMKTG-2904 | Automate weekly refreshes | Alex | In Progress | High |
```

Rules:

- `name` is required for table-level operations by name;
- `key` is optional but required for `upsert`;
- a table marker binds to the next Markdown table if only blank lines appear between them;
- if non-blank non-table content appears before the table, the marker is orphaned and `lint` errors.

### 13.3 Cell rendering

Default cell behavior:

| Input value | Default render |
|---|---|
| missing field | empty cell |
| `null` | empty cell |
| string | escaped Markdown table cell |
| number | JSON stringification |
| boolean | `true` or `false` |
| object | error |
| array | error |
| string containing newline | error |

Flags:

```bash
--missing empty|error
--on-rich-cell error|json|truncate|html
--escape-markdown
```

Definitions:

- rich cells are objects, arrays, or strings containing newlines;
- default `--on-rich-cell` is `error`;
- `json` renders compact JSON stringification with pipes escaped;
- `truncate` converts to a single line and truncates to the configured width;
- `html` converts newlines to `<br>` and escapes table delimiters;
- inline backticks are allowed and preserved;
- code blocks are rich cells and error by default;
- pipe characters are always escaped as `\|`;
- leading and trailing cell whitespace is trimmed unless `--preserve-cell-whitespace` is passed;
- Markdown inline formatting is preserved by default;
- `--escape-markdown` escapes Markdown-significant characters for plain-text tables.

### 13.4 Links

A table column can render as a Markdown link:

```bash
--link Ticket="https://vanguardim.atlassian.net/browse/{key}"
```

Rules:

- link patterns may reference row fields using `{field}` placeholders;
- placeholders use the same simple dotted path rules as column projection;
- missing placeholder fields are errors by default;
- link text is the rendered cell value before link wrapping;
- URLs are not fetched or validated in core.

### 13.5 Truncation

```bash
--truncate Summary=70
```

Rules:

- truncation is by Unicode scalar count, not byte count;
- default ellipsis is `…`;
- truncation happens before table width calculation;
- truncation never splits an escape sequence;
- code fences and multiline content remain rich cells and error unless rich-cell behavior allows conversion.

### 13.6 Sorting

```bash
--sort Priority:asc,Ticket:asc
```

Rules:

- sorting is stable;
- default direction is ascending;
- default comparison is lexical string comparison after rendering raw scalar values;
- numeric sort is supported only when declared with `--types COL=number`;
- custom domain ordering, such as Critical > High > Medium > Low, is out of scope for v1 and should be prepared upstream.

### 13.7 Duplicate keys

For `replace` with `--key` and for `upsert`, duplicate keys in incoming rows are an error by default.

```bash
--on-duplicate-key error|first|last
```

Default: `error`.

### 13.8 Empty datasets

Default: render a valid header-only table.

```md
| Ticket | Summary |
|---|---|
```

Optional placeholder:

```bash
--empty "No matching tickets."
```

If `--empty` is provided, `mdli` renders:

```md
_No matching tickets._
```

instead of a table body. The section/table operation metadata still records that the bound dataset was empty.

## 14. Selector model

### 14.1 Selector types

Selectors are accepted in these forms:

```bash
--id cashplus.analytics
--path "Current Cash Plus Epics & Stories > 4. Campaign & Product Analytics"
--section cashplus.analytics
--after cashplus.touchpoints
--before cashplus.other_epics
```

When a command argument is named `SEL`, the selector resolution order is:

1. stable ID;
2. exact path;
3. error if ambiguous or missing.

Line-number selectors are disallowed.

### 14.2 Stable ID grammar

Stable IDs must match:

```regex
[a-z][a-z0-9._-]*
```

Rules:

- IDs are globally unique within a document;
- IDs are case-sensitive;
- IDs are not URLs;
- IDs are not rendered visibly;
- IDs are not automatically changed when headings are renamed.

### 14.3 Stable ID marker

```md
<!-- mdli:id v=1 id=cashplus.analytics -->
## 4. Campaign & Product Analytics
```

Binding rules:

- an ID marker binds to the next heading;
- only blank lines may appear between marker and heading;
- if any non-blank non-heading content appears first, the marker is orphaned and `lint` errors;
- an ID marker inside a managed block is allowed but must agree with the managed block ID when it identifies the section heading for that block;
- multiple ID markers for the same heading are invalid.

### 14.4 Path selector grammar

Path selectors use visible heading text:

```text
H1 > H2 > H3
```

Rules:

- matching is case-sensitive;
- leading and trailing whitespace in each path segment is ignored;
- internal heading whitespace is normalized to a single space;
- literal `>` in heading text must be escaped as `\>`;
- duplicate matches are an error;
- a path may omit the H1 only if doing so is unambiguous;
- path selectors are for bootstrap and human convenience, not long-lived automation.

### 14.5 ID assignment

Commands:

```bash
mdli id list FILE [--json]
mdli id assign FILE --all [--write | --emit document | --json]
mdli id assign FILE --section PATH --id ID [--write | --emit document | --json]
mdli id assign FILE --section PATH --auto [--write | --emit document | --json]
```

Auto-generated IDs use:

1. slugified heading text;
2. lowercase ASCII;
3. non-alphanumeric runs collapsed to `-`;
4. leading section numbers removed where obvious;
5. suffix on collision;
6. `section-<short-hash>` if slugification produces an empty string.

Example:

```text
"4. Campaign & Product Analytics" -> campaign-product-analytics
Duplicate -> campaign-product-analytics-2
```

For business-critical generated reports, recipes should specify IDs explicitly rather than relying on auto-generated slugs.

## 15. Section operations

### 15.1 Commands

```bash
mdli section list FILE [--json]
mdli section get FILE (--id ID | --path PATH)
mdli section ensure FILE --id ID --path PATH --level N [--after SEL | --before SEL] [--write | --emit document | --json]
mdli section replace FILE (--id ID | --path PATH) --body-from-file BODY.md [--managed] [--write | --emit document | --json]
mdli section replace FILE (--id ID | --path PATH) --section-from-file SECTION.md [--write | --emit document | --json]
mdli section delete FILE (--id ID | --path PATH) [--write | --emit document | --json]
mdli section move FILE --id ID (--after SEL | --before SEL) [--write | --emit document | --json]
mdli section rename FILE --id ID --to TITLE [--write | --emit document | --json]
```

There is no unanchored `section append` command in v1. Unanchored append is not naturally idempotent. Use `block ensure` for repeatable inserts.

### 15.2 `section ensure`

Behavior:

1. resolve by ID if the ID exists;
2. otherwise resolve by path;
3. if neither exists, create the section;
4. assign the given stable ID if missing;
5. verify heading level matches the requested level;
6. insert according to `--after` or `--before` if creating;
7. fail if insertion position is ambiguous.

If the section already exists with matching ID, path, and level, the command is a no-op.

If the section exists with the same ID but a different visible heading, `mdli` treats the ID as canonical and emits a warning unless `--enforce-path` is passed.

### 15.3 `section replace` modes

Body-only replacement:

```bash
mdli section replace report.md --id cashplus.analytics --body-from-file body.md --write
```

This replaces the content below the selected heading until the next sibling or ancestor heading. It preserves the selected heading and stable ID marker.

Whole-section replacement:

```bash
mdli section replace report.md --id cashplus.analytics --section-from-file section.md --write
```

This replaces the heading plus body. The replacement must contain exactly one top-level heading at the selected level. The stable ID is preserved or injected.

Default replacement mode is not inferred. The user must choose `--body-from-file` or `--section-from-file`.

### 15.4 Managed section replacement

With `--managed`, body replacement creates or updates a managed block inside the section.

```md
<!-- mdli:id v=1 id=cashplus.analytics -->
## 4. Campaign & Product Analytics

Human-authored context can live here.

<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:... -->
Generated content.
<!-- mdli:end v=1 id=cashplus.analytics.generated -->
```

By default, `section replace --managed` replaces only the managed block. It does not remove human-authored content outside the block.

## 16. Managed blocks

### 16.1 Purpose

Managed blocks fence generated content so it can be safely regenerated without clobbering human-authored content.

### 16.2 Commands

```bash
mdli block list FILE [--json]
mdli block get FILE --id ID
mdli block ensure FILE --parent-section SEL --id ID (--body-from-file BODY.md | --text TEXT) [--position start|end|before:ID|after:ID] [--write | --emit document | --json]
mdli block replace FILE --id ID --body-from-file BODY.md [--on-modified MODE] [--write | --emit document | --json]
mdli block lock FILE --id ID [--write | --emit document | --json]
mdli block unlock FILE --id ID [--write | --emit document | --json]
```

`block ensure` is the idempotent replacement for unsafe append workflows.

### 16.3 Managed block marker

```md
<!-- mdli:begin v=1 id=cashplus.analytics.generated recipe="cashplus-report.yml@sha256:a3f9" checksum=sha256:7b2... locked=false -->
Generated content.
<!-- mdli:end v=1 id=cashplus.analytics.generated -->
```

Fields:

| Field | Required | Description |
|---|---:|---|
| `v` | yes | marker version |
| `id` | yes | globally unique block ID |
| `checksum` | yes for generated blocks | SHA-256 of block content between markers |
| `recipe` | no | source recipe plus content hash |
| `locked` | no | `true` or `false`; default false |

### 16.4 Conflict policy

When the on-disk checksum does not match the recorded checksum, the block has been modified outside `mdli`.

```bash
--on-modified fail|force|three-way
```

Default: `fail`.

Behavior:

- `fail`: exit non-zero, emit conflict metadata, write nothing;
- `force`: overwrite and emit a warning;
- `three-way`: write a `.mdli.conflict` artifact and exit non-zero.

This policy is per invocation and never silent.

### 16.5 Lock semantics

A locked block cannot be modified by:

- `block replace`;
- `block ensure`;
- `section replace --managed`;
- `apply`;
- `patch`;
- `apply-plan`.

unless `--force-locked` is passed.

Dry-run reports attempted locked edits as:

```json
{
  "code": "E_BLOCK_LOCKED",
  "block_id": "cashplus.analytics.generated"
}
```

Lock state is represented on the begin marker:

```md
<!-- mdli:begin v=1 id=foo checksum=sha256:... locked=true -->
```

## 17. Wire format and versioning

`mdli` markers are persistent on-disk wire format. They are not private implementation details. They will live in source-controlled documents indefinitely.

### 17.1 Marker types

Stable ID:

```md
<!-- mdli:id v=1 id=cashplus.analytics -->
```

Managed block:

```md
<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:7b2... -->
...
<!-- mdli:end v=1 id=cashplus.analytics.generated -->
```

Named table:

```md
<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->
```

### 17.2 Marker grammar

Rules:

- `v=1` is required on every marker written by `mdli`;
- markers are single-line HTML comments;
- multi-line markers are parse errors;
- field order is canonical: `v` first, primary identifier second (`id` or `name`), remaining fields alphabetical;
- field values may be unquoted only if they match `[A-Za-z0-9._@:/+\-]+`;
- all other values must be double-quoted;
- quoted strings support `\"` and `\\` escaping;
- values containing `-->` are illegal;
- unknown fields are preserved on read and unrelated writes;
- whitespace between fields is exactly one space when `mdli` writes a marker.

### 17.3 Compatibility policy

A vN reader must read vM markers for all M <= N.

A vN writer writes only vN markers when creating new structures.

When round-tripping an unrelated higher-version marker, `mdli` preserves the marker and refuses to modify the associated structure unless explicitly told to migrate or ignore newer fields.

Forward-version behavior:

- default: accept newer markers, preserve unknown fields, refuse to modify the associated structure;
- `--strict-version`: refuse to read newer markers;
- `--ignore-newer-fields`: modify anyway and drop unknown fields; available only to `mdli migrate`.

Breaking changes to v1 markers are forbidden after v1.0. New behavior ships as v2 markers.

## 18. Frontmatter

### 18.1 Supported dialects

MVP supports:

- YAML frontmatter delimited by `---`;
- TOML frontmatter delimited by `+++`.

JSON frontmatter is out of scope for MVP unless discovered to be required during fixture collection.

### 18.2 Commands

```bash
mdli frontmatter get FILE [--key KEY] [--json]
mdli frontmatter set FILE KEY VALUE [--write | --emit document | --json]
mdli frontmatter delete FILE KEY [--write | --emit document | --json]
```

### 18.3 Fidelity

If frontmatter is not edited, it is preserved byte-for-byte.

If frontmatter is edited, only the frontmatter block is canonicalized. Body content remains subject to the normal fidelity contract.

Frontmatter edits preserve key order where possible. If a serializer cannot preserve formatting exactly, the command must state that in dry-run output before writing.

## 19. Streaming, stdout, and write behavior

### 19.1 Universal flags

```bash
--write                  # atomic in-place write
--emit plan|document|diff|json
--json                   # shorthand for --emit json
--preimage-hash HASH     # refuse if input bytes do not match
--require-clean-git      # refuse to write if Git working tree is dirty for target file
--quiet                  # suppress non-error diagnostics
```

### 19.2 Default behavior

Read-only commands output their requested payload to stdout.

Mutating commands default to dry-run plan output. They do not write and do not emit transformed Markdown unless explicitly requested.

To stream transformed Markdown, pass:

```bash
--emit document
```

To write to a real file, pass:

```bash
--write
```

To get machine-readable output, pass:

```bash
--json
```

`--write` and `--emit document` are mutually exclusive.

### 19.3 Output streams

- stdout carries the primary product: payload, document, diff, or JSON;
- stderr carries diagnostics, warnings, and one-line summaries;
- `--json` never mixes diagnostics with stdout JSON;
- on failure, `mdli` emits no partial transformed document to stdout.

### 19.4 File argument conventions

`FILE` may be `-` to mean stdin for reads.

For mutating commands with `FILE=-`, the transformed document can be emitted with `--emit document`.

Example:

```bash
cat report.md \
  | mdli section ensure - \
      --id cashplus.analytics \
      --path "4. Campaign & Product Analytics" \
      --level 2 \
      --emit document \
  | mdli table replace - \
      --section cashplus.analytics \
      --name analytics-tickets \
      --columns Ticket=key,Summary=summary,Status=status \
      --from-rows /tmp/analytics.ndjson \
      --key Ticket \
      --emit document \
  | mdli fmt - --tables --emit document \
  > report.md.new
```

### 19.5 Atomic writes

When `--write` targets a real path:

1. read source bytes;
2. validate `--preimage-hash` if provided;
3. take advisory file lock where supported;
4. render new content to a temp file in the same directory;
5. preserve file permissions;
6. fsync temp file where supported;
7. atomically rename over the original path;
8. best-effort fsync parent directory;
9. release lock.

Temp-file pattern:

```text
FILE.mdli-tmp.<pid>.<random>
```

If write fails, the original file remains unchanged.

### 19.6 Symlinks and permissions

Default behavior for symlinks: follow symlink and write to target path.

Future option:

```bash
--no-follow-symlinks
```

Permissions:

- preserve mode bits of the existing file;
- preserve owner/group where supported by OS and permissions;
- fail clearly on read-only targets.

### 19.7 Encoding and line endings

- input must be UTF-8;
- invalid UTF-8 is `E_INVALID_UTF8`;
- existing LF or CRLF line endings are detected;
- line endings are preserved when possible;
- new generated content uses the document’s dominant line ending;
- UTF-8 BOM is preserved if present.

### 19.8 Concurrency model

`--preimage-hash` protects against stale reads.

Advisory file locking protects against local concurrent writes.

Rules:

- `--preimage-hash` is computed against exact input bytes;
- for stdin pipelines, the hash refers to the input stream;
- for real files, `mdli` takes an advisory exclusive lock during write;
- pipelines do not lock;
- either mechanism is sufficient for common cases;
- both together are recommended for agents.

## 20. Error model and exit codes

### 20.1 Error output

Every error has a stable code.

Example:

```json
{
  "schema": "mdli/output/v1",
  "ok": false,
  "error": {
    "code": "E_AMBIGUOUS_SELECTOR",
    "message": "Path matched more than one section.",
    "matches": [
      {"path": "Report > Analytics", "line": 42, "id": null},
      {"path": "Appendix > Analytics", "line": 155, "id": null}
    ]
  }
}
```

### 20.2 Exit codes

| Code | Meaning |
|---:|---|
| 0 | success |
| 1 | user error |
| 2 | document invariant violation |
| 3 | I/O error |
| 4 | stale preimage or concurrent edit |
| 64 | internal bug |

### 20.3 Required error codes

| Code | Meaning |
|---|---|
| `E_AMBIGUOUS_SELECTOR` | selector matched multiple nodes |
| `E_SELECTOR_NOT_FOUND` | selector matched nothing |
| `E_DUPLICATE_ID` | stable ID appears more than once |
| `E_ORPHAN_MARKER` | marker does not bind to expected structure |
| `E_BLOCK_MODIFIED` | checksum mismatch on managed block |
| `E_BLOCK_LOCKED` | attempted edit of locked block |
| `E_NEWER_FORMAT` | attempted edit of newer marker version |
| `E_TABLE_INVALID` | malformed Markdown table |
| `E_TABLE_KEY_MISSING` | key column missing |
| `E_TABLE_DUPLICATE_KEY` | duplicate key in rows |
| `E_ROW_INPUT_INVALID` | invalid NDJSON or JSON array |
| `E_RICH_CELL` | rich cell encountered with default policy |
| `E_INVALID_UTF8` | file is not valid UTF-8 |
| `E_STALE_PREIMAGE` | preimage hash mismatch |
| `E_WRITE_FAILED` | atomic write failed |
| `E_TEMPLATE_PARSE` | invalid template syntax |
| `E_TEMPLATE_MISSING_DATASET` | template references missing dataset |
| `E_RECIPE_INVALID` | recipe schema invalid |

## 21. JSON output contract

Every command supports `--json`.

Top-level success shape:

```json
{
  "schema": "mdli/output/v1",
  "ok": true,
  "result": {}
}
```

Top-level error shape:

```json
{
  "schema": "mdli/output/v1",
  "ok": false,
  "error": {
    "code": "E_SELECTOR_NOT_FOUND",
    "message": "No section matched selector cashplus.analytics."
  }
}
```

### 21.1 Edit summary shape

Mutating commands return:

```json
{
  "schema": "mdli/output/v1",
  "ok": true,
  "result": {
    "changed": true,
    "preimage_hash": "sha256:...",
    "postimage_hash": "sha256:...",
    "ops": [
      {
        "op": "replace_table",
        "table": "analytics-tickets",
        "rows_before": 10,
        "rows_after": 12,
        "rows_added": 2,
        "rows_removed": 0,
        "rows_updated": 4
      }
    ],
    "warnings": []
  }
}
```

### 21.2 Schema versioning

The output schema version is independent of CLI version.

Rules:

- additive fields are allowed in minor CLI versions;
- field removals require a schema version bump;
- agents should ignore unknown fields;
- `--json --schema-version mdli/output/v1` may be added if multiple schemas coexist.

## 22. Linting and validation

### 22.1 Lint commands

```bash
mdli lint FILE [--rules RULES] [--json]
mdli lint FILE --fix safe [--write | --emit document | --json]
```

### 22.2 Default lint rules

| Rule | Default severity | Description |
|---|---|---|
| `no-duplicate-headings` | warn | same visible heading path appears more than once |
| `valid-tables` | error | malformed tables, mismatched column counts |
| `managed-blocks-balanced` | error | orphan begin/end markers |
| `managed-blocks-checksum` | error | managed block content checksum mismatch |
| `heading-hierarchy` | warn | skipped heading levels, e.g. H1 to H3 |
| `unique-stable-ids` | error | duplicate IDs |
| `stable-id-binding` | error | ID marker does not bind to heading |
| `table-marker-binding` | error | table marker does not bind to table |
| `link-syntax` | warn | malformed Markdown links |
| `wire-format` | error | malformed `mdli` marker |
| `newer-marker-version` | warn | marker version newer than reader |

### 22.3 Validation schemas

Post-MVP:

```bash
mdli validate FILE --schema report.schema.yml [--json]
```

Schema example:

```yaml
schema: mdli/validation/v1
required_sections:
  - id: cashplus.okr
  - id: cashplus.dashboard
  - id: cashplus.analytics
required_tables:
  - name: analytics-tickets
    columns: [Ticket, Summary, Assignee, Status, Priority]
    key: Ticket
managed_blocks:
  - id: cashplus.analytics.generated
    locked: false
```

Validation is about document structure and mdli-managed invariants, not prose style.

## 23. Templates

Templates are Markdown files with a minimal substitution language.

### 23.1 Commands

```bash
mdli template render TEMPLATE --data NAME=PATH ... [--json]
mdli section render FILE --id ID --template TEMPLATE --data NAME=PATH ... [--write | --emit document | --json]
```

### 23.2 Helpers

`value` helper:

```md
**Last updated:** {{ value last_updated }}
```

`table` helper:

```md
{{ table tickets
   columns=["Ticket=key", "Summary=summary", "Assignee=assignee", "Status=status", "Priority=priority"]
   key="Ticket"
   sort=["Ticket:asc"]
   truncate={"Summary": 70}
   link={"Ticket": "https://vanguardim.atlassian.net/browse/{key}"}
   empty="No matching tickets."
}}
```

### 23.3 Template restrictions

Templates do not support:

- loops;
- arbitrary conditionals;
- expressions;
- function calls beyond built-in helpers;
- file reads;
- shell execution;
- network access.

Presence checks are allowed only for rendering optional slots:

```md
{{ if_present summary_note }}
{{ value summary_note }}
{{ end }}
```

No other conditionals are allowed in v1.

### 23.4 Template error behavior

| Error | Code |
|---|---|
| missing dataset | `E_TEMPLATE_MISSING_DATASET` |
| unknown helper | `E_TEMPLATE_UNKNOWN_HELPER` |
| invalid helper syntax | `E_TEMPLATE_PARSE` |
| missing value | `E_TEMPLATE_MISSING_VALUE` |
| missing table column | `E_TABLE_MISSING_COLUMN` unless `missing=empty` |
| unused binding | warning |

## 24. Recipes

Recipes bind prepared datasets to document slots. They do not fetch, filter, group, join, classify, or derive data.

### 24.1 Command surface

```bash
mdli recipe validate RECIPE [--json]
mdli apply FILE --recipe RECIPE --data NAME=PATH ... [--write | --emit document | --json]
mdli build --recipe RECIPE --data NAME=PATH ... --out FILE [--json]
```

### 24.2 Recipe schema

```yaml
schema: mdli/recipe/v1
title: "Current Cash Plus Epics & Stories"

settings:
  on_modified: fail
  on_missing_dataset: error
  generated_block_suffix: generated

datasets:
  stories: { from: data, name: stories }
  strategic: { from: data, name: strategic }
  refreshes: { from: data, name: refreshes }
  analytics: { from: data, name: analytics }

sections:
  - id: cashplus.okr
    path: "1. OKR & KR Reporting"
    level: 2
    after: null
    template: templates/okr.mdli

  - id: cashplus.dashboard
    path: "2. Cash Plus Dashboard"
    level: 2
    after: cashplus.okr
    template: templates/dashboard.mdli
    bindings:
      strategic_table: strategic
      refreshes_table: refreshes

  - id: cashplus.analytics
    path: "4. Campaign & Product Analytics"
    level: 2
    after: cashplus.touchpoints
    before: cashplus.other_epics
    template: templates/analytics.mdli
    bindings:
      tickets_table: analytics
```

### 24.3 Apply semantics

For each recipe section, in order:

1. validate recipe schema;
2. resolve all dataset bindings;
3. locate section by stable ID;
4. if not found, locate by path;
5. if still not found, create the section using `level`, `after`, and `before`;
6. assign stable ID if missing;
7. render the template with bound datasets;
8. create or replace the section’s managed generated block;
9. preserve human-authored content outside the managed block;
10. validate managed block checksum;
11. emit edit-plan metadata.

If neither `after` nor `before` can be resolved for a missing section:

- if the document has a single H1, append as the last child of that H1;
- otherwise fail with `E_INSERTION_POSITION_REQUIRED`.

### 24.4 Recipe provenance

`apply` records recipe provenance in generated blocks:

```md
<!-- mdli:begin v=1 id=cashplus.analytics.generated recipe="cashplus-report.yml@sha256:a3f9" checksum=sha256:... -->
```

The recipe hash is the SHA-256 of the recipe file content. Template hashes may be added post-MVP.

### 24.5 Build semantics

`build` creates a new Markdown file from a recipe and datasets.

Rules:

- fails if output file exists unless `--overwrite` is passed;
- writes all recipe sections in order;
- emits frontmatter if specified;
- creates stable ID markers and managed blocks;
- uses the same template rendering semantics as `apply`.

## 25. Edit plans and patching

### 25.1 Plan command

```bash
mdli plan FILE --recipe RECIPE --data NAME=PATH ... --json
```

Output:

```json
{
  "schema": "mdli/output/v1",
  "ok": true,
  "result": {
    "preimage_hash": "sha256:...",
    "ops": [
      {
        "op": "ensure_section",
        "id": "cashplus.analytics",
        "path": "4. Campaign & Product Analytics",
        "level": 2,
        "after_id": "cashplus.touchpoints"
      },
      {
        "op": "replace_block",
        "id": "cashplus.analytics.generated",
        "checksum_before": "sha256:...",
        "checksum_after": "sha256:...",
        "rendered_lines": 47
      }
    ],
    "sections_changed": ["cashplus.analytics"],
    "tables_changed": 1,
    "rows_added": 12,
    "rows_removed": 3,
    "warnings": []
  }
}
```

### 25.2 Apply-plan command

```bash
mdli apply-plan FILE --plan plan.json --write
```

Rules:

- refuses if `preimage_hash` does not match;
- refuses if any referenced selector changed;
- applies atomically;
- either every operation succeeds or none do.

### 25.3 Patch command

```bash
mdli patch FILE --edits edits.json [--write | --emit document | --json]
```

Patch example:

```json
[
  {
    "op": "ensure_section",
    "id": "cashplus.analytics",
    "path": "4. Campaign & Product Analytics",
    "level": 2,
    "after": "cashplus.touchpoints"
  },
  {
    "op": "replace_table",
    "section_id": "cashplus.analytics",
    "name": "analytics-tickets",
    "columns": ["Ticket=key", "Summary=summary", "Status=status"],
    "rows_from": "/tmp/analytics.ndjson",
    "key": "Ticket"
  }
]
```

Patches are atomic.

## 26. Semantic diff

Post-MVP command:

```bash
mdli diff FILE --against REF --semantic [--json]
```

Semantic diff reports:

- sections added, removed, renamed, or moved;
- stable IDs added or changed;
- managed blocks changed;
- tables added or removed;
- table schema changes;
- table rows added, removed, or updated by key;
- checksum conflicts;
- locked-block edit attempts.

Human-readable example:

```text
Section changed: cashplus.analytics
  Table changed: analytics-tickets
    Columns unchanged
    Rows added: 3
    Rows removed: 1
    Rows updated: 4
```

## 27. Agent ergonomics

### 27.1 Bounded context

```bash
mdli context FILE --id ID --max-tokens 2000 [--json]
```

Returns:

- selected section content;
- breadcrumb path;
- immediate sibling headings;
- child heading summary;
- stable IDs;
- managed-block metadata;
- byte and line ranges.

This lets agents avoid reading a 10,000-line document to edit one section.

### 27.2 Anti-footgun behavior

Required safeguards:

- mutators do not write unless `--write` is passed;
- streaming transformed documents requires explicit `--emit document`;
- ambiguous selectors error;
- operations affecting more than one section require `--allow-multiple`;
- large diffs over a configurable threshold warn or fail unless `--force-large-diff` is passed;
- locked blocks fail by default;
- modified managed blocks fail by default;
- newer marker versions are not modified by default;
- row duplicate keys error by default.

### 27.3 Recommended agent workflow

```bash
mdli inspect report.md --json
mdli plan report.md --recipe report.yml --data tickets=tickets.ndjson --json > plan.json
mdli apply-plan report.md --plan plan.json --write --require-clean-git
```

## 28. Security and privacy requirements

`mdli` is designed for agent use, so it must not create hidden execution or data-exfiltration paths.

Requirements:

- core performs no network access;
- recipes cannot execute shell commands;
- templates cannot execute shell commands;
- templates cannot read arbitrary files;
- datasets are provided explicitly via `--data` or `--from-rows`;
- recipe-relative template paths are resolved relative to the recipe file;
- paths using `../` outside the recipe directory are rejected unless `--allow-outside-recipe-dir` is passed;
- symlink behavior is explicit and documented;
- no telemetry is collected;
- HTML is preserved as text and never interpreted by `mdli`;
- managed block IDs, table names, and marker fields are parsed as data, never executed;
- environment variables are not interpolated in recipes by default;
- secrets in input files are not logged;
- error messages must avoid dumping entire row payloads unless `--debug` is passed.

## 29. Distribution and versioning

### 29.1 Installation targets

Supported installation methods:

- GitHub Releases binaries;
- `cargo install mdli-cli`;
- Homebrew tap after v0.5;
- internal package registries as needed.

Supported platforms:

- macOS arm64;
- macOS x64;
- Linux x64;
- Windows x64.

### 29.2 Versioning

- CLI follows semver.
- `mdli-core` follows semver once v1.0 is released.
- JSON output schema is independently versioned.
- Recipe schema is versioned as `mdli/recipe/v1`.
- Marker wire format is versioned as `v=1` in comments.
- Canonicalization changes after v1.0 require migration documentation.

## 30. Testing strategy

### 30.1 Fixture corpus

The fixture corpus must include:

- duplicate headings;
- headings with escaped `>` characters;
- headings with Unicode text;
- nested sections;
- empty documents;
- documents with no H1;
- code fences containing Markdown-looking headings and tables;
- malformed tables;
- valid tables with pipes in cells;
- nulls, booleans, numbers, objects, arrays, and newlines in row input;
- managed block tampering;
- locked blocks;
- orphan markers;
- newer-version markers;
- inline HTML;
- YAML frontmatter;
- TOML frontmatter;
- CRLF files;
- UTF-8 BOM files;
- large documents over 50,000 lines;
- Cash Plus report golden fixtures.

### 30.2 Test types

Required:

- unit tests for selector parsing;
- unit tests for marker parsing;
- unit tests for table rendering;
- property tests for idempotency;
- golden tests for formatting and diffs;
- fuzz tests for malformed Markdown;
- concurrency tests for preimage hash and locking;
- CLI snapshot tests for JSON output;
- round-trip tests for fidelity.

### 30.3 Acceptance gates

No release can ship unless:

- all fixture files round-trip to expected output;
- idempotency tests pass for all mutating commands;
- JSON schema snapshots are reviewed;
- marker version compatibility tests pass;
- table renderer tests cover all cell types;
- atomic write tests pass on supported platforms where feasible.

## 31. Roadmap

| Phase | Scope | Target | Acceptance |
|---:|---|---|---|
| 0 | Reconcile with `mdx`; parser benchmark; fixture corpus; ADRs | Pre-work | Decision docs in `docs/adr/` |
| 1 | AST core: inspect, tree, context, sections, stable IDs, formatting, atomic writes | MVP | idempotency suite passes; Cash Plus fixture round-trips |
| 2 | Tables: JSON/NDJSON-to-table, named tables, upsert, sort, formatting, rendering contract | MVP | table output byte-identical across repeated runs |
| 3 | Managed blocks: checksums, conflict policy, lock/unlock, wire format | MVP | conflict modes pass positive/negative tests |
| 4 | Lint and validation: rule set, JSON output, safe fixes | v0.4 | every rule has positive and negative fixtures |
| 5 | Templates: table/value helpers, no expressions | v0.5 | Cash Plus report renders from templates |
| 6 | Recipes: schema, validate, apply, build | v0.6 | single `mdli apply` regenerates Cash Plus report |
| 7 | Agent mode: plan, apply-plan, patch, semantic diff, bounded context | v0.7 | two-step plan/apply works under concurrent edit fuzz |
| 8 | Git integration: require-clean-git, snapshot, PR-style semantic diffs | v0.8 | review readability bar met |
| 9 | Library stabilization: public `mdli-core` API | v1.0 | semver commitment; crates published |

Explicitly not on this roadmap:

- data normalization commands;
- Jira/GitHub/Linear connectors;
- arbitrary recipe logic;
- package/plugin system;
- editor integrations.

These can be separate tools later, such as `mdli-jira` or `mdli-data`, if real workflows require them.

## 32. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Round-trip drift | User files change unexpectedly | fixture corpus, golden tests, canonicalization contract |
| Selector ambiguity | Agent edits wrong section | stable IDs, ambiguity errors, `id assign` workflow |
| Recipe scope creep | Product becomes a data pipeline | recipe philosophy doc; upstream data prep boundary |
| Table rendering edge cases | Broken Markdown tables | explicit cell contract and tests |
| Marker format regret | Future versions break old docs | versioned wire format and migration policy |
| Large-file performance | Slow agent workflows | benchmarks, lazy context extraction, streaming row input |
| GFM parser drift | GitHub rendering differs | pin parser version, fixture tests against supported subset |
| Concurrent edits | Lost updates | preimage hash, advisory lock, atomic writes |
| Agent misuse | Large unintended edits | dry-run defaults, large-diff warnings, locked blocks |
| Security surprises | Recipes/templates become execution vectors | no shell, no network, explicit paths, no telemetry |

## 33. Remaining open decisions

The final implementation plan should close these before Phase 1 begins:

1. **`mdx` reconciliation.** Is `mdx` a container format, an editor, or a lower-level AST library? If it already provides section addressing or parsing primitives, decide whether `mdli` depends on it.
2. **Frontmatter JSON support.** YAML and TOML are in MVP. Decide whether JSON frontmatter is common enough to support before v1.
3. **`--from-rows-fd` support.** Decide whether file-descriptor row input is worth MVP complexity or whether separate row files are sufficient.
4. **Windows file locking semantics.** Define exact implementation equivalent for advisory locking on Windows.
5. **Safe lint fixes.** Decide which lint rules can auto-fix without risking content loss.

## 34. Appendix A — Cash Plus workflow after mdli

### 34.1 Upstream data prep

```bash
jira-cli fetch --board 25000 --project CPMKTG \
  --fields key,summary,status,priority,assignee,parent,issuetype,description,updated \
  | jq -c '.[]' > /tmp/cp_raw.ndjson

jq -c 'select(.issuetype == "Story" and (.summary | test("dashboard|Cash Plus"; "i")))' \
  /tmp/cp_raw.ndjson > /tmp/strategic.ndjson

jq -c 'select(.issuetype == "Story" and (.summary | test("monitoring|refresh|mid-month"; "i")))' \
  /tmp/cp_raw.ndjson > /tmp/refreshes.ndjson

jq -c 'select(.issuetype == "Story" and (.summary | test("campaign|analytics|new to vanguard|n2v"; "i")))' \
  /tmp/cp_raw.ndjson > /tmp/analytics.ndjson
```

### 34.2 One-shot recipe application

```bash
mdli apply cash_plus.md \
  --recipe cashplus-report.yml \
  --data stories=/tmp/cp_raw.ndjson \
  --data strategic=/tmp/strategic.ndjson \
  --data refreshes=/tmp/refreshes.ndjson \
  --data analytics=/tmp/analytics.ndjson \
  --json > /tmp/plan.json

mdli apply-plan cash_plus.md \
  --plan /tmp/plan.json \
  --write \
  --require-clean-git
```

### 34.3 Direct table replacement without a recipe

```bash
mdli table replace cash_plus.md \
  --section cashplus.analytics \
  --name analytics-tickets \
  --columns Ticket=key,Summary=summary,Assignee=assignee,Status=status,Priority=priority \
  --from-rows /tmp/analytics.ndjson \
  --key Ticket \
  --link Ticket="https://vanguardim.atlassian.net/browse/{key}" \
  --truncate Summary=70 \
  --write
```

### 34.4 Expected generated table

Input row:

```json
{"key":"CPMKTG-2904","summary":"Automate weekly dashboard refreshes","assignee":"Alex Kim","status":"In Progress","priority":"High"}
```

Output:

```md
<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->
| Ticket | Summary | Assignee | Status | Priority |
|---|---|---|---|---|
| [CPMKTG-2904](https://vanguardim.atlassian.net/browse/CPMKTG-2904) | Automate weekly dashboard refreshes | Alex Kim | In Progress | High |
```

The agent no longer hand-builds Markdown, manually escapes pipes, or risks duplicate sections.

## 35. Appendix B — Example recipe and template

### 35.1 Recipe

```yaml
schema: mdli/recipe/v1
title: "Current Cash Plus Epics & Stories"

frontmatter:
  title: "Current Cash Plus Epics & Stories"
  source: "CPMKTG board #25000"

sections:
  - id: cashplus.dashboard
    path: "2. Cash Plus Dashboard"
    level: 2
    after: cashplus.okr
    template: templates/dashboard.mdli
    bindings:
      strategic: strategic
      refreshes: refreshes

  - id: cashplus.analytics
    path: "4. Campaign & Product Analytics"
    level: 2
    after: cashplus.touchpoints
    before: cashplus.other_epics
    template: templates/analytics.mdli
    bindings:
      tickets: analytics
```

### 35.2 Template

```md
The Campaign & Product Analytics workstream covers ad-hoc analysis, new-to-Vanguard studies, and campaign-specific reporting.

{{ table tickets
   columns=["Ticket=key", "Summary=summary", "Assignee=assignee", "Status=status", "Priority=priority"]
   key="Ticket"
   link={"Ticket":"https://vanguardim.atlassian.net/browse/{key}"}
   truncate={"Summary":70}
   empty="No matching analytics tickets."
}}
```

## 36. Appendix C — Command appendix

| Command | Phase | Mutates | JSON | Notes |
|---|---:|---:|---:|---|
| `inspect` | 1 | no | yes | structure summary |
| `tree` | 1 | no | yes | heading tree |
| `context` | 7 | no | yes | bounded section context |
| `id list` | 1 | no | yes | stable IDs |
| `id assign` | 1 | yes | yes | assign stable markers |
| `section list` | 1 | no | yes | section inventory |
| `section get` | 1 | no | yes | selected section body |
| `section ensure` | 1 | yes | yes | idempotent create/verify |
| `section replace` | 1 | yes | yes | body or whole-section modes |
| `section delete` | 1 | yes | yes | remove selected section |
| `section move` | 1 | yes | yes | reposition by selector |
| `section rename` | 1 | yes | yes | visible heading only |
| `table list` | 2 | no | yes | named and unnamed tables |
| `table get` | 2 | no | yes | table content/metadata |
| `table replace` | 2 | yes | yes | JSON/NDJSON-to-table |
| `table upsert` | 2 | yes | yes | key-based updates |
| `table delete-row` | 2 | yes | yes | key/value only |
| `table sort` | 2 | yes | yes | stable table sorting |
| `table fmt` | 2 | yes | yes | canonical table formatting |
| `block list` | 3 | no | yes | managed blocks |
| `block get` | 3 | no | yes | block content |
| `block ensure` | 3 | yes | yes | idempotent generated insert |
| `block replace` | 3 | yes | yes | checksum-aware replacement |
| `block lock` | 3 | yes | yes | prevent generated overwrite |
| `block unlock` | 3 | yes | yes | allow generated overwrite |
| `frontmatter get` | 1 | no | yes | frontmatter read |
| `frontmatter set` | 1 | yes | yes | frontmatter edit |
| `frontmatter delete` | 1 | yes | yes | frontmatter edit |
| `lint` | 4 | no | yes | structural lint |
| `validate` | 4 | no | yes | schema validation |
| `template render` | 5 | no | yes | render template to stdout |
| `section render` | 5 | yes | yes | render template into section |
| `recipe validate` | 6 | no | yes | validate recipe |
| `apply` | 6 | yes | yes | recipe-driven update |
| `build` | 6 | yes | yes | create doc from recipe |
| `plan` | 7 | no | yes | create edit plan |
| `apply-plan` | 7 | yes | yes | apply reviewed plan |
| `patch` | 7 | yes | yes | atomic JSON patch |
| `diff` | 8 | no | yes | semantic or textual diff |

