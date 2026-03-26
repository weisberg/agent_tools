# vaultli — Storage & Metadata Specification v1.0

**Version:** 1.0  
**Author:** Brian  
**Date:** March 2026  
**Status:** Draft

---

## 1. Executive Summary

vaultli is a command-line interface for managing a file-based knowledge vault. It provides a standardized system for attaching structured metadata to documents, building a searchable flat-file index, and querying that index with standard Unix tools.

The system is designed around three core principles: markdown files are the universal unit of knowledge, YAML frontmatter is the universal metadata format, and JSONL is the universal index format. Every document in the vault — whether a native markdown file, a SQL query, a Jinja2 template, or any other asset — gets a standardized metadata envelope that makes it discoverable, classifiable, and composable.

This specification defines the storage layout, the metadata schema, the sidecar file convention for non-markdown assets, the INDEX.jsonl format, and the indexing process. It establishes the foundation upon which all vaultli CLI commands, agent integrations, and retrieval systems are built.

---

## 2. Design Philosophy

### 2.1 Files Over Databases

vaultli stores everything as plain files on the filesystem. There is no SQLite database, no embedded key-value store, no proprietary binary format. The vault is a directory tree of human-readable files that can be versioned with git, searched with grep, edited with any text editor, and backed up by copying a folder. The INDEX.jsonl file is a derived artifact — a cache that accelerates search — not the source of truth. The source of truth is always the files themselves.

### 2.2 Convention Over Configuration

The system relies on predictable conventions rather than extensive configuration. Sidecar files use a deterministic naming pattern. The vault root is identified by a marker file. The frontmatter schema uses standardized field names. These conventions mean that any tool — human or agent — can navigate the vault without reading a configuration file.

### 2.3 Agent-Native Design

Every design decision optimizes for consumption by AI agents operating under token-budget constraints. The INDEX.jsonl file exists so that an agent can grep a single file to find relevant documents without traversing the filesystem. The `tokens` field in the metadata exists so that a context-assembly algorithm can solve the knapsack problem of fitting the most relevant content into a fixed context window. The `description` field is written to be the single most important signal for retrieval relevance.

### 2.4 Unix Tool Compatibility

The index format is JSONL specifically because grep returns one complete record per matching line. No custom query language is needed for basic search. For structured queries, jq provides full JSON filtering. The combination of grep for keyword search and jq for structured queries covers the vast majority of retrieval needs without any custom tooling.

---

## 3. Vault Structure

### 3.1 Vault Root and the .kbroot Marker

The vault root is the top-level directory that contains all knowledge assets. It is identified by the presence of a `.kbroot` marker file. This follows the same pattern used by git (`.git`), cargo (`Cargo.toml`), and node (`package.json`) to establish project boundaries.

Any tool that needs to locate the vault root walks up the directory tree from its current working directory until it finds a `.kbroot` file. This means vaultli commands can be invoked from any subdirectory within the vault.

The `.kbroot` file may be empty or may contain optional vault-level configuration in the future (e.g., vault name, default author). For version 1.0, its mere presence is sufficient.

**Root discovery algorithm:**

```python
def find_root(start: Path = Path.cwd()) -> Path:
    current = start.resolve()
    for parent in [current, *current.parents]:
        if (parent / ".kbroot").exists():
            return parent
    raise FileNotFoundError("No .kbroot found")
```

### 3.2 Directory Layout

The vault imposes no mandatory directory structure. Users organize files in whatever hierarchy makes sense for their domain. A typical vault looks like this:

```
~/vault/
  .kbroot                          ← vault root marker
  INDEX.jsonl                      ← derived search index
  docs/
    experimentation-guide.md       ← native markdown (frontmatter inline)
    cuped-methodology.md
  queries/
    retention_holdout.sql           ← native SQL file (executable)
    retention_holdout.sql.md        ← sidecar metadata + documentation
    campaign_metrics.sql
    campaign_metrics.sql.md
  templates/
    campaign_report.j2              ← native Jinja2 template
    campaign_report.j2.md           ← sidecar metadata + documentation
  skills/
    athena-analyst/
      SKILL.md                     ← native markdown with frontmatter
  runbooks/
    deploy-pipeline.md
```

The INDEX.jsonl file always lives at the vault root, adjacent to `.kbroot`. All file paths stored in the index and in frontmatter are relative to this root.

### 3.3 File Classification

vaultli recognizes two classes of files in the vault:

