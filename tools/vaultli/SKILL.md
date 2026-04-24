---
name: vaultli
description: |
  Manage a file-based knowledge vault with YAML frontmatter, sidecar markdown, and
  JSONL indexing. Use when initializing or maintaining a knowledge base, attaching
  metadata to markdown or non-markdown assets, rebuilding or validating INDEX.jsonl,
  or searching vault records by metadata. Trigger on: "knowledge base", "vault",
  "frontmatter", "sidecar", "INDEX.jsonl", "document this query/template/skill",
  "search the vault", or "set up KB docs".
---

# vaultli

Use `vaultli` when the job is to make a repository's knowledge assets agent-discoverable.
It is strongest at standardizing metadata, creating sidecars, building `INDEX.jsonl`,
and validating vault integrity.

## What vaultli does

`vaultli` turns a directory of markdown files, queries, templates, runbooks, and
other assets into a structured file-based knowledge base.

- Native markdown files keep metadata inline in YAML frontmatter.
- Non-markdown assets get same-directory sidecars such as `report.sql.md`.
- Indexed records are written to `INDEX.jsonl` for fast lookup and filtering.
- Validation checks help catch stale, broken, or internally inconsistent vault state.

The source of truth is the files on disk. `INDEX.jsonl` is a derived cache.

## What This Skill Is Not

`vaultli` is not a full-text search engine and not a document reader.

- `search` and `show` operate on `INDEX.jsonl`.
- Search quality depends on frontmatter, especially `description`, `tags`, and `category`.
- After a hit is found, open the file from the returned `file` field.
- For sidecars, you often need both the `.md` sidecar and the `source` asset.

## Default Invocation

Prefer the Rust binary — it is at behavioral parity with the Python reference
(verified by cross-language parity tests) and starts an order of magnitude
faster, which matters in tight agent loops:

```bash
# one-time build from the vaultli directory
cd <vaultli>/rs && cargo build --release

# then invoke the binary directly (or put it on your PATH)
<vaultli>/rs/target/release/vaultli --json <command> ...
```

`<vaultli>` is wherever this package lives; the binary has no other install-time
dependencies and the directory can be relocated freely.

Fall back to the Python CLI only when the Rust binary is unavailable (no Rust
toolchain, or debugging a suspected Rust-specific bug):

```bash
uv run python -m tools.vaultli ...
```

Both implementations accept the same subcommands, the same flags (including
`--json` and `--root`), and produce byte-identical index records. Prefer
`--json` in agent flows so results are machine-readable. Use `--root` to target
the vault explicitly instead of depending on the current working directory.

## Core model

vaultli is built around three simple ideas:

1. Markdown is the universal knowledge wrapper.
2. YAML frontmatter is the universal metadata format.
3. JSONL is the universal index format.

A typical vault looks like this:

```text
kb/
  .kbroot
  INDEX.jsonl
  docs/
    guide.md
  queries/
    retention.sql
    retention.sql.md
  templates/
    campaign_report.j2
    campaign_report.j2.md
```

## Core Workflow

1. Find or confirm the vault root.

```bash
vaultli --json root .
```

2. Create the vault only if `.kbroot` is missing.

```bash
vaultli --json init ./kb
```

3. Choose the right write path.

| Need | Use |
|---|---|
| Add frontmatter to an existing markdown file and index it right away | `add <file>` |
| Create a sidecar for a non-markdown asset without indexing yet | `scaffold <file>` |
| Bulk scaffold missing metadata for a file or directory | `ingest <path>` |
| Preview a bulk ingest without writing files | `ingest <path> --dry-run` |
| Preview the generated metadata before writing anything | `infer <file>` |
| Rebuild cache state after edits | `index` |
| Audit the vault for broken links, duplicate IDs, and stale index state | `validate` |
| Look up candidate records by metadata | `search`, then `show`; narrow with `--category`, `--status`, `--domain`, `--scope`, `--tag`, or `--limit` |

4. Immediately refine generated metadata.

The inferred metadata is a draft. A new agent should treat the generated values as placeholders:

- rewrite `description` to be retrieval-friendly and specific
- tighten `tags`
- correct `category` if the inference is too generic
- add `depends_on` and `related` only when the IDs are real
- flesh out the markdown body so humans and later agents have context

5. Rebuild and validate.

```bash
vaultli --json index --root ./kb
vaultli --json validate --root ./kb
```

6. Retrieve in two stages.

```bash
vaultli --json search retention --root ./kb
vaultli --json search --root ./kb --category query --tag retention --limit 5
vaultli --json show queries/retention --root ./kb
```

Then open the actual file referenced by `file`.

## Sidecar rules

For non-markdown files, vaultli uses same-directory sidecars:

| Source file | Sidecar |
|---|---|
| `report.sql` | `report.sql.md` |
| `template.j2` | `template.j2.md` |
| `config.yaml` | `config.yaml.md` |

The sidecar must include a valid `source` field, usually:

```yaml
source: ./report.sql
```

## Rules That Prevent Bad KBs

- Never edit `INDEX.jsonl` directly.
- Native markdown gets inline frontmatter.
- Non-markdown files need same-directory sidecars named `<file>.<ext>.md`.
- Sidecars must have a valid relative `source`, typically `./filename.ext`.
- Non-markdown assets are not searchable until a sidecar exists.
- `validate` reports problems; it does not repair them.
- Prefer `search` filters over `search --jq` for common metadata fields.
- `search --jq` requires the `jq` binary to be installed.
- Sidecar hash changes come from the source asset bytes, not from sidecar prose edits.

## A Safe Default Loop

```bash
vaultli --json root .
vaultli --json add ./kb/docs/guide.md --root ./kb
vaultli --json scaffold ./kb/queries/report.sql --root ./kb
vaultli --json ingest ./kb --root ./kb --dry-run
# edit the generated markdown files
vaultli --json index --root ./kb
vaultli --json validate --root ./kb
vaultli --json search report --root ./kb
```

## When A New Agent Usually Gets Confused

- Expecting body text to be searchable without opening files.
- Assuming a source file is indexed just because it exists in the vault.
- Leaving the generic inferred `description` untouched.
- Forgetting to re-run `index` after metadata edits.
- Treating `validate` output as if it were self-healing.

## Related docs

- `README.md` explains what vaultli is and how the core model works.
- `vaultli-spec-v1.0.md` defines the storage layout and metadata schema.
- `rs/` is the primary implementation (Rust); build with `cargo build --release` and invoke `rs/target/release/vaultli`.
- `py/core.py` is the Python reference implementation, still kept in sync and used as the parity oracle by `rs/tests/parity.rs`.
