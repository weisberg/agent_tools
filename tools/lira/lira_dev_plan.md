# lira — Development Plan

**Source PRD:** `lira_prd_v0_4.md` (v0.4, schema_version 3)
**Plan version:** v1.1
**Last updated:** 2026-05-03
**Target:** Rust CLI delivering local-first agent ticketing with read-only Jira parents and bidirectional GitHub Issues sync.

This plan turns the PRD into ordered, verifiable engineering work. Each phase has a goal, scope, deliverables, public CLI surface, exit criteria, and tests. Phases roughly map to PRD §22 milestones (M1–M8) but are split where the work decomposes naturally.

---

## 0. Plan Conventions

- **One feature, one PR.** Every numbered task in this plan is a candidate PR boundary. If a numbered task grows beyond one reviewable change, split it by public CLI surface first, then by crate-internal plumbing.
- **Schema version is `3` from day one.** No migrations from earlier drafts; the field set in PRD §7 is the starting point.
- **YAML is canonical.** SQLite, `gh-cache/`, reverse links, and JSONL logs are derivable. Any test that asserts behavior must be reproducible from a clean YAML tree + `lira reindex`.
- **Every command ships `--json` in the same PR that adds the human form.** No follow-up "add JSON" tickets.
- **Every mutation appends history + JSONL log before returning success.** Canonical YAML and JSONL are the durable mutation record. The SQLite index is updated in the same command path when available, but index failures do not roll back YAML; they mark the index stale and are recoverable with `lira reindex` (PRD §16.2, FR-31).
- **Locks are mandatory** on every write path from the first ticket-mutating PR.
- **No `--force` shortcuts in code paths.** `--force` is a user-only escape hatch.
- **No interactive prompts in `--json` mode from day one.** Any command that would ask a question in human mode must return a structured error with `suggestions` in JSON mode (NFR-12).
- **Local-only commands stay offline.** They must not instantiate Jira or GitHub clients, check credentials, or inspect remote configuration (FR-3, NFR-15).
- **Sensitive data never enters canonical YAML or logs.** Store only metadata needed for references and sync markers; never store raw Jira/GitHub credentials (NG7, PRD §20).

### Definition of Done (per task)

A task is done when:

1. Code merged with passing CI.
2. `--json` output is stable and includes `schema_version: 3`.
3. Unit + integration tests cover happy path, validation failure, and error code surfaces.
4. Golden YAML/JSON fixtures committed where output shape is user-visible (PRD §23.3).
5. CLI help, generated docs, or a test name references the PRD/FR/GH/JIRA requirement it satisfies.
6. Any new mutating command proves, in a regression test, that an interrupted or failed write leaves canonical YAML valid.

### Risk Register (referenced throughout)

| Risk | Mitigation Phase |
|---|---|
| Deterministic YAML emitter availability in Rust ecosystem | Phase 1 spike (§1.2) |
| `gh` CLI surface stability across versions | Phase 5 transport abstraction (§5.1) |
| Three-way reconciliation correctness | Phase 6 conflict harness (§6.3) |
| Cross-platform locking semantics on Windows | Phase 1 lock spike (§1.5) |
| ETag/body-hash drift causing false conflicts | Phase 6 hash projection tests (§6.2) |
| Global `~/.lira/` writes during tests corrupting real user data | CC-1 `TestWorkspace` and mandatory home override |
| Agent-hostile partial JSON adoption | Phase 1 `JsonEnvelope` and error registry stub |
| Jira/GitHub auth checks leaking into local commands | Conventions + Phase 5/7 transport boundaries |

### Release Slices

Use these slices to keep work demonstrable even before the full MVP ships:

| Slice | Phases | User-visible capability |
|---|---|---|
| Walking skeleton | 1 | `lira --version --json`, `init`, empty project list, schema/error envelope, no real tickets |
| Local tracker | 2-3 | Project lifecycle, required AC/tasks, task updates, comments, links, claims |
| Fast local workspace | 4 | Indexed list/search/query/board with rebuildable SQLite |
| External context | 7 | Read-only Jira parent fetch/cache and parent-aware local queries |
| GitHub mirror | 5 | Bind/create/adopt/import plus explicit push/pull |
| Distributed sync | 6 | Three-way sync, conflict files, resolution |
| Agent-ready CLI | 8-9 | Pagination, schemas, docs, packaging, release binaries |

---

## Phase 1 — Foundations (PRD M1, week 1)

**Goal:** Workspace, schemas, store primitives, and the `MutationContext` machinery every later phase depends on.

### 1.1 Workspace scaffold