**Native markdown files** are `.md` files whose content is the knowledge asset itself. These files carry their YAML frontmatter directly at the top of the file. Examples include documentation, guides, PRDs, meeting notes, and skill definitions.

**Sidecar markdown files** are `.md` files that provide metadata and documentation for a non-markdown asset. They are identified by their double extension: the original file's full name plus `.md` (e.g., `query.sql.md` is the sidecar for `query.sql`). The sidecar contains YAML frontmatter with a `source` field pointing to the original asset, plus optional prose documentation.

Non-markdown files without a sidecar are invisible to the index. They exist in the vault but are not searchable until a sidecar is created for them. This is intentional — it means the indexer never needs to parse SQL, Jinja2, Python, or any other language. It only reads YAML frontmatter from `.md` files.

---

## 4. Sidecar File Convention

### 4.1 Naming Pattern

A sidecar file is created by appending `.md` to the full filename of the source asset. The sidecar must reside in the same directory as the source file.

| Source File | Sidecar File |
|---|---|
| `retention_holdout.sql` | `retention_holdout.sql.md` |
| `campaign_report.j2` | `campaign_report.j2.md` |
| `etl_pipeline.py` | `etl_pipeline.py.md` |
| `dashboard_config.json` | `dashboard_config.json.md` |
| `model_weights.onnx` | `model_weights.onnx.md` |

This pattern is unambiguous: any `.md` file whose stem itself contains an extension (i.e., the filename before `.md` contains a dot) is a sidecar. A file named `report.md` is native markdown; a file named `report.sql.md` is a sidecar for `report.sql`.

### 4.2 Sidecar File Structure

A sidecar file has the same structure as any markdown file in the vault: YAML frontmatter delimited by `---` lines, followed by an optional markdown body. The body can contain documentation, usage examples, parameter descriptions, sample output, or any other prose that contextualizes the source asset.

```yaml
---
id: queries/retention-holdout
title: Retention Holdout Measurement Query
description: >-
  Athena SQL measuring 12-month retention delta between holdout
  and exposed groups for campaign experiments
category: query
tags: [retention, experiment, athena, cuped, holdout]
aliases: [retention-query, holdout-measurement]
author: brian
status: active
created: 2025-11-15
updated: 2026-03-10
source: ./retention_holdout.sql
depends_on: [campaign-exposure-schema, cuped-adjustment-udf]
related: [email-experiment-treatise, cx-journey-case-study]
domain: experimentation
scope: team
tokens: 340
priority: 2
version: 3
---

## Purpose

This query measures the incremental retention impact of ...
```

### 4.3 The source Field

The `source` field is required in all sidecar files and optional in native markdown files. It contains a relative path from the sidecar to the source asset. By convention this is always a same-directory reference using `./` notation.

The `source` field serves multiple purposes: it establishes the sidecar-to-asset relationship for the indexer, it tells agents where to find the executable content, and it provides the path used for content hashing during incremental index rebuilds.

### 4.4 When to Use Sidecars vs. Native Markdown

The decision is straightforward: if the file is already markdown, put the frontmatter directly in the file. If the file is anything else, create a sidecar. Never convert a SQL query, Jinja2 template, Python script, or configuration file into markdown just to add frontmatter. The source file must remain valid and executable in its native format.

---

## 5. YAML Frontmatter Schema

The frontmatter schema defines 18 first-class fields organized into five functional groups. All fields are optional except `id`, `title`, and `description`, which are required for a file to be indexed. Fields should appear in the order listed below for consistency across the vault.

### 5.1 Identity and Discovery

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | **Yes** | Stable unique identifier, derived from the file's relative path. This is the join key used in `depends_on` and `related` references. Filesystem-safe slug, e.g. `queries/retention-holdout`. |
| `title` | string | **Yes** | Human-readable display name. Not constrained by filesystem naming rules. Shown in search results and agent context assemblies. |
| `description` | string | **Yes** | One-sentence purpose statement. The single most important field for retrieval — BM25 matches against it, agents read it to assess relevance, and it populates INDEX.jsonl for grep. |
| `tags` | list | No | Flat list of lowercase hyphenated keywords for faceted filtering. Non-hierarchical; be generous. Example: `[retention, experiment, athena]` |
| `category` | string | No | Single canonical classification: `query`, `template`, `prd`, `skill`, `runbook`, `note`, `reference`, `tutorial`, or a custom value. Drives how agents interpret the document. |
| `aliases` | list | No | Alternative names, acronyms, or search terms. Provides cheap synonym expansion for BM25. Example: `[variance-reduction, pre-experiment-adjustment]` |

