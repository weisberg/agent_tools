---
name: mdli
description: Use mdli to read, edit, and regenerate structured Markdown documents as an AST. It owns sections, tables, frontmatter, managed blocks, and recipe-driven apply for agent workflows that would otherwise hand-build .md files.
---

# mdli Skill

Use `mdli` whenever an agent needs to **modify** Markdown — not just generate
it. If the task is "regenerate the analytics table", "add an OKR section if
missing", "update one row of the dashboard table", or "extract the section
about X for review", reach for `mdli` instead of writing Python or shell that
manipulates strings.

## When to use mdli

- The document is long-lived and edited repeatedly (status reports, runbooks,
  OKR rollups, knowledge base pages, project plans).
- Changes must be idempotent — re-running must produce zero diff.
- A table needs to be regenerated from prepared row data.
- A section must be created if missing but left alone if present.
- A generated block must coexist with human-authored content in the same
  file.
- An agent needs only a bounded slice of the document to reason about a
  specific section.

## When **not** to use mdli

- The task is to fetch data from Jira, GitHub, or any external system.
  Prepare data upstream with `jq`, `jirali`, etc., then hand NDJSON to
  `mdli`.
- The task is prose linting or grammar feedback.
- The task is converting Markdown to PDF/HTML/Word.

`mdli` does Markdown editing primitives plus a recipe/template layer. It does
not fetch, classify, or transform data.

## Agent Contract

- stdout carries the primary product (JSON envelope, document, or text).
- stderr carries diagnostics and one-line summaries.
- `--json` never mixes diagnostics with stdout.
- Mutations default to a dry-run plan. Pass `--write` to mutate in place,
  `--emit document` to stream the transformed document, or `--emit json` for
  machine-readable plan output.
- Ambiguous selectors are an error (`E_AMBIGUOUS_SELECTOR`). `mdli` will not
  guess between duplicate headings.
- Selector resolution order: stable ID (`--id`) → exact path
  (`--path "H1 > H2"`) → error.

### Branch on exit codes

| Code | Meaning | What to do |
|---:|---|---|
| 0 | success | continue |
| 1 | user error (bad flags, missing fields, invalid ID) | fix the command and retry |
| 2 | document invariant violation (locked block, modified block, duplicate ID) | inspect the document, decide if `--on-modified force` or `block unlock` is appropriate |
| 3 | I/O error | check the path/permissions |
| 4 | stale preimage / concurrent edit | re-read the document and rebuild the plan |
| 64 | internal bug | file a report with the failing command |

### Required error codes (subset)

| Code | When |
|---|---|
| `E_AMBIGUOUS_SELECTOR` | path matched multiple sections |
| `E_SELECTOR_NOT_FOUND` | no section/block/table matched |
| `E_DUPLICATE_ID` | stable ID appears more than once |
| `E_BLOCK_MODIFIED` | managed block content was edited outside `mdli` |
| `E_BLOCK_LOCKED` | locked block edit attempted without `--force-locked` |
| `E_TABLE_INVALID` | malformed Markdown table |
| `E_TABLE_DUPLICATE_KEY` | duplicate key in `--from-rows` |
| `E_RICH_CELL` | object/array/multiline cell with `--on-rich-cell error` |
| `E_INVALID_UTF8` | input bytes are not UTF-8 |
| `E_STALE_PREIMAGE` | `--preimage-hash` mismatch |
| `E_RECIPE_INVALID` | recipe schema invalid |
| `E_TEMPLATE_PARSE` | template syntax error |
| `E_TEMPLATE_MISSING_DATASET` | template references a missing dataset |

## Recommended Workflows

### 1. Bootstrap stable IDs on a legacy file

```bash
mdli id assign report.md --all --write
```

This walks every heading and inserts `<!-- mdli:id v=1 id=... -->` markers
above each one, generating slugs from the visible text. After this, all
subsequent operations should reference sections by stable ID, not path.

### 2. Inspect before mutating

```bash
mdli inspect report.md --json
```

Returns sections, tables, blocks, and lint issues in one call. Use this to
decide whether the structure you intend to create already exists.

### 3. Get a bounded slice for an agent prompt

```bash
mdli context report.md --id cashplus.analytics --max-tokens 2000
```

Returns the selected section's body plus breadcrumb path, immediate sibling
headings, child heading summary, and managed-block metadata. Use this instead
of reading the whole file when only one section is in scope.

### 4. Idempotent section creation

```bash
mdli section ensure report.md \
  --id cashplus.analytics \
  --path "Report > 4. Campaign & Product Analytics" \
  --level 2 \
  --after cashplus.touchpoints \
  --write
```

Re-running this is a no-op if the section is already present with the right
ID and level.

### 5. NDJSON-to-table

```bash
mdli table replace report.md \
  --section cashplus.analytics \
  --name analytics-tickets \
  --columns Ticket=key,Summary=summary,Status=status,Priority=priority \
  --from-rows /tmp/analytics.ndjson \
  --key Ticket \
  --sort "Ticket:asc" \
  --truncate Summary=70 \
  --link Ticket="https://example.atlassian.net/browse/{key}" \
  --write
```

### 6. Recipe-driven regeneration