- Create Cargo workspace per PRD §15: `lira-core`, `lira-store`, `lira-index`, `lira-jira`, `lira-github`, `lira-format`, `lira-agent`, `lira-cli`, plus `xtask`.
- Wire `clap` in `lira-cli` with global flags: `--json`, `--yaml`, `--format`, `--project`, `--no-color`, `--quiet`, `--verbose` (PRD §12 preamble).
- Define output precedence now: `--json` and `--yaml` are mutually exclusive; `--format json|yaml|human` is the canonical internal representation; explicit `--json`/`--yaml` are aliases validated by `clap`.
- Add `cargo deny`, `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` to CI.
- Add a required test-only home override, e.g. `LIRA_HOME`, so integration tests never touch the developer's real `~/.lira/`.

**Exit:** `cargo build --workspace` produces an empty `lira` binary that prints version with `--json`.

### 1.1a JSON envelope and error registry stub

- Introduce `JsonEnvelope<T>` in the first CLI PR so every command, including `--version`, has the final outer shape: `schema_version`, `ok`, `result` or `error`.
- Introduce `LiraError` with stable `error_code`, `message`, `details`, and `suggestions` fields (PRD §18.1-§18.3).
- Add `xtask check-error-codes` as a stub that fails if an error variant lacks a registry entry. Phase 8 expands the schema generation, but the contract starts here.

### 1.2 Deterministic YAML emitter (spike + decision)

- Spike `serde_yaml` vs. a hand-rolled emitter that guarantees: stable key order matching schema declaration order, LF line endings, no anchor reuse, `|` block scalars for multi-line strings.
- Acceptance: same logical input round-trips byte-identically (NFR-6).
- Decision recorded in `crates/lira-store/docs/yaml-emitter.md`.

### 1.3 Core data model

In `lira-core`:

- Define `Ticket`, `Task`, `AcceptanceCriterion`, `Comment`, `HistoryEvent`, `Actor`, `ParentRef`, `GithubBinding`, `Labels`, `GithubLabel`, `TimeTracking`, `AgentMetadata`, `Timestamps` (PRD §7).
- Define `Project`, `Workflow`, `GlobalConfig` (PRD §8).
- Define enums: `TicketStatus`, `TaskStatus`, `SyncState`, `ParentType`, `FieldSyncMode`, `LabelStrategy`.
- Define newtype wrappers for IDs and external references: `ProjectKey`, `TicketId`, `TaskId`, `JiraKey`, `GithubRepo`, `GithubIssueNumber`, `AgentName`. Parsing belongs at the boundary; core code should not pass raw strings where a typed ID is available.
- Implement validators:
  - `validate_acceptance_criteria` — non-empty list, no whitespace-only entries (FR-7, §5.3).
  - `validate_tasks` — non-empty, unique IDs, no extra fields, valid status (FR-8 through FR-11, §7.5).
  - `validate_transition` — consults workflow allowed_transitions (FR-13).
  - `validate_completion_policy` — AC present + all tasks terminal (FR-14, §5.3 rule 5).
- Each validator returns a typed error mapping to a stable `error_code` (PRD §18.3).
- Add a schema fixture copied from PRD §7.1 and verify deserialize -> validate -> emit stays valid.

### 1.4 Store: paths, atomic writes, locks

In `lira-store`:

- `Paths` helper resolves `~/.lira/` or `LIRA_HOME` + project + status-directory layout (PRD §6).
- `atomic_write(path, bytes)` — temp file in same fs, fsync, rename (FR-28, §16.2).
- `advisory_lock(path)` — `fs2`-based, with timeout + `E_LOCK_UNAVAILABLE` (FR-27).
- `read_ticket(id) -> Ticket` and `write_ticket(Ticket)` enforcing deterministic emit.
- Enforce POSIX permissions through `lira-store::permissions` for every created directory/file; Windows records a warning-only capability flag until the Windows packaging phase verifies ACL behavior.
- Cross-platform spike: confirm advisory locks behave on macOS, Linux, Windows. Document fallback for Windows in `crates/lira-store/docs/locking.md`.

### 1.5 MutationContext

The single chokepoint every write path uses:

```rust
ctx.with_ticket_lock(id, |t| {
    t.apply(change)?;
    t.history.push(auto_event);
    t.timestamps.updated = now();
    Ok(())
})
.then_index_update()
.then_log_jsonl()
```

Guarantees:

1. Lock acquired before read.
2. Validation runs after mutation, before write.
3. History event auto-appended (FR-19).
4. `timestamps.updated` bumped (FR-12 for tasks, generally for ticket mutations).
5. JSONL log appended and flushed before success is returned (FR-29).
6. Index update attempted after canonical write; index failure marks stale state and returns a warning/error according to command policy, but never corrupts YAML (FR-31).
7. On any failure before the canonical write, the YAML on disk is unchanged.

Phase 1 implements the hook points even though the real SQLite index lands in
Phase 4. Until then, the index hook is a no-op that can be forced to fail in
tests.

### 1.6 JSONL logger