### 5.2 Authorship and Lifecycle

| Field | Type | Required | Description |
|---|---|---|---|
| `author` | string | No | Creator of the document. Simple string or handle. Used by agent personas for attribution. |
| `status` | string | No | Lifecycle state: `draft`, `active`, `review`, `deprecated`, `archived`. Agents can filter on this to exclude stale content from context assembly. |
| `created` | date | No | ISO date of original creation (`YYYY-MM-DD`). Immutable once set. |
| `updated` | date | No | ISO date of last meaningful edit. Freshness signal for recency-weighted retrieval. Also used by the indexer to detect changes. |

### 5.3 Relationships and Lineage

| Field | Type | Required | Description |
|---|---|---|---|
| `source` | string | No* | Relative path to the executable asset this sidecar documents. **Required for sidecar files.** Uses `./` same-directory notation. Example: `./retention_holdout.sql` |
| `depends_on` | list | No | List of `id`s this document structurally requires. Builds a dependency graph — agents can pull in prerequisites during context assembly. |
| `related` | list | No | Soft links to conceptually related documents by `id`. Not structural dependencies. Used for context expansion when token budget allows. |

### 5.4 Agent and Retrieval Mechanics

| Field | Type | Required | Description |
|---|---|---|---|
| `tokens` | integer | No | Approximate token count of the document body. Pre-computed so context assembly can solve the knapsack problem without tokenizing at query time. |
| `priority` | integer | No | Importance rank from 1 (highest) to 5 (lowest). Breaks ties when multiple documents match and the context window is tight. |
| `scope` | string | No | Audience boundary: `personal`, `team`, `org`, `public`. Controls visibility for agent context assembly and future multi-user scenarios. |

### 5.5 Context and Grounding

| Field | Type | Required | Description |
|---|---|---|---|
| `domain` | string | No | Knowledge domain: `experimentation`, `marketing-analytics`, `infrastructure`, `tooling`, `finance`, `management`. Coarser than tags; helps agents select expertise mode. |
| `version` | string | No | Semantic version string or simple integer. For living documents that evolve (PRDs, skills, schemas). Keeps old versions discoverable. |

---

## 6. ID Generation

The `id` field is a stable, human-readable slug derived from the file's relative path within the vault. It serves as the permanent address for the document across the entire system — used in `depends_on` references, `related` links, agent context requests, and dependency graph traversal.

### 6.1 Derivation Rules

The `id` is computed by taking the file's path relative to the vault root, stripping the `.md` extension (and the source extension for sidecars), lowercasing, and replacing spaces and underscores with hyphens. Directory separators are preserved as forward slashes.

| File Path (relative to root) | Derived ID |
|---|---|
| `docs/experimentation-guide.md` | `docs/experimentation-guide` |
| `queries/retention_holdout.sql.md` | `queries/retention-holdout` |
| `templates/campaign_report.j2.md` | `templates/campaign-report` |
| `skills/athena-analyst/SKILL.md` | `skills/athena-analyst/skill` |

**Reference implementation:**

```python
from pathlib import Path

def make_id(filepath: str, root: str = ".") -> str:
    rel = Path(filepath).relative_to(root)
    # Strip .md, then strip source extension for sidecars
    stem = rel.with_suffix("")
    if stem.suffix:  # has another extension → sidecar
        stem = stem.with_suffix("")
    slug = str(stem).replace("_", "-").replace(" ", "-").lower()
    return slug
```

### 6.2 Collision Handling

Because the `id` is derived from the filesystem path, collisions are impossible within a single vault. If a shortened slug scheme is desired for display purposes, the indexer can generate `short_id` values with parent-directory disambiguation for conflicts. The full path-based `id` remains the canonical identifier.

---

## 7. INDEX.jsonl Specification

### 7.1 Format

The INDEX.jsonl file is a JSONL (JSON Lines) file where each line is a self-contained JSON object representing one indexed document. The file lives at the vault root, adjacent to `.kbroot`. It is a derived artifact — fully rebuildable from the source `.md` files — and should be treated as a cache, not a source of truth.

The JSONL format was chosen for three specific reasons:

1. **grep returns complete records** because each line is one record.
2. **jq provides structured queries** when needed without custom tooling.
3. **Every language parses JSON natively** — no custom deserializer needed for agents or scripts.

### 7.2 Record Structure

Each line contains a JSON object with all frontmatter fields from the source `.md` file, plus two indexer-managed fields:

