# mdli

Agent-native Markdown document operations. `mdli` treats Markdown as an
editable AST — sections, tables, frontmatter, managed blocks — and exposes
each as a small, idempotent CLI command with structured JSON output. It exists
so AI agents stop hand-building `.md` files line by line.

The full design is in [`mdli-prd-final.md`](./mdli-prd-final.md). This README
is the operator-facing summary.

## Quickstart

```bash
cargo run -- inspect report.md --json
cargo run -- tree report.md
cargo run -- section ensure report.md \
  --id cashplus.analytics \
  --path "Report > 4. Campaign & Product Analytics" \
  --level 2 \
  --write
cargo run -- table replace report.md \
  --section cashplus.analytics \
  --name analytics-tickets \
  --columns Ticket=key,Summary=summary,Status=status \
  --from-rows /tmp/analytics.ndjson \
  --key Ticket \
  --write
```

For agent context extraction:

```bash
cargo run -- context report.md --id cashplus.analytics --max-tokens 2000
```

## Design Principles

1. **Idempotency by default.** Re-running any mutating command produces zero
   diff against an already-managed file.
2. **Stable selectors over visible paths.** Hidden ID markers
   (`<!-- mdli:id v=1 id=foo -->`) survive heading renames and reorderings;
   path selectors are for bootstrap only.
3. **Tables are data.** Agents pass NDJSON rows; the renderer owns escaping,
   alignment, link formatting, and stable sort order.
4. **Generated content is fenced.** Managed blocks
   (`<!-- mdli:begin ... -->` / `<!-- mdli:end ... -->`) carry SHA-256
   checksums so regeneration cannot clobber human edits.
5. **Ambiguity is an error.** `mdli` never guesses between duplicate
   selectors.
6. **Dry-run is first-class.** Mutating commands default to a JSON edit plan;
   you must pass `--write` or `--emit document` to mutate.
7. **Clean stdout for agents.** `--json` never mixes diagnostics with stdout.

## Implementation Status

MVP surface (PRD Phases 1–4) — implemented:

| Group | Commands |
|---|---|
| Inspection | `inspect`, `tree`, `context` |
| Stable IDs | `id list`, `id assign` |
| Sections | `section list/get/ensure/replace/delete/move/rename` |
| Tables | `table list/get/replace/upsert/delete-row/sort/fmt` |
| Managed blocks | `block list/get/ensure/replace/lock/unlock` |
| Frontmatter | `frontmatter get/set/delete` (YAML + TOML) |
| Lint | `lint` (and `lint --fix safe`) |
| Validate | `validate --schema SCHEMA.yml` |

Post-MVP surface (PRD Phases 5–7) — implemented:

| Group | Commands |
|---|---|
| Templates | `template render` (`value`, `table`, `if_present` helpers) |
| Recipes | `recipe validate`, `apply`, `build` |
| Plans | `plan`, `apply-plan`, `patch` |

Not yet implemented:

- semantic `diff` (Phase 8)
- Git integration (`--require-clean-git`, snapshot mode) — backlogged

## Universal Flags

All mutating commands accept:

```text
--write                  # atomic in-place write (mutually exclusive with --emit document)
--emit plan|document|json   # default: plan
--preimage-hash HASH     # refuse if input bytes do not match
```

All commands accept:

```text
--json                   # force the mdli/output/v1 envelope on stdout
--quiet                  # suppress non-error diagnostics
```

`FILE` may be `-` for stdin reads. `--write` requires a real path.

## Output Contract

Every JSON response is wrapped in a stable envelope:

```json
{
  "schema": "mdli/output/v1",
  "ok": true,
  "result": { ... }
}
```

Errors:

```json
{
  "schema": "mdli/output/v1",
  "ok": false,
  "error": {
    "code": "E_AMBIGUOUS_SELECTOR",
    "message": "selector matched more than one structure"
  }
}
```

Mutating commands return an edit summary:

```json
{
  "changed": true,
  "preimage_hash": "sha256:...",
  "postimage_hash": "sha256:...",
  "ops": [ { "op": "ensure_section", "id": "...", "path": "...", "level": 2 } ],
  "warnings": []
}
```

## Exit Codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 1 | User error (bad flags, missing input, invalid grammar) |
| 2 | Document invariant violation (locked block, modified block, duplicate ID) |
| 3 | I/O error |
| 4 | Stale preimage / concurrent edit |
| 64 | Internal bug |

## Recipe / Apply Flow

A recipe is a YAML or JSON document declaring a target structure for a
Markdown report. `apply` regenerates that structure idempotently against
prepared row datasets.

```yaml
# cashplus-report.yml
schema: mdli/recipe/v1
title: Current Cash Plus Epics & Stories
sections:
  - id: cashplus.analytics
    path: "4. Campaign & Product Analytics"
    level: 2
    after: cashplus.touchpoints
    template: templates/analytics.mdli
    bindings:
      tickets: analytics
```