- Path: `~/.lira/logs/<UTC-DATE>.jsonl` (FR-29).
- One event per mutation. Schema matches PRD Appendix C.
- Logger flushes on each event; rotation is daily by filename.
- Secrets-redaction guard: refuse to log fields named `token`, `authorization`, `password` (PRD §20).

### 1.7 `lira init` and `lira doctor` (skeleton)

- `lira init` creates `~/.lira/`, `config.yaml`, `index/`, `gh-cache/`, `locks/`, `logs/` with `0700` permissions where supported (FR-1, §20).
- `lira init --dry-run --json` reports the paths it would create and whether `~/.lira/` already exists.
- `lira doctor` reports presence of root, projects, lock staleness, and version. Full validation lands in Phase 2.

**Phase 1 exit gates:**

- All §1.3 validators have unit tests including negative cases.
- A `MutationContext` test writes a synthetic ticket, simulates a mid-write panic, and proves the YAML on disk is unchanged.
- Round-trip golden test: load → emit → load is byte-stable.
- `LIRA_HOME` integration test proves `lira init` does not touch real `~/.lira/`.

---

## Phase 2 — Ticket lifecycle (PRD M1 completion, week 2)

**Goal:** Create, list, show, and move tickets with the full required-AC + required-tasks discipline.

### 2.1 `lira project create|list|show|archive`

- Allocates project directory tree including all status directories (FR-4).
- Writes `project.yaml`, `counters.yaml`, `workflow.yaml` from PRD §8 defaults.
- `--json` returns project key + counter state.
- `project create --dry-run --json` validates key, default workflow, and target paths without creating files.
- Project key validation is centralized in `ProjectKey`; counters are locked separately from ticket files so concurrent ticket creation cannot allocate duplicate IDs.

### 2.2 `lira new`

- Required flags enforced at parse time: `--acceptance-criterion` (≥1) and `--task` (≥1) — both repeatable (PRD §12.3, FR-7, FR-8).
- Allocates next ticket ID via project counter under project lock (FR-6).
- Writes ticket YAML to `tickets/<default_status>/<ID>.yaml` (FR-5).
- Auto-assigns `T1`, `T2`, … task IDs in flag order.
- Records `created` history event.
- `--parent-jira VAN-1234` populates `parent` block but does not fetch (Phase 7 handles fetch/cache).
- Supports `--description` and `--description-stdin` from the first implementation so agents do not have to pass large bodies through shell arguments (FR-30).
- If counter allocation succeeds but ticket write fails, the counter is not decremented. Gaps are allowed and logged; duplicate IDs are not.

### 2.3 `lira show` and `lira ls`

- `show <ID>` resolves child tickets, links, parent, GitHub binding (if any), task summary, comment count, history tail (FR-15).
- `show` must have `--raw` or equivalent only if needed for canonical YAML inspection; default JSON is the agent-facing projection, not the internal YAML struct.
- `ls` lists by project; supports `--status`, `--assignee`, `--label`, `--limit`, `--cursor` (NFR-14). Reads from filesystem until Phase 4 wires the index.

### 2.4 `lira mv`

- Validates target status exists, transition allowed (FR-13).
- For target `done`: enforces completion policy unless `--force` (FR-14, §5.3 rule 5; §16.3).
- Atomically renames YAML across status directories.
- Marks `github.sync_state = local-ahead` if a binding exists and state is in policy (deferred to Phase 5 wiring; struct field flipped here).
- Supports `--dry-run --json` to report the transition decision, completion-policy blockers, and destination path without moving the ticket.
- If moving across directories succeeds but post-write logging/indexing fails, `doctor` must still see status field and directory location in agreement.

### 2.5 `lira update` and `lira archive`

- `update` mutates non-status fields (`title`, `priority`, `description`, `assignee`, `reporter`).
- `archive` moves to `archived/`. `cancel` moves to `cancelled/`. No `rm`-like command exists (FR-32, NFR-7).
- `update` supports `--description-stdin`; long descriptions are not accepted through ad hoc temp files.
- Field-specific validators prevent local ticket updates from smuggling embedded task-only or GitHub-cache-only fields into canonical YAML.

### 2.6 `lira validate` and full `lira doctor`

- `validate`: schema, AC presence, task schema, transition correctness on touched tickets (FR-25, FR-26).
- `doctor`: status-vs-directory drift, duplicate IDs, stale locks, missing AC, invalid task fields, orphan files (acceptance §15).

**Phase 2 exit gates:**

- Integration test: create project → create ticket without AC → fails with `E_ACCEPTANCE_CRITERIA_REQUIRED`.
- Integration test: create with AC + tasks → move backlog → in-progress → done with all tasks `todo` → fails `E_COMPLETION_POLICY` → mark tasks done → succeeds.
- Golden ticket YAML matches PRD §7.1 example shape exactly.

---

## Phase 3 — Tasks, comments, history, links, agents (PRD M2, week 3)

