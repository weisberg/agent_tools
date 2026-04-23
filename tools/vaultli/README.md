# vaultli

`vaultli` is a CLI for building and maintaining a file-based knowledge vault.

It helps agents and humans turn a directory of markdown files, SQL queries, templates, runbooks, and other assets into a structured knowledge base with:

- YAML frontmatter for document metadata
- sidecar markdown for non-markdown assets
- a derived `INDEX.jsonl` for fast lookup and filtering
- validation checks for broken or stale vault state

The source of truth is always the files on disk. `INDEX.jsonl` is just a cache built from those files.

## What It Does

`vaultli` standardizes how knowledge is stored and discovered:

- Native markdown files keep their metadata inline in frontmatter.
- Non-markdown assets such as `.sql` or `.j2` files get sidecar docs like `query.sql.md`.
- Each indexed document gets a stable `id`, `title`, `description`, and other retrieval-friendly metadata.
- The vault can be re-indexed and validated at any time.

This makes the vault easier to:

- search
- audit
- version with git
- consume from agents
- evolve without a database

## Core Model

vaultli is built around three ideas:

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

## What It Does Not Do

`vaultli` is not a full retrieval engine or document database.

- `search` works against `INDEX.jsonl`, not raw document bodies.
- Non-markdown files are invisible until they have sidecars.
- `validate` reports problems but does not auto-fix them.
- `INDEX.jsonl` should not be edited by hand.

If you want the actual content behind a match, use the indexed `file` path and, for sidecars, the `source` field.

## Main Commands

| Command | Purpose |
|---|---|
| `init [path]` | Create a new vault root with `.kbroot` and an empty `INDEX.jsonl` |
| `index [--full]` | Rebuild the vault index |
| `search <query>` | Search indexed metadata |
| `show <id>` | Show one indexed record by `id` |
| `add <file>` | Scaffold metadata for a file and re-index |
| `scaffold <file>` | Create frontmatter or sidecar metadata without re-indexing |
| `validate` | Report broken sources, duplicate ids, dangling refs, and stale index state |
| `root [path]` | Find the nearest vault root |
| `make-id <file>` | Derive the canonical vault id for a file |
| `infer <file>` | Preview inferred metadata without writing |
| `dump-index` | Dump all index records as JSON |

All commands support `--root`, and agent workflows should usually use `--json`.

## Quickstart

The Rust binary is the default implementation. Build once:

```bash
cd rs && cargo build --release
# binary is now at ./rs/target/release/vaultli
```

Then (either put it on your PATH or invoke by full path):

```bash
vaultli --help
vaultli --json init ./kb
vaultli --json add ./kb/docs/guide.md --root ./kb
vaultli --json scaffold ./kb/queries/retention.sql --root ./kb
vaultli --json index --root ./kb
vaultli --json validate --root ./kb
vaultli --json search retention --root ./kb
vaultli --json show queries/retention --root ./kb
```

Python fallback (invoke with the parent of the `vaultli` package on `PYTHONPATH`):

```bash
PYTHONPATH=<parent-of-vaultli> python -m vaultli --help
```

## Sidecars

For non-markdown files, `vaultli` uses same-directory sidecars:

| Source file | Sidecar |
|---|---|
| `report.sql` | `report.sql.md` |
| `template.j2` | `template.j2.md` |
| `config.yaml` | `config.yaml.md` |

The sidecar carries metadata and optional prose documentation, including a required `source` field such as:

```yaml
source: ./report.sql
```

## Recommended Agent Workflow

For a new agent, the safest default loop is:

1. Find the vault root with `root`.
2. Use `add` for markdown and `scaffold` for non-markdown files.
3. Improve the inferred metadata, especially `description`, `tags`, and `category`.
4. Run `index`.
5. Run `validate`.
6. Use `search` to shortlist records, then open the real files.

## Implementations

vaultli currently ships in two implementations:

| Implementation | Role | Run |
|---|---|---|
| Rust | Primary implementation | `cd rs && cargo build --release && ./rs/target/release/vaultli ...` |
| Python | Reference / parity oracle | `PYTHONPATH=<parent-of-vaultli> python -m vaultli ...` |

Both implementations are behaviorally identical — the Rust crate's parity test
suite compares their outputs byte-for-byte. The package can be relocated freely;
set `VAULTLI_PY_PATH` if you want to run the parity tests from outside the
default in-repo layout.

## Related Docs

- `vaultli-spec-v1.0.md` — storage format and metadata spec
- `SKILL.md` — agent-first operating guide
- `rs/` — primary (Rust) implementation
- `py/core.py` — Python reference implementation
