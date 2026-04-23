# vaultli

A CLI for managing a file-based knowledge vault with YAML frontmatter metadata and JSONL indexing. Every document in the vault gets a standardized metadata envelope that makes it discoverable, classifiable, and composable by AI agents operating under token-budget constraints.

Core principles: markdown files are the universal unit of knowledge, YAML frontmatter is the universal metadata format, JSONL is the universal index format. The vault is a directory tree of human-readable files — no databases, no proprietary formats. The `INDEX.jsonl` file is a derived cache, never the source of truth.

The v1.0 specification lives in `vaultli-spec-v1.0.md`. The short operational guide for agent use lives in `SKILL.md`.

## Agent-first summary

If a new agent is handed `vaultli` as a skill, the main thing it needs to understand is that `vaultli` manages metadata and index state, not the full knowledge-retrieval loop.

- `search` and `show` read `INDEX.jsonl`, which contains frontmatter-derived records plus `file` and `hash`.
- `search` does not search markdown bodies or the contents of non-markdown source assets.
- After finding a relevant record, the agent still needs to open the file named in `file`, and for sidecars often the `source` asset as well.
- `add` and `scaffold` create draft metadata quickly, but the inferred `title`, `description`, and `tags` are placeholders and usually need refinement before the vault is useful.
- `INDEX.jsonl` is a derived artifact. Do not hand-edit it; rebuild it with `index`.
- `validate` audits the vault and reports issues. It does not repair them automatically.

## Quickstart For A New Agent

Prefer the Python implementation for first use:

```bash
uv run python -m tools.vaultli --help
```

Recommended first-use loop:

```bash
# 1. See whether a vault already exists
uv run python -m tools.vaultli root .

# 2. Create one only if .kbroot is missing
uv run python -m tools.vaultli init ./kb

# 3. Add metadata to native markdown in one step
uv run python -m tools.vaultli add ./kb/docs/guide.md --root ./kb

# 4. Or create a sidecar for a non-markdown asset
uv run python -m tools.vaultli scaffold ./kb/queries/retention.sql --root ./kb

# 5. Edit the generated markdown to improve title/description/tags/body

# 6. Rebuild and validate
uv run python -m tools.vaultli index --root ./kb --json
uv run python -m tools.vaultli validate --root ./kb --json

# 7. Retrieve candidate records by metadata, then open the files yourself
uv run python -m tools.vaultli search retention --root ./kb --json
uv run python -m tools.vaultli show queries/retention --root ./kb --json
```

## Common First-use Mistakes

| Mistake | What vaultli actually does |
|---|---|
| "Search will find text inside the document body." | Search matches the indexed JSON record, which is derived from frontmatter plus `file` and `hash`. |
| "A `.sql` or `.j2` file will show up automatically." | Non-markdown assets are invisible until they have a same-directory sidecar like `query.sql.md`. |
| "The scaffolded metadata is good enough to keep." | Inferred metadata is intentionally generic. Retrieval quality depends heavily on rewriting `description`, `tags`, and often `category`. |
| "Editing `INDEX.jsonl` is a quick fix." | `INDEX.jsonl` is disposable cache state and should always be rebuilt, not edited manually. |
| "Changing sidecar prose updates the indexed content hash." | Sidecar hashes are based on the `source` asset bytes, not the sidecar body. |
| "`validate` will fix broken links or duplicate IDs." | `validate` only reports issues; the agent must repair the files and then re-run `index` and `validate`. |
| "`search --jq` is always available." | Structured filtering depends on `jq` being installed on the machine. |

## Recommended Agent Workflow

When using `vaultli` as part of a knowledge base implementation, this sequence is the safest default:

1. Discover the vault root with `root` or pass `--root` explicitly.
2. Use `add` for markdown files when you want scaffold + index in one step.
3. Use `scaffold` for non-markdown assets or when the generated metadata should be reviewed before indexing.
4. Improve the generated frontmatter immediately, especially `description`, `tags`, and relationship fields.
5. Run `index` after metadata edits.
6. Run `validate` before depending on the vault for retrieval.
7. Use `search` to shortlist candidates, then read the actual files.

## Implementations

vaultli has two implementations at full command parity:

| | Python (`py/`) | Rust (`rs/`) |
|---|---|---|
| **Role** | Reference implementation | Performance-oriented port |
| **Commands** | All 11 | All 11 |
| **Run** | `uv run python -m tools.vaultli ...` | `cd rs && cargo run -- ...` |

## Commands