**Goal:** Everything inside a ticket; everything between tickets locally.

### 3.1 Embedded task commands

`lira task add | list | show | status | tag add | tag remove | done | cancel` (PRD §12.5).

- Mutates `tasks[]` only; never spawns child tickets.
- Each mutation: bumps `timestamps.updated`, appends `task_added` / `task_status_changed` / `task_updated` history (FR-12, FR-19).
- `task add` rejects bodies that look like a sub-ticket (e.g. text >200 chars or containing `\n##` heading) with a soft warning suggesting `child add`.
- Schema enforcement: any field outside the six allowed (`id`, `title`, `status`, `tags`, `created_on`, `last_modified`) returns `E_INVALID_TASK_SCHEMA` (FR-10).
- `task status` validates allowed task statuses from project config, not only the default enum, while still enforcing the minimal task field set.
- `task cancel` is the only destructive-looking task operation. There is no `task rm`; cancellation preserves history (FR-32).

### 3.2 Comments

`lira comment <ID> <body>` and `--stdin` form (PRD §12.6).

- Allocates `local-c<N>` IDs (monotonic per ticket).
- `comment sync <ID> <comment-id> --github` flips `sync.github.push = true` (Phase 5 actually pushes).
- Append-only: no edit/delete commands in v1 (NG10, GH-28, GH-29).
- `comment list <ID>` exists in this phase and is paginated from the start; Phase 8 only formalizes schema docs.

### 3.3 History

- Auto-events flow through `MutationContext` from Phase 1.
- `lira history add` lets agents write structured `analysis_note`, `decision`, etc. with `--action`, `--message`, `--actor` (FR-18, §12.6).
- `lira history <ID>` prints the stream; `--json` returns the array.

### 3.4 Links and dependencies

`lira link` and `lira child add|remove` (PRD §12.7).

- `--jira VAN-1234` validates Jira key shape regex (FR-22).
- `--parent-lira ORION-12` typed parent + reverse link file under `links/lira/`.
- `--parent-github org/repo#100` typed parent (separate from peer binding — see PRD §5.7).
- `--blocks`, `--relates-to`, `--blocked-by`, `--duplicates` mutate the `links` map.
- Reverse-link files written under `projects/<P>/links/<type>/` (PRD §6).
- Link mutation uses ordered locking when two local tickets are touched; lock order is lexical by `TicketId` to avoid deadlocks.
- Removing a child link never deletes the child ticket.

### 3.5 Labels and tags

- `lira label add|remove` and `lira tag add|remove` operate on `labels.local` (PRD §10.1).
- `tag` is a CLI alias for `label` at the ticket level (FR-20).
- Task tags live separately on each task; managed via `lira task tag` from §3.1.

### 3.6 Agent assignment

`lira claim | release | active | next | summary` (PRD §12.4).

- `claim` fails with `E_LOCK_UNAVAILABLE` (or a dedicated `E_CLAIM_HELD`) when another agent owns the ticket; only `--force` overrides (PRD §19 rule 9).
- `next` returns the highest-priority unclaimed candidate filtered by project + agent; tie-break: `priority desc, created asc`.
- `active --agent <name>` lists owned tickets.
- Prefer a dedicated `E_CLAIM_HELD` for ownership conflicts; reserve `E_LOCK_UNAVAILABLE` for filesystem lock contention.
- `claim --force` requires an explicit `--reason` in human and JSON modes so the audit log explains the steal.

**Phase 3 exit gates:**

- Concurrent-claim integration test: two threads `claim` the same ticket; exactly one wins (FR-27).
- Comment + history JSONL log shape matches PRD Appendix C.
- `task add` with a 7th field via raw JSON input is rejected.
- Link-cycle test: `child add` rejects cycles with a stable error code.

---

## Phase 4 — Index, search, query, board (PRD M3, week 4)

**Goal:** SQLite/FTS5 cache; sub-100ms list/query at 1k tickets, sub-200ms at 10k (NFR-1, NFR-2).

### 4.1 SQLite schema

- `rusqlite` with bundled SQLite + FTS5.
- Tables: `tickets`, `tasks`, `task_tags`, `labels_local`, `labels_github`, `links`, `comments_meta`, `tickets_fts` (virtual FTS5).
- All columns indexed per PRD §17 list.
- Schema versioned; mismatch triggers automatic reindex.
- Store only projections needed for query. The index must not become a second canonical schema; every indexed row includes `source_path`, `source_mtime`, and canonical ticket ID for drift checks.

### 4.2 Index maintenance

- `MutationContext::then_index_update()` from Phase 1 now writes through to SQLite.
- Failure to update the index does **not** roll back the YAML write — index is rebuildable (FR-31). Instead it writes/updates `~/.lira/index/stale.json` and returns a structured warning or `E_INDEX_STALE` according to command policy.
- `lira reindex` rebuilds from YAML walk; safe to run anytime (acceptance §19).
- `lira doctor --json` includes `index_stale: true` and the stale reason when `stale.json` exists or source mtimes diverge from indexed mtimes.