- `file` — the relative path to the `.md` file from the vault root.
- `hash` — a 12-character SHA-256 hex digest of the content body (see §8.3).

These fields are not part of the frontmatter schema — they are computed and stored exclusively in the index.

**Example record** (shown wrapped for readability; in the actual file this is a single line):

```json
{
  "id": "queries/retention-holdout",
  "title": "Retention Holdout Measurement Query",
  "description": "Athena SQL measuring 12-month retention delta between holdout and exposed groups",
  "category": "query",
  "tags": ["retention", "experiment", "athena", "cuped", "holdout"],
  "aliases": ["retention-query", "holdout-measurement"],
  "author": "brian",
  "status": "active",
  "created": "2025-11-15",
  "updated": "2026-03-10",
  "source": "./retention_holdout.sql",
  "depends_on": ["campaign-exposure-schema", "cuped-adjustment-udf"],
  "related": ["email-experiment-treatise", "cx-journey-case-study"],
  "domain": "experimentation",
  "scope": "team",
  "tokens": 340,
  "priority": 2,
  "version": 3,
  "file": "queries/retention_holdout.sql.md",
  "hash": "a1b2c3d4e5f6"
}
```

**Important:** JSON string escaping guarantees one record per line. Newlines in string values become `\n` (literal backslash-n), quotes become `\"`, tabs become `\t`. Always use `json.dumps()` or equivalent to serialize — never string concatenation.

### 7.3 Search Patterns

The INDEX.jsonl file supports a spectrum of query complexity using standard Unix tools:

```bash
# Simple keyword search
grep -i "retention" INDEX.jsonl

# Structured field query
jq 'select(.tags[] | contains("athena"))' INDEX.jsonl

# Find active documents in a domain
jq 'select(.status=="active" and .domain=="experimentation")' INDEX.jsonl

# Agent-friendly: grep then parse
grep "holdout" INDEX.jsonl | jq -r '.id'

# Token-budget-aware selection
jq 'select(.tokens < 500 and .priority <= 2)' INDEX.jsonl

# Dependency resolution
jq 'select(.depends_on[]? | contains("campaign-exposure"))' INDEX.jsonl

# List all documents by category
jq -r 'select(.category=="query") | "\(.id)\t\(.title)"' INDEX.jsonl

# Find documents with stale status
jq 'select(.status=="deprecated")' INDEX.jsonl
```

---

## 8. Indexing Process

### 8.1 Full Rebuild

A full rebuild walks the entire vault, reads every `.md` file's frontmatter, and produces a fresh INDEX.jsonl. This is the simplest mode and is appropriate for initial vault setup or when the index has become corrupted.

**Algorithm:**

1. Locate the vault root by walking up from the current directory to find `.kbroot`.
2. Recursively find all `.md` files in the vault, excluding INDEX.jsonl and `.kbroot`.
3. For each `.md` file, parse the YAML frontmatter between the opening and closing `---` delimiters.
4. Validate that the required fields (`id`, `title`, `description`) are present. Skip files missing any of these with a warning.
5. Compute the content hash: for sidecar files (those with a `source` field), hash the source file; for native markdown, hash the body after the frontmatter.
6. Serialize the frontmatter plus `file` and `hash` as a single-line JSON object using `json.dumps()`.
7. Write all lines to INDEX.jsonl, atomically replacing the previous index.

### 8.2 Incremental Rebuild

An incremental rebuild avoids re-processing unchanged files by comparing content hashes. This is the default mode for the `vaultli index` command and becomes important as the vault grows.

**Algorithm:**

1. Load the existing INDEX.jsonl into a dictionary keyed by `id`.
2. Walk all `.md` files in the vault as in the full rebuild.
3. For each file, compute the current content hash.
4. If the `id` exists in the current index and the hash matches, carry the existing index line forward unchanged.
5. If the hash differs or the `id` is new, re-parse the frontmatter and generate a new index line.
6. Remove index entries whose `id`s no longer correspond to existing `.md` files (pruning deleted documents).
7. Write the new INDEX.jsonl atomically.

### 8.3 Hash Computation

The content hash uses SHA-256, truncated to 12 hexadecimal characters (48 bits of collision resistance — far more than sufficient for any practical vault size).

**Critical design decision:** The hash is stored exclusively in INDEX.jsonl, never in the YAML frontmatter. This keeps source files as read-only inputs to the indexer and avoids the chicken-and-egg problem of a file's metadata describing its own content.

