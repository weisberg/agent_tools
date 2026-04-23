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