### 4.3 Query commands

`lira ls`, `lira search`, `lira query`, `lira count`, `lira board` (PRD §12.10).

- `ls` switches to index-backed pagination with `--limit`, `--cursor`.
- `search "tokens"` runs against FTS5 index over title, description, AC, task titles, comments (PRD §17 item 20).
- `query --task-status blocked`, `--task-tag sql`, `--label rust`, `--status in-progress`, `--parent-jira VAN-1234`, `--github-repo owner/repo`, `--github-issue 142`, `--sync-state conflict` (GH-38, JIRA-5).
- `count --group-by status|priority|assignee|label`.
- `board --project ORION` returns kanban-shaped `{ status: [tickets] }`.
- All list/query outputs return compact ticket projections by default plus a documented `--verbose` expansion; agents can call `show` for full ticket bodies.

### 4.4 Performance gates

- Bench harness in `xtask bench` generates 1k and 10k synthetic ticket workspaces.
- CI runs `cargo test --release -- --include-ignored bench_ls_p95` and fails if NFR-1 / NFR-2 thresholds break.
- Bench datasets include comments, history, labels, links, task tags, and mixed sync states so the index is tested against realistic rows rather than title-only fixtures.

**Phase 4 exit gate:** All NFR-1, NFR-2, NFR-3 thresholds met locally on macOS reference machine; documented in `docs/perf-baseline.md`.

---

## Phase 5 — GitHub bridge: binding, one-way push/pull (PRD M5, week 6)

**Goal:** Local↔GitHub plumbing without conflict logic. Push and pull each work in isolation; sync arrives in Phase 6.

### 5.1 `gh` transport abstraction

In `lira-github`:

- `trait GhTransport` with methods: `issue_view`, `issue_create`, `issue_edit`, `issue_close`, `issue_reopen`, `issue_comment_create`, `issue_comment_list`, `labels_list`, `labels_create`, `auth_status`.
- Default impl shells out to `gh` (PRD §15.4); a `MockTransport` is the test backend.
- Pre-flight: every command requiring auth runs `auth_status` and surfaces `E_GH_NOT_INSTALLED` / `E_GH_AUTH` (GH-37).
- Local-only commands never construct a transport (NFR-15).
- Transport captures raw `gh` exit status, stderr, and parsed JSON separately so error mapping can distinguish auth, not found, permission, PR-not-issue, and rate-limit failures.
- Record minimum supported `gh` version in `docs/github-transport.md`; tests use fixture JSON rather than relying on a live `gh` binary.

### 5.2 Hash projections

- `local_hash` projection excludes volatile fields: `timestamps.updated`, `github.last_synced`, `github.remote_etag`, `github.remote_body_hash`, history entries with `action == github_sync` (PRD §9.2).
- `remote_body_hash` computed from normalized GitHub payload (sorted JSON keys, stripped trailing whitespace).
- Use an honest algorithm prefix such as `blake3:<hex>` or switch to SHA-256 and emit `sha256:<hex>`. The prefix must name the actual algorithm because agents may compare markers across versions.
- Add projection golden fixtures where volatile-only local changes do not alter `local_hash`, while synced-field changes do.

### 5.3 Binding commands

`lira gh link | unlink | status` (PRD §12.9 binding).

- `link` calls `issue_view` to validate existence, sets `github` block, initializes `last_synced`, `remote_etag`, `remote_body_hash`, `local_hash`, `sync_state: synced` (GH-3, GH-4).
- `link` rejects pull requests with `E_GH_PR_NOT_ISSUE` (GH-39).
- `unlink` clears the peer binding but keeps a `disabled` historical record under `sync/github/state.yaml` (PRD §6) and appends a history event.
- `status` summarizes binding for one or all tickets.

### 5.4 GitHub body rendering

- Reserved markdown sections: `## Description`, `## Acceptance Criteria`, `## Tasks` (PRD §9.4).
- Task list rendered as `- [ ] [T1] Title #tag1 #tag2`; `- [x]` when status is `done`.
- Parser is the inverse of the renderer; round-trip golden tests required.
- Parser preserves user-authored markdown outside reserved sections where policy allows body merge. If unsupported, it must surface a clear body-policy conflict instead of dropping content.
- Escaping rules for tags, brackets, and task titles are documented with examples.

### 5.5 Field policies