**For native markdown files**, the hash is computed from the file body after the closing `---` of the frontmatter:

```python
import hashlib

def content_hash(filepath: Path) -> str:
    text = filepath.read_text()
    parts = text.split("---", 2)
    body = parts[2] if len(parts) >= 3 else text
    return hashlib.sha256(body.encode()).hexdigest()[:12]
```

**For sidecar files** (those with a `source` field), the hash is computed from the source file's content, not from the sidecar's markdown body:

```python
def sidecar_hash(md_path: Path, source_field: str) -> str:
    target = md_path.parent / source_field
    return hashlib.sha256(target.read_bytes()).hexdigest()[:12]
```

This ensures that meaningful changes to the executable asset trigger re-indexing, while edits to the documentation prose alone do not.

### 8.4 Atomic Writes

The indexer must write INDEX.jsonl atomically to prevent corruption if the process is interrupted. The standard approach is to write to a temporary file (`INDEX.jsonl.tmp`) in the same directory, then rename it over the original. On POSIX systems, rename is atomic when source and destination are on the same filesystem.

```python
import tempfile, os

def atomic_write(path: Path, lines: list[str]):
    tmp = path.with_suffix(".tmp")
    with open(tmp, "w") as f:
        for line in lines:
            f.write(line + "\n")
    os.rename(tmp, path)
```

---

## 9. vaultli CLI Overview

This section outlines the initial command surface for vaultli version 1.0. Detailed command specifications, flag definitions, and output formats will be defined in a separate CLI reference document.

### 9.1 Core Commands

**`vaultli init`** — Initialize a new vault in the current directory. Creates the `.kbroot` marker file and an empty INDEX.jsonl. Fails if a vault root already exists in the current directory or any parent.

**`vaultli index`** — Rebuild the INDEX.jsonl. Performs an incremental rebuild by default. Accepts a `--full` flag to force a complete rebuild. Reports the number of files indexed, updated, pruned, and skipped.

**`vaultli search <query>`** — Search the index for documents matching the query. Runs grep under the hood for keyword matching. Accepts a `--jq` flag for structured queries. Returns formatted results showing `id`, `title`, and `description`.

**`vaultli add <file>`** — Add metadata to a file. For `.md` files, injects a frontmatter template at the top. For non-`.md` files, creates a sidecar `.md` file with pre-populated frontmatter including the `source` field. Computes and suggests the `id`. Runs the indexer on the new/modified file.

**`vaultli show <id>`** — Display the full metadata and file path for a document by its `id`. Resolves the `id` against INDEX.jsonl and pretty-prints the record.

**`vaultli validate`** — Audit the vault for integrity issues. Checks for: missing required fields, broken `source` references in sidecars, orphaned sidecar files, dangling `depends_on` and `related` references, duplicate `id`s, and index staleness (files modified since last index).

**`vaultli scaffold <file>`** — Auto-generate a sidecar or frontmatter stub for a file. Uses the filename, file extension, and optionally the file's content to infer sensible defaults for `category`, `tags`, `domain`, and `description`. Designed to be run by an agent that can fill in richer metadata after reading the file.

---

## 10. Future Considerations

The following capabilities are explicitly deferred from version 1.0 but are anticipated in the roadmap and have influenced the schema design.

### 10.1 Semantic Search Integration

The `tokens` field and the description-optimized retrieval design anticipate integration with a vector embedding layer. A future embedd daemon or sqlite-vec index could provide hybrid BM25 + vector retrieval, using the JSONL index for keyword filtering and a vector store for semantic similarity. The flat-file architecture does not preclude this — the JSONL index and a vector index can coexist as parallel retrieval paths.

### 10.2 Agent Context Assembly

The `tokens`, `priority`, `depends_on`, and `related` fields are specifically designed for a context-assembly algorithm that solves the knapsack problem: given a token budget, select the most relevant and important documents, resolve their dependencies, and pack them into a context window. This algorithm is a consumer of the vaultli index, not part of vaultli itself.

### 10.3 Multi-Vault Federation

The `scope` field and the path-based `id` scheme support a future where multiple vaults can be federated into a single search surface. Each vault maintains its own INDEX.jsonl; a federation layer merges them with vault-prefix disambiguation of `id`s.

### 10.4 Git Integration

Because the vault is a plain directory tree, it is already git-compatible. Future versions may integrate git metadata (commit history, blame) to auto-populate the `created` and `updated` fields, track authorship across contributors, and provide change history for individual documents.

---

*End of Specification*
