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

Use the Python CLI unless you specifically need the Rust port:

```bash
uv run python -m tools.vaultli ...
```

Prefer `--json` in agent flows so results are machine-readable.
All commands also accept `--root` so you can target the vault explicitly instead of
depending on the current working directory.

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
uv run python -m tools.vaultli root .
```

2. Create the vault only if `.kbroot` is missing.

```bash
uv run python -m tools.vaultli init ./kb
```

3. Choose the right write path.

| Need | Use |
|---|---|
| Add frontmatter to an existing markdown file and index it right away | `add <file>` |
| Create a sidecar for a non-markdown asset without indexing yet | `scaffold <file>` |
| Preview the generated metadata before writing anything | `infer <file>` |
| Rebuild cache state after edits | `index` |
| Audit the vault for broken links, duplicate IDs, and stale index state | `validate` |
| Look up candidate records by metadata | `search`, then `show` |

4. Immediately refine generated metadata.

The inferred metadata is a draft. A new agent should treat the generated values as placeholders:

- rewrite `description` to be retrieval-friendly and specific
- tighten `tags`
- correct `category` if the inference is too generic
- add `depends_on` and `related` only when the IDs are real
- flesh out the markdown body so humans and later agents have context

5. Rebuild and validate.

```bash
uv run python -m tools.vaultli index --root ./kb --json
uv run python -m tools.vaultli validate --root ./kb --json
```

6. Retrieve in two stages.

```bash
uv run python -m tools.vaultli search retention --root ./kb --json
uv run python -m tools.vaultli show queries/retention --root ./kb --json
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
- `search --jq` requires the `jq` binary to be installed.
- Sidecar hash changes come from the source asset bytes, not from sidecar prose edits.

## A Safe Default Loop

```bash
uv run python -m tools.vaultli root .
uv run python -m tools.vaultli add ./kb/docs/guide.md --root ./kb
uv run python -m tools.vaultli scaffold ./kb/queries/report.sql --root ./kb
# edit the generated markdown files
uv run python -m tools.vaultli index --root ./kb --json
uv run python -m tools.vaultli validate --root ./kb --json
uv run python -m tools.vaultli search report --root ./kb --json
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
- `py/core.py` is the Python reference implementation.
- `rs/` contains the Rust port.