- Implement `FieldPolicy` engine reading from project config (PRD §8.4).
- Policies for: `title`, `body`, `state`, `labels`, `assignees`, `comments`, `milestones`.
- `state` mapping uses `state_reason_map` for `done → closed/completed`, `cancelled → closed/not_planned` (GH-21, GH-22).
- Label strategies: `union` (default), `mapped`, `local_only`, `remote_only`, `replace_remote` (GH-18).
- Auto-create remote labels gated on `auto_create_remote: true` (GH-19).
- Ignore-list filtering (GH-20) is applied before strategy resolution in both directions and covered by golden tests.
- Field policy decisions return an explainable plan object used by `--dry-run --json` for push, pull, and sync.

### 5.6 `lira gh create | adopt | import`

- `gh create <ID>` builds a GitHub issue from local fields including rendered body sections; binds the result (GH-5, GH-6).
- `gh adopt <repo>#<n>` extracts AC and tasks from body sections; if missing, **fails** unless `--acceptance-criterion`/`--task` flags supply them (GH-9).
- `gh import` adopts in bulk with `--state`, `--label`, `--acceptance-criteria-file`, `--task-template` (PRD §12.9 creating).
- PRs are detected and rejected with `E_GH_PR_NOT_ISSUE` unless a future `--include-prs` flag is added (GH-39).
- Local IDs are always freshly allocated; never reuse GitHub issue numbers (GH-35).

### 5.7 `lira gh push` and `lira gh pull`

- `push` writes local fields (per policy) to the bound issue. Idempotent when the local hash hasn't changed (GH-33).
- `pull` reads remote, applies policy, writes back to local YAML, updates markers.
- Both append `github_sync` history + JSONL events.
- Comments: `pull` adds new remote comments (GH-27); `push` pushes locally-flagged comments (GH-26). Edits/deletes are no-ops in v1 (GH-28, GH-29).
- Both support `--dry-run --json`, returning the field plan, comment plan, and expected marker changes without mutating either side.
- `push` refuses to overwrite a changed remote marker unless invoked through Phase 6 sync/resolution or explicit human `--force` (NFR-11).
- `pull` remote-driven local status changes validate workflow transitions; invalid transitions become structured conflicts rather than forced local moves (GH-24).

### 5.8 Labels and users

- `lira gh labels list|pull|push` (PRD §12.9 labels and users).
- `lira gh users map --add|--remove` writes to project config `assignees.user_map`.
- Unmapped local assignees on push: `E_GH_PERMISSION` style failure (GH-31). Unmapped remote on pull: stored as `gh:<login>` (GH-32).

**Phase 5 exit gates:**

- Mock-transport integration tests for create, adopt, import (with and without parseable body sections), push, pull, label sync, user mapping.
- Adopting a GH issue with no parseable AC/tasks and no flag overrides fails with `E_ACCEPTANCE_CRITERIA_REQUIRED` (GH-9).
- Round-trip: local ticket → `gh create` → `gh pull` → byte-identical body sections.
- Dry-run tests prove `gh push --dry-run --json` and `gh pull --dry-run --json` make zero transport mutation calls.

---

## Phase 6 — GitHub sync, conflicts, resolution (PRD M6, weeks 7–8)

**Goal:** Three-way reconciliation, conflict detection, and resolution.

### 6.1 `lira gh sync`

- Compares: current local hash vs. `local_hash` marker; current `remote_etag`/`remote_body_hash` vs. cached marker (PRD §9.3).
- Decision matrix from §9.3 implemented as a pure function `decide(local_changed, remote_changed, overlap) -> SyncAction`.
- `sync --all --project P` and `sync --repo owner/repo` iterate with project lock + ordered ticket locks (§16.1 batch sync).
- `sync` starts with a fresh `issue_view` and never trusts stale cache as the current remote state.
- `sync --all` has `--continue-on-error` defaulting to true for batch mode and emits per-ticket outcomes; single-ticket sync exits non-zero on the first failure.

### 6.2 Conflict detection

- Field-level overlap detection: a conflict requires both sides changed **and** changed the same policy-synced field (GH-14).
- Body field is treated as a single unit for conflict purposes; section-level merging is not v1.
- On conflict:
  1. Set `github.sync_state: conflict`.
  2. Write `~/.lira/gh-cache/conflicts/<ID>.diff` (using `similar` crate).
  3. Write `~/.lira/gh-cache/conflicts/<ID>.yaml` with structured metadata (fields, local_hash, remote_etag).
  4. Log conflict to JSONL.
  5. Exit non-zero with `E_GH_CONFLICT` (GH-14, GH-15).
- No local or remote field is mutated until resolution (GH-14, NFR-11).

### 6.3 Conflict harness (correctness)

- Property-based tests using `proptest`: random local + remote field-change vectors → assert decision matrix invariants.
- Golden tests: every cell of PRD §9.3 table backed by a fixture pair.

### 6.4 `lira gh conflicts | resolve | diff`