```bash
mdli apply cash_plus.md \
  --recipe cashplus-report.yml \
  --data analytics=/tmp/analytics.ndjson \
  --write
```

For two-step review, generate a plan first and apply it after inspection:

```bash
mdli plan cash_plus.md --recipe cashplus-report.yml --data analytics=/tmp/analytics.ndjson > plan.json
mdli apply-plan cash_plus.md --plan plan.json --write
```

`apply-plan` refuses to write if the document's preimage hash no longer
matches the plan, protecting against concurrent edits.

## Templates

Templates support exactly three helpers — no expressions, no loops, no shell:

```md
**Last updated:** {{ value last_updated }}

{{ if_present optional_note }}
{{ value optional_note }}
{{ end }}

{{ table tickets
   columns=["Ticket=key", "Summary=summary", "Status=status"]
   key="Ticket"
   sort=["Ticket:asc"]
   truncate={"Summary": 70}
   link={"Ticket": "https://example.atlassian.net/browse/{key}"}
   empty="No matching tickets."
}}
```

## Wire Format

`mdli` markers are persistent on-disk wire format and live in
source-controlled documents:

```md
<!-- mdli:id v=1 id=cashplus.analytics -->
## 4. Campaign & Product Analytics

<!-- mdli:table v=1 name=analytics-tickets key=Ticket -->
| Ticket | Summary | Status |
| --- | --- | --- |

<!-- mdli:begin v=1 id=cashplus.analytics.generated checksum=sha256:... locked=false -->
Generated content goes here.
<!-- mdli:end v=1 id=cashplus.analytics.generated -->
```

A `vN` reader must read all `vM` markers for `M <= N`. Unknown fields are
preserved on round-trip.

## Round-trip Fidelity

`mdli` documents what it preserves and what it canonicalizes:

- Preserved: heading text/level, code-fence content byte-for-byte, list bullet
  style, link reference order, frontmatter when not edited, blank-line counts
  between top-level blocks, unknown marker fields, CRLF line endings, UTF-8
  BOM.
- Canonicalized: generated table alignment uses `|---|` style with one space
  of cell padding, trailing whitespace outside code fences is stripped, final
  newline is enforced, marker fields written by `mdli` use canonical order.

The fixture corpus in `tests/fixtures/` covers each documented edge case
(duplicate headings, escaped `>`, Unicode, code-fence content, malformed
tables, locked/tampered blocks, orphan markers, newer-version markers,
inline HTML, YAML/TOML frontmatter, CRLF, BOM).

## Validation Schemas

Gate a recipe-driven report's structure in CI without requiring the recipe
itself:

```yaml
# report.schema.yml
schema: mdli/validation/v1
required_sections:
  - id: cashplus.okr
    level: 2
  - id: cashplus.analytics
    level: 2
required_tables:
  - name: analytics-tickets
    columns: [Ticket, Summary, Status, Priority]
    key: Ticket
managed_blocks:
  - id: cashplus.analytics.generated
    locked: false
```

```bash
mdli validate report.md --schema report.schema.yml
```

`validate` exits 0 with `"ok": true` when the document satisfies every
required section, table column shape, table key, and managed-block lock
state. Each failure is reported as a structured finding with a stable error
code (`E_VALIDATION_MISSING_SECTION`, `E_VALIDATION_TABLE_COLUMNS`, …).

## Three-way Conflict Resolution

When a managed block has been edited outside `mdli`, `block replace --on-modified three-way`
writes a `<file>.mdli.conflict` JSON sidecar containing the recorded base
checksum, the on-disk body, and the incoming body, and exits non-zero. The
source file is left untouched so an agent can read the artifact, reconcile,
and re-issue the edit.

```bash
mdli block replace report.md --id section.gen \
  --body-from-file body.md --on-modified three-way
# → exits non-zero, writes report.md.mdli.conflict
```

## Ambiguity Errors Carry Match Details

`E_AMBIGUOUS_SELECTOR` includes a `matches` array under `error.details` so
agents can disambiguate without re-parsing the document:

```json
{
  "schema": "mdli/output/v1",
  "ok": false,
  "error": {
    "code": "E_AMBIGUOUS_SELECTOR",
    "message": "selector matched more than one structure",
    "details": {
      "matches": [
        {"id": null, "path": "Top > Same", "line": 3, "level": 2},
        {"id": null, "path": "Top > Same", "line": 11, "level": 2}
      ]
    }
  }
}
```

## Testing

```bash
cargo test
```

Currently 84 integration tests across `cli_contract`, `fixtures`, `recipe`,
`template`, `tree`, `context`, and `validate`.

## Skill

See [`SKILL.md`](./SKILL.md) for the agent contract and command-by-command
reference.