```bash
mdli plan cash_plus.md \
  --recipe cashplus-report.yml \
  --data tickets=/tmp/tickets.ndjson \
  --data refreshes=/tmp/refreshes.ndjson \
  > plan.json

mdli apply-plan cash_plus.md --plan plan.json --write
```

`apply-plan` refuses to write if the document changed between `plan` and
`apply-plan` (preimage-hash mismatch).

### 7. Managed-block ensure

```bash
mdli block ensure report.md \
  --parent-section cashplus.analytics \
  --id cashplus.analytics.notes \
  --body-from-file notes.md \
  --write
```

Subsequent runs replace only the block's content, never the surrounding
human-authored prose.

## Common Commands

```bash
# inspection
mdli inspect FILE [--json]
mdli tree FILE
mdli context FILE (--id ID | --path PATH) [--max-tokens N] [--include-managed-blocks]

# stable IDs
mdli id list FILE
mdli id assign FILE --all [--write]
mdli id assign FILE --section PATH (--id ID | --auto) [--write]

# sections
mdli section list FILE
mdli section get FILE (--id ID | --path PATH)
mdli section ensure FILE --id ID --path PATH --level N [--after SEL | --before SEL] [--write]
mdli section replace FILE (--id ID | --path PATH) --body-from-file BODY.md [--managed] [--write]
mdli section delete FILE (--id ID | --path PATH) [--write]
mdli section move FILE --id ID (--after SEL | --before SEL) [--write]
mdli section rename FILE --id ID --to "Title" [--write]

# tables
mdli table list FILE
mdli table get FILE (--section SEL | --name NAME)
mdli table replace FILE --section SEL [--name NAME] --columns SPEC --from-rows PATH [--key COL] [--sort SPEC] [--truncate COL=N] [--link COL=PATTERN] [--empty TEXT] [--write]
mdli table upsert FILE --name NAME --key COL (--row K=V ... | --from-rows PATH) [--write]
mdli table delete-row FILE --name NAME --key COL --value VALUE [--write]
mdli table sort FILE --name NAME --by SPEC [--write]
mdli table fmt FILE (--all | --name NAME) [--write]

# managed blocks
mdli block list FILE
mdli block get FILE --id ID
mdli block ensure FILE --parent-section SEL --id ID (--body-from-file BODY.md | --text TEXT) [--position start|end|before:ID|after:ID] [--write]
mdli block replace FILE --id ID --body-from-file BODY.md [--on-modified fail|force|three-way] [--write]
mdli block lock FILE --id ID [--write]
mdli block unlock FILE --id ID [--write]

# frontmatter (YAML and TOML)
mdli frontmatter get FILE [--key KEY]
mdli frontmatter set FILE KEY VALUE [--write]
mdli frontmatter delete FILE KEY [--write]

# lint
mdli lint FILE [--json]
mdli lint FILE --fix safe [--write]

# templates / recipes / plan / patch
mdli template render TEMPLATE.mdli --data NAME=PATH ...
mdli recipe validate RECIPE.yml
mdli apply FILE --recipe RECIPE.yml --data NAME=PATH ... [--write]
mdli build --recipe RECIPE.yml --data NAME=PATH ... --out FILE.md [--overwrite]
mdli plan FILE --recipe RECIPE.yml --data NAME=PATH ... > plan.json
mdli apply-plan FILE --plan plan.json [--write]
mdli patch FILE --edits edits.json [--write]
```

## Anti-footgun Defaults

- Mutating commands default to a JSON edit plan. They do not write. Pass
  `--write` or `--emit document` explicitly.
- Locked blocks fail to mutate by default. Use `block unlock` first or
  `--force-locked` (planned).
- Modified managed blocks fail with `E_BLOCK_MODIFIED`. Pass
  `--on-modified force` only after confirming the human edits are not worth
  preserving.
- Newer marker versions (`v=2` or higher in a v=1 reader) are preserved on
  read but not modified by default.
- Duplicate row keys in `--from-rows` error by default. Pass
  `--on-duplicate-key first|last` to choose a tie-breaker.

## Composition Boundary

`mdli` consumes prepared row datasets. It does **not** fetch, filter, group,
join, classify, or derive data. Prepare data upstream:

```bash
jirali issue list --jql 'project = CPMKTG' --json \
  | jq -c '.issues[] | {key, summary: .fields.summary, status: .fields.status.name}' \
  > /tmp/tickets.ndjson

mdli table replace report.md \
  --section cashplus.analytics \
  --name analytics-tickets \
  --columns Ticket=key,Summary=summary,Status=status \
  --from-rows /tmp/tickets.ndjson \
  --key Ticket \
  --write
```

## Wire Format Quick Reference

```md
<!-- mdli:id v=1 id=cashplus.analytics -->
<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->
<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:... locked=false -->
<!-- mdli:end v=1 id=cashplus.analytics.generated -->
```

Markers are single-line HTML comments. Field order written by `mdli` is
canonical: `v` first, primary identifier (`id` or `name`) second, remaining
fields alphabetical. Unknown fields round-trip untouched.

## See Also

- [`README.md`](./README.md) — operator-facing summary and quickstart.
- [`mdli-prd-final.md`](./mdli-prd-final.md) — full PRD, including the
  fidelity contract, success metrics, and roadmap.