- `gh conflicts` lists tickets in conflict.
- `gh conflicts show <ID>` prints the structured metadata + diff path.
- `gh diff <ID>` prints the human diff inline.
- `gh resolve <ID> --prefer local|remote|--interactive` (PRD §12.9 conflicts).
- Resolution updates markers and emits a `conflict_resolved` history + JSONL event.
- `--interactive` is disabled in `--json` mode and returns `E_INTERACTIVE_DISABLED` with suggestions.
- `resolve --prefer local|remote` requires `--reason` unless the operation is running from an explicit human command in non-JSON mode. Agent-mode resolution should escalate by default (PRD §25.6).

### 6.5 Rate limit + backoff

- `gh` rate-limit errors map to `E_GH_RATE_LIMIT` and trigger exponential backoff with jitter (NFR-10).
- `--all` runs respect a per-run cap and surface a structured summary of skipped tickets.

### 6.6 Reopen / state-reason policy edge cases

- Closed-to-open remote transitions consult `github_link.state_reason_map` and project policy. Default behavior pending PRD open question §25.5: move local to `todo` when the workflow allows it and emit a `state_reopened` history note recommending human review (GH-23).
- All remote-driven status changes still validate against `allowed_transitions` (GH-24); failure surfaces `E_INVALID_TRANSITION` and converts the operation into a conflict.

**Phase 6 exit gates:**

- All 21 integration tests in PRD §23.2 pass under `MockTransport`.
- Conflict file shape matches a committed golden.
- A scripted "agent runs sync after long hiatus" scenario with 50 simulated tickets produces a structured conflict list rather than silent overwrites.

---

## Phase 7 — Jira read-only bridge (PRD M4, week 5; sequenced after Phase 4 in practice)

**Goal:** Read-only Jira parent context. Small phase; implement after Phase 4 or in parallel with Phase 5 if staffing allows. It is numbered after GitHub only to keep the larger sync work contiguous in the document.

### 7.1 Jira CLI delegation

- `lira jira fetch VAN-1234` shells out to a configured Jira CLI (PRD §8.3 `jira:` block).
- Caches title + URL under `projects/<P>/links/jira/<KEY>.yaml` (JIRA-6).
- Failure modes: missing CLI → `E_JIRA_NOT_INSTALLED`; auth → `E_JIRA_AUTH`. Local commands keep working (JIRA-3).
- Jira transport is read-only by type: expose `fetch_issue` and `auth_status`, no write methods. This keeps Jira write paths impossible rather than merely unimplemented.
- Cache records include `fetched_at`, source key, title, URL, and a short status summary only. Do not cache full confidential Jira bodies unless the PRD/config explicitly allows it.

### 7.2 Sync-parents and parent-aware queries

- `lira jira sync-parents` refreshes all cached parents in a project.
- `lira ticket list --parent-jira VAN-1234` enumerates lira children (JIRA-5).
- `ticket list --parent-jira` is an alias over the Phase 4 query path, not a separate scanner.
- `sync-parents --dry-run --json` lists the Jira keys it would fetch.

### 7.3 Non-goals enforcement

- No write paths to Jira exist in code (NG1, JIRA-2). A `lira jira push` command does not exist; `clap` should reject it as an unknown command rather than advertising a future write command.

**Phase 7 exit gate:** Round-trip `fetch` → `show <ID>` displays Jira title in parent block; `sync-parents` is idempotent.

---

## Phase 8 — Agent integration polish (PRD M7, week 9)

**Goal:** Make every agent-facing surface stable, paginated, and predictable.

### 8.1 Schema versioning

- All `--json` outputs continue to use the Phase 1 `JsonEnvelope` type; Phase 8 freezes and publishes the schemas rather than introducing the envelope late (PRD §18.1, §19 rule 1).
- Schema definitions are emitted to `docs/schemas/*.json` via `xtask gen-schemas`; CI verifies they're up to date (NFR-13).

### 8.2 Pagination and bounded output

- All collection commands (`ls`, `query`, `search`, `gh conflicts`, `history`, `comment list`, `task list`, `project list`, `gh status --all`, `jira sync-parents`) accept `--limit` where output can exceed one page and `--cursor` where ordering is stable (NFR-14, §19 rule 5).
- Cursor format: opaque base64-encoded `(last_id, last_sort_key)`.
- Cursor payloads include a query fingerprint so a cursor from one filter set cannot silently page through another.

### 8.3 Structured suggestions

- Every error code has a registry entry mapping to a list of `suggestions` strings (PRD §18.2).
- `xtask check-error-codes` ensures every variant in the `LiraError` enum has a registry entry.
- Suggestions are structured as short machine-readable actions plus human text, e.g. `{ "command": "lira doctor --json", "reason": "check workspace health" }`, not prose-only paragraphs.

### 8.4 Sync summaries

- `gh sync` and `gh sync --all` emit the structured `summary` shape from PRD §19 example.
- Long-running `--all` writes incremental progress to JSONL log so an agent can resume on interrupt.

### 8.5 Agent-safety guards