| Command | Description |
|---|---|
| `init [path]` | Initialize a new vault (creates `.kbroot` marker + empty `INDEX.jsonl`) |
| `index [--full]` | Rebuild `INDEX.jsonl` (incremental by default, `--full` for complete rebuild) |
| `search <query> [--jq FILTER]` | Search the index by keyword or structured jq filter |
| `show <id>` | Display full metadata for a document by its id |
| `add <file>` | Add metadata to a file (scaffold + re-index in one step) |
| `scaffold <file>` | Create a frontmatter stub or sidecar `.md` file without re-indexing |
| `validate` | Audit vault integrity (broken sources, duplicates, stale index, dangling refs) |
| `root [path]` | Locate the nearest vault root by walking up from the given path |
| `make-id <file>` | Derive a vault id slug from a file path |
| `infer <file>` | Preview inferred scaffold metadata for a file without writing anything |
| `dump-index` | Dump all current index records as JSON |

All commands accept `--json` for structured JSON envelope output and `--root` to specify the vault root explicitly.

---

## Rust crate structure

```
rs/src/
  main.rs        — CLI entry point (clap) and output formatting
  lib.rs         — module declarations
  error.rs       — VaultliError enum
  model.rs       — data structs
  frontmatter.rs — YAML frontmatter parser
  paths.rs       — root discovery, path resolution, filesystem traversal
  id.rs          — vault ID generation
  infer.rs       — metadata inference from file properties
  index.rs       — INDEX.jsonl build, read, write, content hashing
  search.rs      — keyword search, jq filtering, record lookup
  scaffold.rs    — scaffold, add, init, YAML rendering
  validate.rs    — vault integrity checks
  util.rs        — shared constants and helpers
```

---

## Module reference

### `error.rs` — Error types

Defines `VaultliError`, a `thiserror`-derived enum covering every failure mode in the crate. Each variant carries context (usually a path or id string) and maps to a stable string code via `VaultliError::code()`.

#### `VaultliError` (enum)

Every variant, its error code, and when it fires:

| Variant | Code | Meaning |
|---|---|---|
| `RootNotFound(String)` | `ROOT_NOT_FOUND` | Walked all ancestors from the given path without finding `.kbroot` |
| `RootExists(String)` | `ROOT_EXISTS` | `init` was called but a `.kbroot` already exists in the target or a parent |
| `PathOutsideRoot(String)` | `PATH_OUTSIDE_ROOT` | A file path could not be made relative to the vault root (it's outside the vault) |
| `FileNotFound(String)` | `FILE_NOT_FOUND` | A file path was provided that does not exist on disk |
| `NotMarkdown(String)` | `NOT_MARKDOWN` | A function expected a `.md` file but received a different extension |
| `MalformedFrontmatter(String)` | `MALFORMED_FRONTMATTER` | An opening `---` was found but no closing `---` delimiter exists |
| `InvalidFrontmatter(String, String)` | `INVALID_FRONTMATTER` | Frontmatter exists but contains syntax errors (unexpected indentation, missing colon, unsupported block) |
| `IndexMissing(String)` | `INDEX_MISSING` | `INDEX.jsonl` does not exist at the vault root |
| `InvalidIndex` | `INDEX_INVALID` | A line in `INDEX.jsonl` parsed as valid JSON but was not a JSON object |
| `MissingRequiredFields(String)` | `MISSING_REQUIRED_FIELDS` | A document is missing one or more of `id`, `title`, `description` (or `source` for sidecars) |
| `BrokenSource(String, String)` | `BROKEN_SOURCE` | A sidecar's `source` field points to a file that does not exist |
| `IdNotFound(String)` | `ID_NOT_FOUND` | A `show` lookup found no record matching the requested id |
| `FrontmatterExists(String)` | `FRONTMATTER_EXISTS` | `scaffold` was called on a `.md` file that already has `---` frontmatter |
| `SidecarExists(String)` | `SIDECAR_EXISTS` | `scaffold` tried to create a sidecar `.md` but one already exists |
| `NotAFile(String)` | `NOT_A_FILE` | A directory was passed where a file was expected |
| `JqUnavailable` | `JQ_UNAVAILABLE` | `--jq` was used but the `jq` binary is not on `$PATH` |
| `JqFilterFailed(String)` | `JQ_FILTER_FAILED` | `jq` exited with a non-zero status; the variant carries stderr output |
| `JqFilterInvalid` | `JQ_FILTER_INVALID` | `jq` produced output that was not JSON objects (one per line) |
| `Unsupported(String)` | `UNSUPPORTED` | Internal guard for unexpected state (should not occur in normal usage) |
| `Io(std::io::Error)` | `IO_ERROR` | Any underlying filesystem I/O error (transparent wrapper) |
| `Json(serde_json::Error)` | `JSON_ERROR` | Any JSON serialization/deserialization error (transparent wrapper) |

#### `VaultliError::code(&self) -> &'static str`

Returns a stable, uppercase, underscore-separated string code for the error variant. Used by the CLI to populate the `error.code` field in JSON error envelopes. The code is deterministic and does not change between releases — callers can match on it programmatically.

---

### `model.rs` — Data structures

Pure data types with no business logic. All structs derive `Debug` and `Clone`. Serializable structs derive `serde::Serialize`.

#### `ParsedDocument`

Represents a single `.md` file after reading it from disk and splitting it into frontmatter metadata and body text.

| Field | Type | Description |
|---|---|---|
| `relative_path` | `String` | Path of the `.md` file relative to the vault root, using forward slashes |
| `metadata` | `Map<String, Value>` | Parsed YAML frontmatter as a JSON object. Keys are ordered per `FRONTMATTER_FIELD_ORDER`. Empty map if no frontmatter was present |
| `body` | `String` | Everything after the closing `---` delimiter. Full file content if no frontmatter |
| `has_frontmatter` | `bool` | `true` if the file started with `---` and had a valid closing `---` |

Methods:

- **`doc_id(&self) -> Option<&str>`** — Returns the value of the `id` key from `metadata` if it exists and is a string. Returns `None` if the key is missing or not a string. This is the document's canonical identifier used in `depends_on`, `related`, and index lookups.
- **`is_sidecar(&self) -> bool`** — Returns `true` if the relative path ends in `.md` and the stem (filename without `.md`) contains at least one dot. For example, `queries/report.sql.md` is a sidecar (stem `report.sql` contains a dot), while `docs/guide.md` is native markdown (stem `guide` has no dot).

#### `WarningRecord`

A non-fatal issue encountered during indexing. Warnings are collected into `IndexBuildResult.warnings` rather than aborting the index build.

| Field | Type | Description |
|---|---|---|
| `code` | `String` | Machine-readable warning code (e.g., `MISSING_REQUIRED_FIELDS`, `BROKEN_SOURCE`) |
| `message` | `String` | Human-readable description of the problem |
| `file` | `Option<String>` | Relative path of the file that triggered the warning. `None` if not file-specific. Omitted from JSON output when `None` |

#### `ValidationIssue`

A single integrity problem found by `validate_vault`. Derives `PartialEq`, `Eq`, `PartialOrd`, `Ord` so the final issue list can be sorted and deduplicated.

| Field | Type | JSON key | Description |
|---|---|---|---|
| `code` | `String` | `code` | Machine-readable issue code (e.g., `BROKEN_SOURCE`, `DUPLICATE_ID`, `STALE_INDEX`) |
| `message` | `String` | `message` | Human-readable description |
| `file` | `Option<String>` | `file` | Relative path of the affected file. Omitted from JSON when `None` |
| `doc_id` | `Option<String>` | `id` | Document id if known. Serialized as `id` in JSON. Omitted when `None` |
| `level` | `String` | `level` | Severity level. Currently always `"error"` |

#### `ValidationResult`

The complete output of `validate_vault`.

| Field | Type | Description |
|---|---|---|
| `root` | `String` | Absolute path to the vault root that was validated |
| `valid` | `bool` | `true` if zero issues were found |
| `issue_count` | `usize` | Total number of issues |
| `issues` | `Vec<ValidationIssue>` | Sorted, deduplicated list of all issues found |

#### `IndexBuildResult`

The complete output of `build_index`.

| Field | Type | Description |
|---|---|---|
| `root` | `String` | Absolute path to the vault root that was indexed |
| `full` | `bool` | `true` if a full rebuild was performed, `false` for incremental |
| `indexed` | `usize` | Number of new documents added to the index |
| `updated` | `usize` | Number of existing documents whose records changed (incremental mode only) |
| `pruned` | `usize` | Number of index entries removed because their source files no longer exist (incremental mode only) |
| `skipped` | `usize` | Number of unchanged documents carried forward without re-processing (incremental mode only) |
| `warnings` | `Vec<WarningRecord>` | Non-fatal issues encountered during indexing |

---

### `frontmatter.rs` — YAML frontmatter parser

A hand-rolled YAML subset parser purpose-built for vaultli frontmatter. It does not depend on `serde_yaml` — this is intentional. The parser handles exactly the YAML constructs that appear in vault frontmatter (scalars, inline lists, block lists, folded/literal block scalars) and rejects everything else with clear error messages. This avoids pulling in a full YAML library for a tightly constrained format.

#### `parse_frontmatter_text(text: &str, display_path: &str) -> Result<(Map<String, Value>, String, bool), VaultliError>`

The main entry point. Takes the full text content of a `.md` file and splits it into three parts:

1. **Metadata** (`Map<String, Value>`) — the parsed YAML frontmatter as a JSON-compatible map.
2. **Body** (`String`) — everything after the closing `---` delimiter.
3. **Has frontmatter** (`bool`) — whether the file contained valid frontmatter delimiters.

**Algorithm:**
- If the text does not start with `---\n`, returns immediately with an empty map, the full text as body, and `false`.
- Scans line-by-line after the opening `---` looking for a closing `---`.
- If no closing `---` is found, returns `MalformedFrontmatter`.
- Passes the lines between the delimiters to `parse_frontmatter_map` for key-value parsing.
- Everything after the closing `---` becomes the body string.

The `display_path` parameter is used only in error messages to identify which file had the problem.

#### `parse_frontmatter_map(lines: &[String], display_path: &str) -> Result<Map<String, Value>, VaultliError>` (private)

Iterates through the frontmatter lines and builds a `Map<String, Value>`. Handles four YAML constructs:

1. **Simple scalars** — `key: value`. The value is passed to `parse_scalar` for type inference.
2. **Block lists** — A key with an empty value followed by indented `- item` lines. Each item is passed through `parse_scalar`.
3. **Folded block scalars** — `key: >-` or `key: >` followed by indented continuation lines. Lines are joined with spaces (for folded) or newlines (for literal `|`), with empty lines filtered out in folded mode.
4. **Inline lists** — Detected by `parse_scalar` when the value is wrapped in `[...]`.

Rejects unexpected indentation at the top level and unsupported indented blocks with `InvalidFrontmatter`.

#### `parse_scalar(raw: &str) -> Value` (private)

Converts a raw string value into a typed `serde_json::Value`:

- `[a, b, c]` — parsed as `Value::Array` by splitting on commas and recursively calling `parse_scalar` on each element.
- `"quoted"` or `'quoted'` — stripped of quotes and returned as `Value::String`.
- Integer literals (e.g., `42`, `-1`) — parsed as `Value::Number`.
- `true` / `false` — parsed as `Value::Bool`.
- Everything else — returned as `Value::String` verbatim.

---

### `paths.rs` — Root discovery and path resolution

All functions that resolve, canonicalize, or traverse filesystem paths. These are the foundational building blocks used by every other module.

#### `find_root(start: Option<&Path>) -> Result<PathBuf, VaultliError>` (public)

Locates the vault root by walking up the directory tree from `start` (or the current working directory if `None`) looking for a `.kbroot` marker file. Returns the canonicalized absolute path of the directory containing `.kbroot`. Returns `RootNotFound` if no marker is found in any ancestor.

This follows the same pattern used by git (`.git`), cargo (`Cargo.toml`), and node (`package.json`) to establish project boundaries.

#### `resolve_root(root: &Path) -> Result<PathBuf, VaultliError>` (crate-internal)

Convenience wrapper. If `root` directly contains `.kbroot`, returns it canonicalized. Otherwise falls through to `find_root(Some(root))` to walk up. Used by most commands that accept a `--root` argument — handles both "this is the vault root" and "this is somewhere inside the vault" cases.

#### `canonicalize_or_join(path: &Path) -> Result<PathBuf, VaultliError>` (crate-internal)

Resolves a path to an absolute path using one of three strategies:
1. If the path exists on disk, returns `path.canonicalize()` (resolves symlinks, normalizes components).
2. If the path is absolute but doesn't exist yet (e.g., a path about to be created), returns it as-is.
3. If the path is relative and doesn't exist, joins it with the current working directory.

This handles the common case where a file hasn't been created yet but we need its absolute path for operations like `make_id`.

#### `relative_path(path: &Path, root: &Path) -> Result<String, VaultliError>` (crate-internal)

Computes the relative path from the vault root to a file. Both paths are resolved (via `resolve_root` and `canonicalize_or_join`) before computing the relative. The result uses forward slashes regardless of platform. Returns `PathOutsideRoot` if the file is not under the vault root.

#### `iter_markdown_files(root: &Path) -> Result<Vec<PathBuf>, VaultliError>` (crate-internal)

Recursively walks the vault directory tree starting from `root` and collects all `.md` files. Skips `INDEX.jsonl` (which lives at the root but is not a knowledge document). Returns the list sorted alphabetically by path for deterministic index ordering. Delegates to `visit_markdown` for the recursive traversal.

#### `visit_markdown(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), VaultliError>` (private)

Recursive directory walker. For each entry in `path`: if it's a directory, recurse into it. If it's a file named `INDEX.jsonl`, skip it. If it's a `.md` file, add it to the accumulator. All other files are ignored.

---

### `id.rs` — Vault ID generation

#### `make_id(file: &Path, root: &Path) -> Result<String, VaultliError>` (public)

Derives a stable, human-readable slug from a file's path relative to the vault root. The id serves as the permanent address for a document across the entire system — used in `depends_on` references, `related` links, and index lookups.

**Algorithm:**
1. Canonicalize both `file` and `root`.
2. Strip `root` from `file` to get the relative path. Return `PathOutsideRoot` if stripping fails.
3. Normalize path separators to forward slashes.
4. Strip the `.md` extension from the end.
5. If the remaining filename (after the last `/`) still contains a dot, it's a sidecar — strip the source extension too (e.g., `report.sql` becomes `report`).
6. Replace underscores and spaces with hyphens.
7. Lowercase the entire string.

**Examples:**

| File path (relative to root) | Generated ID |
|---|---|
| `docs/experimentation-guide.md` | `docs/experimentation-guide` |
| `queries/retention_holdout.sql.md` | `queries/retention-holdout` |
| `templates/campaign_report.j2.md` | `templates/campaign-report` |
| `skills/athena-analyst/SKILL.md` | `skills/athena-analyst/skill` |

---

### `infer.rs` — Metadata inference

Generates sensible default metadata for a file based on its name, extension, path components, and content. Used by `scaffold` and `add` to pre-populate frontmatter so the user (or agent) only needs to refine rather than author from scratch.

#### `infer_frontmatter(file: &Path, root: &Path) -> Result<Map<String, Value>, VaultliError>` (public)

The main entry point. Reads the file, runs all inference functions, and returns an ordered metadata map ready to be written as YAML frontmatter. Returns `FileNotFound` if the file doesn't exist.

**Fields populated:**
- `id` — via `make_id`
- `title` — via `infer_title`
- `description` — via `infer_description`
- `tags` — via `infer_tags`
- `category` — via `infer_category`
- `status` — always `"draft"`
- `created` — today's date in `YYYY-MM-DD` format (UTC)
- `updated` — same as `created`
- `tokens` — via `estimate_tokens` on the file's text content
- `priority` — always `3` (middle of the 1-5 range)
- `scope` — always `"personal"`
- `domain` — via `infer_domain` (only set if a known domain is detected in the path)
- `source` — set to `./filename` for non-`.md` files (sidecar convention). Omitted for native markdown.

The returned map is ordered per `FRONTMATTER_FIELD_ORDER`.

#### `infer_category(path: &Path) -> String` (private)

Determines the document category from the file extension and path components:

| Condition | Category |
|---|---|
| `.md` extension + filename is `skill.md` or path contains `skills/` | `skill` |
| `.md` extension + path contains `runbooks/` | `runbook` |
| `.md` extension (all other) | `note` |
| `.sql` extension | `query` |
| `.j2`, `.jinja`, or `.jinja2` extension | `template` |
| Everything else | `reference` |

#### `infer_title(path: &Path) -> String` (private)

Derives a human-readable title from the filename:
1. Takes the file stem (filename without the last extension).
2. For sidecar `.md` files (where the stem still contains a dot, like `report.sql`), strips the inner extension too to get `report`.
3. Replaces hyphens and underscores with spaces.
4. Title-cases each word (first character uppercased, rest preserved).

**Examples:** `retention_holdout.sql.md` becomes `Retention Holdout`. `campaign-report.j2.md` becomes `Campaign Report`.

#### `infer_description(path: &Path, category: &str, title: &str) -> String` (private)

Generates a one-sentence description based on the category:

| Category | Template |
|---|---|
| `query` | `"SQL query asset for {title} stored in the vault."` |
| `template` | `"Template asset for {title} stored in the vault."` |
| `skill` | `"Skill definition for {title} stored in the vault."` |
| `runbook` | `"Runbook documenting {title} for the vault."` |
| Everything else | `"Markdown document for {title} stored in the vault."` |

The title is lowercased in the output.

#### `infer_tags(path: &Path, category: &str) -> Vec<String>` (private)

Extracts tags from every component of the file path:
1. Splits each path component on dots, hyphens, and underscores.
2. Lowercases each token and deduplicates.
3. Appends the category if it's not already in the list.
4. Truncates to a maximum of 8 tags.

**Example:** `templates/campaign_report.j2` yields `["templates", "campaign", "report", "j2", "template"]`.

#### `infer_domain(path: &Path) -> Option<String>` (private)

Scans path components for a recognized knowledge domain. Returns the first match (lowercased, underscores replaced with hyphens), or `None` if no domain directory is found.

**Recognized domains:** `experimentation`, `marketing-analytics`, `infrastructure`, `tooling`, `finance`, `management`.

#### `estimate_tokens(text: &str) -> i64` (private)

Approximates the token count of a text string by counting whitespace-separated words and multiplying by 1.3 (a rough heuristic for English text with typical subword tokenization). Returns 0 for empty text. This estimate is stored in the `tokens` frontmatter field so context-assembly algorithms can solve the knapsack problem without tokenizing at query time.

---

### `index.rs` — Index build, read, and write

Manages the `INDEX.jsonl` file — the derived search index that lives at the vault root. Each line in the file is a self-contained JSON object representing one indexed document.

#### `parse_markdown_file(path: &Path, root: &Path) -> Result<ParsedDocument, VaultliError>` (public)

Reads a `.md` file from disk, parses its YAML frontmatter, and returns a `ParsedDocument`. This is the single point where raw files become structured data.

**Steps:**
1. Canonicalizes the path. Returns `FileNotFound` if it doesn't exist.
2. Returns `NotMarkdown` if the extension is not `.md`.
3. Reads the file text and passes it to `parse_frontmatter_text`.
4. Computes the relative path from the vault root.
5. Orders the metadata keys per `FRONTMATTER_FIELD_ORDER`.

#### `load_index_records(root: &Path) -> Result<Vec<Map<String, Value>>, VaultliError>` (public)

Reads `INDEX.jsonl` from the vault root and parses each line into a JSON object. Returns `IndexMissing` if the file doesn't exist. Returns `InvalidIndex` if any non-empty line is valid JSON but not a JSON object. Skips blank lines.

#### `build_index(root: &Path, full: bool) -> Result<IndexBuildResult, VaultliError>` (public)

Rebuilds the `INDEX.jsonl` file. The core indexing engine.

**Incremental mode** (`full = false`, the default):
1. Loads the existing index into a map keyed by document id.
2. Walks all `.md` files in the vault.
3. For each file, parses it and builds an index record.
4. If the record matches the existing entry (by deep equality), counts it as `skipped`.
5. If the record differs, counts it as `updated`.
6. If the id is new, counts it as `indexed`.
7. Any ids in the old index that no longer have corresponding files are counted as `pruned`.
8. Writes the new index atomically.

**Full mode** (`full = true`):
Same as incremental but skips the comparison step — every successfully parsed document counts as `indexed`. No `updated`, `pruned`, or `skipped` counts.

Files that fail to parse or lack required fields are recorded as warnings rather than aborting the build.

#### `build_index_record(root: &Path, path: &Path, document: &ParsedDocument) -> Result<Map<String, Value>, VaultliError>` (crate-internal)

Constructs a single index record from a parsed document. Validates that all required fields (`id`, `title`, `description`) are present, and that sidecars have a `source` field. Computes the content hash and appends the `file` (relative path) and `hash` fields to the metadata. Returns `MissingRequiredFields` if validation fails.

#### `compute_content_hash(path: &Path, document: &ParsedDocument) -> Result<String, VaultliError>` (private)

Computes a 12-character hex SHA-256 digest (48 bits of collision resistance) of the document's meaningful content.

- **For sidecar files** (those with a `source` field): hashes the bytes of the source asset file (e.g., the `.sql` file), not the sidecar's markdown body. This ensures that changes to the executable content trigger re-indexing.
- **For native markdown files**: hashes the body text after the frontmatter.

Returns `BrokenSource` if a sidecar's source file doesn't exist.

#### `write_index_records(root: &Path, records: &[Map<String, Value>]) -> Result<(), VaultliError>` (crate-internal)

Writes a list of index records to `INDEX.jsonl` atomically. Each record is serialized as a single-line JSON string. The file is written to `INDEX.jsonl.tmp` first, then renamed over the original to prevent corruption if the process is interrupted mid-write. On POSIX systems, `rename` is atomic when source and destination are on the same filesystem.

---

### `search.rs` — Search and record lookup

#### `show_record(root: &Path, doc_id: &str) -> Result<Map<String, Value>, VaultliError>` (public)

Loads the index and performs a linear scan for a record whose `id` field matches `doc_id` exactly. Returns the first matching record as a JSON object. Returns `IdNotFound` if no record matches.

#### `search_records(root: &Path, query: Option<&str>, jq_filter: Option<&str>) -> Result<Vec<Map<String, Value>>, VaultliError>` (public)

Searches the index using up to two filtering stages applied in sequence:

**Stage 1 — Keyword filtering** (when `query` is `Some`):
Serializes each index record to a JSON string, lowercases it, and checks if it contains the lowercased query string. This is a brute-force substring match across all fields — equivalent to `grep -i` on the JSONL file. Records that don't match are discarded.

**Stage 2 — jq filtering** (when `jq_filter` is `Some`):
1. Locates the `jq` binary on `$PATH` using `which("jq")`. Returns `JqUnavailable` if not found.
2. Serializes the remaining records as JSONL and pipes them to `jq -c <filter>` via stdin.
3. Captures stdout and parses each output line as a JSON object.
4. Returns `JqFilterFailed` if jq exits non-zero (carries stderr), or `JqFilterInvalid` if the output contains non-object values.

Both stages are optional. If neither is provided, returns all index records.

---

### `scaffold.rs` — Vault initialization, scaffolding, and file rendering

#### `init_vault(target: &Path) -> Result<Map<String, Value>, VaultliError>` (public)

Creates a new vault at the target directory:
1. Canonicalizes the target path.
2. Checks that no `.kbroot` exists in the target or any ancestor. Returns `RootExists` if one is found.
3. Creates the directory tree (including parents) if it doesn't exist.
4. Creates an empty `.kbroot` marker file.
5. Creates an empty `INDEX.jsonl` file.
6. Returns a map with `root`, `marker`, and `index` paths.

#### `scaffold_file(root: &Path, file: &Path) -> Result<Map<String, Value>, VaultliError>` (public)

Creates a metadata stub for a file without re-indexing. Handles two cases:

**For `.md` files (native markdown):**
1. Parses the file. Returns `FrontmatterExists` if it already has `---` frontmatter.
2. Infers metadata via `infer_frontmatter`.
3. Prepends the inferred frontmatter to the existing body content and writes the file back.
4. Returns with `mode: "frontmatter"`.

**For non-`.md` files (sidecar creation):**
1. Returns `SidecarExists` if a sidecar `.md` already exists for this file.
2. Infers metadata (which will include a `source` field pointing to the original file).
3. Creates a new sidecar file (`filename.ext.md`) with the inferred frontmatter and a placeholder body.
4. Returns with `mode: "sidecar"`.

Returns `FileNotFound` if the file doesn't exist, or `NotAFile` if it's a directory.

The return map includes: `root`, `mode` (`"frontmatter"` or `"sidecar"`), `file` (relative path of the written file), `id`, and `metadata` (the full inferred metadata object).

#### `add_file(root: &Path, file: &Path) -> Result<Map<String, Value>, VaultliError>` (public)

Convenience wrapper that calls `scaffold_file` followed by an incremental `build_index`. Returns the scaffold result plus the index build result nested under the `index` key.

#### `render_document(metadata: &Map<String, Value>, body: &str) -> String` (crate-internal)

Serializes a metadata map and body string into a complete markdown document with YAML frontmatter:

```
---
id: docs/guide
title: Guide
description: A helpful guide
tags:
  - docs
  - guide
---

Body content here.
```

Orders metadata keys per `FRONTMATTER_FIELD_ORDER` before rendering. Ensures a newline separates the closing `---` from the body if the body is non-empty.

#### `render_metadata_yaml(metadata: &Map<String, Value>) -> String` (private)

Converts a metadata map to YAML text (without the `---` delimiters). Handles three value types:
- **Arrays** — rendered as block lists with `  - item` lines.
- **Multiline strings** — rendered as folded block scalars with `>-` indicator.
- **Everything else** — rendered as `key: value` using `yaml_scalar` for formatting.

#### `default_sidecar_body(source_path: &Path) -> String` (private)

Generates the placeholder markdown body for a new sidecar file:

```markdown

## Purpose

Describe the purpose and usage of `filename.ext`.
```

---

### `validate.rs` — Vault integrity checks

#### `validate_vault(root: &Path) -> Result<ValidationResult, VaultliError>` (public)

Performs a comprehensive audit of the vault and returns all issues found. The validation runs in multiple passes:

**Pass 1 — File-level checks** (for each `.md` file in the vault):
- Parses the file. Parse failures become issues.
- For sidecars: checks that the sibling source asset exists (e.g., that `report.sql` exists next to `report.sql.md`). Emits `ORPHANED_SIDECAR` if not.
- Runs `document_validation_issues` for field-level checks.

**Pass 2 — Duplicate id detection:**
- Groups all parsed documents by their `id` field.
- Any id claimed by more than one file emits `DUPLICATE_ID` for each file.

**Pass 3 — Dangling reference checks:**
- Builds a set of "referenceable" ids (documents that have an id and pass `index_blocking_issues`).
- For each document, checks every entry in `depends_on` and `related` against this set.
- Emits `DANGLING_DEPENDENCY` or `DANGLING_RELATED` for references that don't resolve.

**Pass 4 — Index staleness:**
- Delegates to `index_staleness_issues` to compare the current index against the filesystem state.

The final issue list is sorted and deduplicated before returning.

#### `issue(code, message, file, doc_id) -> ValidationIssue` (private)

Helper constructor for `ValidationIssue`. Sets `level` to `"error"` for all issues.

#### `index_blocking_issues(path: &Path, document: &ParsedDocument) -> Vec<ValidationIssue>` (crate-internal)

Checks for problems that would prevent a document from being indexed:

1. **Missing required fields** — checks that `id`, `title`, and `description` are all present in the metadata. Emits `MISSING_REQUIRED_FIELDS` with the list of missing fields.
2. **Missing source field for sidecars** — if `is_sidecar()` is true and `source` is not in the metadata. Emits `MISSING_SOURCE_FIELD`.
3. **Broken source reference** — if `source` is present, resolves it relative to the file's directory and checks that the target exists. Emits `BROKEN_SOURCE` if not.

This function is also used by `index_staleness_issues` to filter out documents that can't be indexed when comparing against the existing index.

#### `document_validation_issues(path: &Path, document: &ParsedDocument) -> Vec<ValidationIssue>` (private)

Runs all index-blocking checks (via `index_blocking_issues`) plus additional type-validation checks:

- **List fields** (`tags`, `aliases`, `depends_on`, `related`) — must be JSON arrays if present. Emits `INVALID_FIELD_TYPE`.
- **String fields** (`id`, `title`, `description`, `category`, `author`, `status`, `source`, `scope`, `domain`) — must be JSON strings if present. Emits `INVALID_FIELD_TYPE`.
- **Integer fields** (`tokens`, `priority`) — must be JSON integers if present. Emits `INVALID_FIELD_TYPE`.
- **Priority range** — if `priority` is present and is an integer, must be between 1 and 5 inclusive. Emits `INVALID_PRIORITY`.
- **Date fields** (`created`, `updated`) — if present, must be strings parseable as `YYYY-MM-DD` via `chrono::NaiveDate`. Emits `INVALID_DATE` if not a string or not a valid date.

#### `index_staleness_issues(root: &Path, documents: &[(PathBuf, ParsedDocument)]) -> Result<Vec<ValidationIssue>, VaultliError>` (private)

Compares the current filesystem state against the existing `INDEX.jsonl`:

1. If `INDEX.jsonl` doesn't exist, returns a single `MISSING_INDEX` issue.
2. Loads the existing index into a map keyed by id.
3. Filters the document list to only valid, indexable, non-duplicate documents.
4. For each valid document, builds what the index record *should* be and compares it to the existing entry. Emits `STALE_INDEX` if they differ.
5. For each id in the existing index that has no corresponding valid document, emits `STALE_INDEX` (the index contains a record for something that no longer exists or is no longer valid).

---

### `util.rs` — Shared constants and helpers

Internal module (`pub(crate)`) providing constants and utility functions used across multiple modules.

#### Constants

| Constant | Type | Value | Used by |
|---|---|---|---|
| `VAULT_MARKER` | `&str` | `".kbroot"` | `paths`, `scaffold` |
| `INDEX_FILENAME` | `&str` | `"INDEX.jsonl"` | `paths`, `index`, `scaffold`, `validate` |
| `FRONTMATTER_FIELD_ORDER` | `&[&str]` | 18-element array defining the canonical key order | `util::order_metadata` |
| `REQUIRED_FIELDS` | `&[&str]` | `["id", "title", "description"]` | `index`, `validate` |
| `LIST_FIELDS` | `&[&str]` | `["tags", "aliases", "depends_on", "related"]` | `validate` |
| `STRING_FIELDS` | `&[&str]` | `["id", "title", "description", "category", "author", "status", "source", "scope", "domain"]` | `validate` |
| `INTEGER_FIELDS` | `&[&str]` | `["tokens", "priority"]` | `validate` |

#### `order_metadata(metadata: &Map<String, Value>) -> Map<String, Value>` (crate-internal)

Returns a new map with keys reordered to match `FRONTMATTER_FIELD_ORDER`. Keys in the order array come first (in order), followed by any extra keys not in the array (in their original insertion order). This ensures consistent YAML output and deterministic JSON serialization across rebuilds.

#### `map_from_pairs(pairs: Vec<(&str, Value)>) -> Map<String, Value>` (crate-internal)

Convenience constructor that builds a `Map<String, Value>` from a list of `(key, value)` pairs. Preserves insertion order. Used throughout `scaffold.rs` for building result maps.

#### `yaml_scalar(value: &Value) -> String` (crate-internal)

Converts a `serde_json::Value` to a string suitable for use as a YAML scalar value:
- **Strings** — returned verbatim, unless they're empty or contain characters that need quoting in YAML (`:`, leading `[`, leading `{`, leading `#`), in which case they're double-quoted with Rust debug formatting.
- **Numbers** — decimal string representation.
- **Booleans** — `"true"` or `"false"`.
- **Null** — `"null"`.
- **Arrays/Objects** — serialized as compact JSON (fallback, should not occur in normal frontmatter).

#### `which(binary: &str) -> Option<PathBuf>` (crate-internal)

Searches the `$PATH` environment variable for an executable with the given name. Returns the full path to the first match, or `None` if the binary is not found. Used by `search.rs` to locate the `jq` binary.

---

### `main.rs` — CLI entry point

The binary crate. Defines the `clap`-derived CLI parser and thin dispatch layer. Contains no business logic — all work is delegated to the library modules.

#### CLI structure

Uses `clap` with derive macros. The top-level `Cli` struct has a global `--json` flag and a `Commands` enum with one variant per subcommand. Each variant's fields map directly to the command's arguments.

#### `run(cli: Cli) -> Result<i32, (VaultliError, bool)>`

Dispatches the parsed command to the appropriate library function and wraps the result for output. Returns the process exit code (0 for success, 1 for errors, 1 for validation failure).

#### `emit_result(value: Value, as_json: bool)`

Formats and prints a successful result. In JSON mode, wraps the value in `{"ok": true, "result": ...}` and pretty-prints. In plain mode, prints key-value pairs from the top-level object.

#### `emit_error(error: &VaultliError, as_json: bool)`

Formats and prints an error to stderr. In JSON mode, emits `{"ok": false, "error": {"code": "...", "message": "..."}}`. In plain mode, prints `error [CODE]: message`.

#### `print_map(map: &Map<String, Value>)`

Plain-text formatter for JSON object maps. Arrays are rendered as comma-separated values. Strings are printed verbatim. Everything else uses the JSON representation.

---

## Validation

```bash
# Python tests
uv run pytest tests/test_vaultli.py

# Rust tests + build
cd tools/vaultli/rs && cargo test && cargo clippy && cargo build
```