- `--json` mode disables every interactive prompt (NFR-12, §19 rule 4).
- Claim-stealing requires `--force`; the failure carries `suggestions` pointing to `lira active --agent <other>` (§19 rule 9).
- Add `lira agent preflight --json` to report tool version, workspace path, current agent ownership, stale index state, and missing optional transports without mutating anything.

**Phase 8 exit gate:** A scripted agent loop (claim → mv → task status → comment → gh push → mv done) runs end-to-end producing only structured JSON, with cursor-based pagination on every list.

---

## Phase 9 — Distribution (PRD M8, week 10)

### 9.1 Cross-builds

- `xtask release` produces stripped binaries for: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` (NFR-5).
- Size budget: under 10 MB stripped (NFR-4). Track in CI.

### 9.2 Packaging

- Homebrew tap formula in a separate repo with auto-bump from release tag.
- GitHub Releases with checksums.
- Shell completions for bash, zsh, fish, PowerShell via `clap_complete`.

### 9.3 Documentation

- `README.md` quickstart in the `lira` directory.
- `docs/cli-reference.md` generated from `clap` via `xtask gen-cli-docs`.
- `docs/agent-integration.md` aimed at agent authors; covers the JSON envelope, error codes, pagination, and an end-to-end loop.
- Maintain `SKILL.md` as the installable agent skill and treat `Lira-SKILL.pdf` as archival/generated material. Before release, verify `SKILL.md` against `docs/skills.md`, PRD §19, PRD §26, and Phase 8 contracts.
- Add a docs smoke test that extracts commands from `README.md`, `docs/agent-integration.md`, and `SKILL.md` fenced blocks and verifies each command is known to `clap` or explicitly marked illustrative.

---

## Cross-cutting Workstreams

These run across phases rather than belonging to one.

### CC-1 Test infrastructure

- Test ticket factory in `lira-core::testing` builds valid tickets in one line.
- `MockTransport` in `lira-github::testing` records every call for assertion.
- Golden file harness with `INSTA_UPDATE=auto` for snapshot review.
- A `tempdir`-backed `TestWorkspace` builds a fully-initialized `~/.lira/` clone per test.
- Integration tests must fail fast if `LIRA_HOME` is unset or points at the real home directory.
- Add crash-injection helpers around `MutationContext` steps: after lock, after validation, after temp write, after rename, after log append, after index update.

### CC-2 Observability

- Every mutating command and sync command writes a JSONL event even on no-op, with `result: "no_op"` so audit trails are continuous (NFR-9). Pure reads do not write logs unless explicitly run with a diagnostic flag.
- `lira logs tail [--project P]` reads today's JSONL with optional filtering. (Stretch; can ship in Phase 8.)

### CC-3 Security baseline

- Every file write goes through `lira-store::permissions` which enforces `0700`/`0600` on POSIX (PRD §20).
- Logger redaction unit-tested against a list of secret-shaped strings.
- `gh-cache/` documented as sensitive in README and excluded from default backups (`.lira-backup-ignore`).
- `doctor` warns when canonical files or cache directories are broader than user-only permissions on POSIX.
- Shelling out to `gh` or Jira CLI uses argument arrays, never shell strings.

### CC-4 Open question tracking

PRD §25 lists 12 open questions. Each gets a tracking issue at the start of the relevant phase:

| PRD Q | Phase | Default until resolved |
|---|---|---|
| §25.5 reopening semantics | Phase 6 | move to `todo` + warn (Phase 6.6) |
| §25.6 agent conflict resolution | Phase 6 | always escalate to human |
| §25.11 task completion policy | Phase 2 | fail unless explicit human `--force`; JSON mode returns blockers |
| §25.12 AC satisfaction model | Phase 2 | plain strings only |
| Others | Start of each relevant phase | open an issue before implementation starts |

---

## Phase Dependency Graph

```
Phase 1 (Foundations)
  └─> Phase 2 (Lifecycle)
        └─> Phase 3 (Tasks/Comments/Links/Agents)
              └─> Phase 4 (Index/Search)
                    ├─> Phase 5 (GH binding + push/pull)
                    │     └─> Phase 6 (GH sync + conflicts)
                    │           └─> Phase 8 (Agent polish)
                    └─> Phase 7 (Jira)
                          └─> Phase 8 (Agent polish)
                                └─> Phase 9 (Distribution)
```

---

## Top-level Acceptance Mapping

PRD §21.1 MVP Core (19 items) is fully covered by Phases 1–4.
PRD §21.2 GitHub (19 items) is fully covered by Phases 5–6.
PRD §22 milestones M1–M8 map to Phases 1+2, 3, 4, 7, 5, 6, 8, 9 respectively.
PRD §23 testing requirements are owned by CC-1 and exercised in each phase's exit gates.

When all phase exit gates pass, lira meets the v0.4 PRD's MVP definition (PRD §29).
