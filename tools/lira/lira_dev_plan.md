# lira — Development Plan

**Source PRD:** `lira_prd_v0_4.md` (v0.5, schema_version 3)
**Plan version:** v3
**Last updated:** 2026-05-06
**Target:** A single self-contained Rust binary at `tools/lira/` that delivers local-first agent ticketing, a Symphony-compatible local tracker/control-plane surface, read-only Jira parents, and bidirectional GitHub Issues sync.

This plan replaces the v1 multi-crate workspace plan. lira is a personal/internal CLI for one developer plus their agents. It is not a published library, has no downstream crate consumers, and does not need release-train pacing. The plan is sized for that.

New in v3: lira explicitly implements the tracker side of the Symphony model.
It does not become a resident scheduler, Codex runner, or workspace manager.
Instead, it exposes deterministic local issue projections, candidate selection,
atomic claims, blocker state, and workflow validation helpers that an external
runner can poll.

Reference spec: `https://github.com/openai/symphony/blob/main/SPEC.md`.

---

## 1. Architecture

### 1.1 Single crate, internal modules

One Cargo package, one binary. No workspace, no `xtask`, no `crates/*` directory. Internal separation comes from modules, not crate boundaries.

```text
tools/lira/
├── Cargo.toml                 # one [package], one [[bin]]
├── README.md
├── SKILL.md
├── lira_prd_v0_4.md
├── lira_dev_plan.md           # this document
├── src/
│   ├── main.rs                # ~30 lines: parse CLI, run, print envelope, exit
│   ├── cli.rs                 # clap derive: global flags, subcommand tree, dispatch
│   ├── commands/              # one module per command group; each returns Result<Value>
│   │   ├── mod.rs
│   │   ├── init.rs
│   │   ├── doctor.rs
│   │   ├── project.rs
│   │   ├── ticket.rs          # new, show, ls, mv, update, archive, validate
│   │   ├── task.rs
│   │   ├── comment.rs
│   │   ├── history.rs
│   │   ├── link.rs
│   │   ├── label.rs
│   │   ├── claim.rs           # claim, release, active, next, summary
│   │   ├── candidate.rs       # Symphony-compatible candidates and issue projections
│   │   ├── workflow_symphony.rs # tracker.kind=lira export/validate helpers
│   │   ├── search.rs          # search, query, count, board
│   │   ├── jira.rs
│   │   └── gh.rs
│   ├── output.rs              # JsonEnvelope<T>, --format selection, table render
│   ├── error.rs               # LiraError enum + stable error_code() + suggestions
│   ├── model/
│   │   ├── mod.rs
│   │   ├── ticket.rs          # Ticket, Task, AcceptanceCriterion, Comment, HistoryEvent, Actor
│   │   ├── issue.rs           # NormalizedIssue projection for orchestration
│   │   ├── project.rs         # Project, Workflow, GlobalConfig, FieldPolicy
│   │   ├── ids.rs             # TicketId, TaskId, ProjectKey, JiraKey, GithubRepo
│   │   └── validate.rs        # AC / task / transition / completion validators
│   ├── store/
│   │   ├── mod.rs             # public read_ticket / write_ticket / list_tickets
│   │   ├── paths.rs           # ~/.lira/ resolver honoring LIRA_HOME
│   │   ├── yaml.rs            # deterministic emit + strict parse
│   │   ├── atomic.rs          # temp+fsync+rename
│   │   ├── lock.rs            # fs2 advisory locks with timeout
│   │   └── perms.rs           # 0700/0600 enforcement on POSIX
│   ├── mutation.rs            # MutationContext: lock → mutate → validate → write → log
│   ├── log.rs                 # JSONL audit logger
│   ├── github/
│   │   ├── mod.rs
│   │   ├── client.rs          # shells out to gh; LIRA_GH_FIXTURES env enables replay
│   │   ├── policy.rs          # field policy engine
│   │   ├── body.rs            # render/parse reserved Markdown sections
│   │   ├── hash.rs            # local_hash + remote_body_hash projections
│   │   └── sync.rs            # three-way reconciliation engine
│   ├── jira/
│   │   ├── mod.rs
│   │   └── client.rs          # read-only fetch via configured CLI
│   ├── orchestration.rs       # candidate eligibility, priority mapping, blocker refs
│   ├── workflow_md.rs         # parse/validate the tracker subset of WORKFLOW.md
│   └── search.rs              # filesystem-backed query for v1 (index optional in M5)
└── tests/
    ├── common.rs              # TestWorkspace tempdir helper; gh fixture loader
    ├── lifecycle.rs
    ├── tasks_comments_history.rs
    ├── links_claims.rs
    ├── github_bind.rs
    ├── github_sync.rs
    ├── github_conflicts.rs
    ├── jira.rs
    └── snapshots/             # insta snapshots for goldens
```

### 1.2 Why single crate

A workspace with eight crates and an `xtask` is right when multiple consumers depend on different layers, when build times benefit from parallel crate compilation at scale, or when independent versioning matters. None of that applies here. Modules give the same logical separation, the same test ergonomics (`#[cfg(test)]` per module), and zero cross-crate friction (no version coordination, no public API design, no inter-crate test doubles).

### 1.3 Symphony boundary

lira implements:

- local ticket storage and validation,
- normalized issue projections for polling and prompt rendering,
- candidate eligibility and stable ordering,
- atomic claim/release as dispatch reservation,
- blocker/dependency resolution from local links,
- tracker-only `WORKFLOW.md` export/validation helpers,
- JSONL audit events for tracker mutations.

lira does not implement:

- a poll loop,
- Codex app-server process management,
- retry queues or exponential backoff timers,
- live session/token accounting,
- stall detection or worker cancellation,
- per-issue source-code workspace creation/cleanup.

### 1.4 Deviations from PRD §15 and NFRs

The PRD recommends a workspace and lists Windows as a target (NFR-5). The dev plan deliberately diverges:

| PRD/NFR | v1 build | Reason |
|---|---|---|
| §15 multi-crate workspace | Single binary crate with modules | Personal tool; no library consumers |
| NFR-5 Windows support | macOS + Linux only | No Windows users in scope; revisit if needed |
| §15.4 `gh` transport behind a trait | `GhClient` struct + `LIRA_GH_FIXTURES` env var | Fixture replay is enough for tests |
| §15 `xtask` | None | Cargo + a few `Makefile` targets cover release/bench |
| §22 weekly milestones | 4 untimed milestones | Single-developer pacing |
| Schema publication (`docs/schemas/*.json`) | None — `JsonEnvelope` documented in `SKILL.md` only | No external consumers of the JSON schema |

These deviations are noted here so a future maintainer doesn't read the PRD and conclude the build drifted by accident. Everything else in the PRD stands.

---

## 2. Conventions

- **Schema version is `3` from day one.** No migration paths from earlier drafts.
- **YAML is canonical.** `gh-cache/`, the optional SQLite index, and reverse-link files are all derivable from canonical YAML.
- **Every command supports `--json` from its first commit.** No follow-up "add JSON" work.
- **Every mutation goes through `MutationContext`** (§4.4). That's the single chokepoint that takes the lock, validates after the change, writes atomically, appends a history event, and writes one JSONL log line. There is no other write path.
- **No interactive prompts in `--json` mode.** A command that would prompt in human mode returns a structured error with `suggestions` instead.
- **Local-only commands stay offline.** They must not call `gh`, run `gh auth status`, or read remote configuration (FR-3, NFR-15).
- **Symphony-facing tracker commands stay offline.** `candidates`, `issue show/current`, and `workflow symphony export/validate` read local files only unless a future flag explicitly opts into remote enrichment.
- **lira is not the runner.** No code path launches Codex, creates source workspaces, or waits on live agent turns.
- **`--force` is for humans, never agents.** It exists in `clap` for `mv done`, `claim`, and `gh resolve` only. No internal code path uses `--force` to bypass invariants.
- **Errors carry stable `error_code` strings** like `E_TASK_REQUIRED`. Codes are an enum-backed string method, not a registry file.
- **Secrets never enter YAML, JSONL, or `gh-cache/`.** Tokens are owned by `gh` and `jira` CLIs.

### Definition of Done (per task)

1. Code merged with `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` green.
2. `--json` output stable; envelope includes `schema_version: 3` and `ok`.
3. Tests cover happy path + at least one validation failure with the expected `error_code`.
4. If output shape is user-visible, an `insta` snapshot is committed.

That's it. No "PRD section referenced in test name" requirement — code review covers spec alignment.

### Risk Register

| Risk | Mitigation |
|---|---|
| Deterministic YAML emission in Rust | M1 spike (§3.3); fall back to a hand-rolled emitter on top of `serde_yaml` events if needed |
| `gh` CLI behavior drift across versions | Pin a minimum `gh` version in README; tests use fixture replay, not live `gh` |
| Three-way reconciliation correctness | Build `decide(local, remote, last_synced)` as a pure function with table-driven tests covering every cell of PRD §9.3 |
| Hash projection drift causing false conflicts | Golden tests for `local_hash` over volatile-only mutations (must be stable) and synced-field mutations (must change) |
| `~/.lira/` writes during tests corrupting real data | `LIRA_HOME` env override is required by `tests/common.rs::TestWorkspace`; integration tests panic if it's unset |
| Lock contention deadlocks during link/sync ops touching two tickets | Always lock `TicketId`s in lexical order; one helper enforces it |
| Duplicate dispatch from concurrent runners | Make `claim` the only reservation primitive; add a two-thread integration test with exactly one winner |
| Orchestration reads accidentally mutate state | Keep `candidates`/`issue current` pure reads; golden tests assert YAML and log directories are unchanged |
| Tracker-kind drift from upstream Symphony | Treat `tracker.kind: lira` as a documented local extension; helper validation ignores unknown runner-owned keys |

### Release Slices

| Slice | Milestone | What you can do |
|---|---|---|
| Walking skeleton | M1 partial | `lira init`, `lira project create`, `lira new` with required AC/tasks, `lira show`, `lira ls`, JSON envelope works |
| Local tracker | M1 complete + M2 | Full local lifecycle: `mv`, completion guard, tasks, comments, history, links, labels, claims |
| Symphony tracker | M3 | `candidates`, normalized issue projection, blocker-aware sorting, atomic dispatch claims, `WORKFLOW.md` tracker export/validate |
| GitHub mirror | M4 | Bind, create, adopt, import, push, pull, three-way sync, conflicts, resolve |
| Polish | M5 | Jira read-only, optional SQLite index, agent preflight, install story |

---

## 3. Milestone 1 — Local foundations and lifecycle

**Goal:** Everything needed to create, list, show, and move tickets locally, including the completion guard. After M1 you have a usable local tracker for one project at a time.

### 3.1 Project scaffold

- `cargo init --bin` at `tools/lira/`.
- `Cargo.toml` deps: `clap` (derive), `serde`, `serde_yaml`, `serde_json`, `time` (or `chrono`), `directories`, `fs2`, `thiserror`, `anyhow`, `blake3`, `similar`, `tempfile`, `comfy-table`, `regex`, plus dev-deps `insta`, `assert_cmd`, `predicates`.
- `clap` global flags: `--json`, `--yaml`, `--format human|json|yaml`, `--project`, `--no-color`, `--quiet`, `--verbose`. Output flags resolve to a single `Format` enum with documented precedence (explicit `--json` wins).
- CI: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.

### 3.2 Output envelope and error type

`src/output.rs` defines:

```rust
#[derive(Serialize)]
pub struct JsonEnvelope<T> {
    pub schema_version: u32,         // always 3
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}
```

`src/error.rs` defines `LiraError` (a `thiserror` enum) with `fn error_code(&self) -> &'static str` returning stable codes (PRD §18.3 list). `Display` text is the human message. `suggestions(&self) -> Vec<Suggestion>` returns inline static strings — no registry file, no validation tool. The full mapping is one match expression.

### 3.3 Deterministic YAML

`src/store/yaml.rs` exposes `read_ticket(path)` and `write_ticket(path, &Ticket)`.

Spike `serde_yaml` first. Acceptance: round-trip `tests/snapshots/sample_ticket.yaml` (a copy of PRD §7.1) byte-identically. If `serde_yaml` reorders keys or breaks block scalars, fall back to a small custom emitter that walks the model in declaration order. Document the choice in a comment at the top of `yaml.rs`.

Either way: stable key order, LF line endings, `|` block scalars for multi-line strings, no anchors.

### 3.4 Paths, atomic writes, locks

`src/store/`:

- `paths.rs`: `Paths::resolve()` honors `LIRA_HOME` first, then `~/.lira/`. Provides typed accessors (`paths.ticket(project, status, id)`, `paths.lock(id)`, `paths.log_today()`).
- `atomic.rs`: `atomic_write(target, bytes)` writes to `target.tmp.<rand>` in the same directory, fsyncs, renames.
- `lock.rs`: `with_lock(path, timeout, f)` using `fs2::FileExt::try_lock_exclusive`. Returns `E_LOCK_UNAVAILABLE` on timeout. Lock files live in `~/.lira/locks/`.
- `perms.rs`: `chmod_user_only(path)` enforces `0700`/`0600` on Unix; on macOS/Linux only. No-op on other platforms.

### 3.5 Data model and validators

`src/model/`:

- `ticket.rs`: full PRD §7 schema as serde structs. `Task` is locked to exactly six fields via `#[serde(deny_unknown_fields)]`; the other types use `deny_unknown_fields` too so unknown YAML keys fail loudly.
- `ids.rs`: newtypes (`TicketId`, `ProjectKey`, …) with `FromStr` parsing at command boundaries. Internal code never passes raw strings where a typed ID exists.
- `validate.rs`:
  - `validate_acceptance_criteria(&[String]) -> Result<()>` — non-empty, no whitespace-only entries (`E_ACCEPTANCE_CRITERIA_REQUIRED`).
  - `validate_tasks(&[Task]) -> Result<()>` — non-empty, unique IDs, valid status (`E_TASK_REQUIRED`, `E_INVALID_TASK_SCHEMA`, `E_INVALID_TASK_STATUS`).
  - `validate_transition(&Workflow, from, to) -> Result<()>` — uses `allowed_transitions` (`E_INVALID_TRANSITION`).
  - `validate_completion_policy(&Ticket) -> Result<()>` — AC present + all tasks terminal (`E_COMPLETION_POLICY`).

### 3.6 MutationContext

`src/mutation.rs` is the single write chokepoint:

```rust
pub fn mutate_ticket<F>(paths: &Paths, id: &TicketId, change: F) -> Result<Ticket>
where
    F: FnOnce(&mut Ticket) -> Result<HistoryEvent>,
{
    with_lock(paths.lock(id), TIMEOUT, || {
        let mut ticket = read_ticket(paths.ticket_for(id)?)?;
        let event = change(&mut ticket)?;
        validate(&ticket)?;
        ticket.timestamps.updated = now();
        ticket.history.push(event.clone());
        write_ticket(paths.ticket_for(&ticket)?, &ticket)?;   // atomic
        log::append(LogEntry::from(&event, &ticket))?;
        Ok(ticket)
    })
}
```

Guarantees:

1. Lock before read.
2. Validation runs after the closure mutates the ticket.
3. YAML write is atomic; failure leaves the previous version in place.
4. JSONL log line is appended only after the canonical write succeeds.
5. The closure cannot return without producing a `HistoryEvent` — auto-history is structurally enforced.

Status moves use a sibling `move_ticket` helper that atomically renames between `tickets/<from>/` and `tickets/<to>/` directories under the same lock.

### 3.7 JSONL log

`src/log.rs`:

- One file per UTC day at `~/.lira/logs/YYYY-MM-DD.jsonl` (FR-29).
- Append + fsync per line. Daily rotation by filename.
- Entry shape matches PRD Appendix C.
- `redact()` helper strips any field whose key matches `/token|authorization|password|secret/i` before serialization.

### 3.8 Commands

| Command | What it does |
|---|---|
| `lira init` | Creates `~/.lira/` tree (or `LIRA_HOME`), `config.yaml`, `index/`, `gh-cache/`, `locks/`, `logs/` with user-only perms. Idempotent. |
| `lira doctor` | Reports root presence, project list, lock staleness, version. Full validation lands in M2. |
| `lira project create|list|show|archive` | Allocates project tree (all status dirs), writes `project.yaml`, `counters.yaml`, `workflow.yaml` from PRD §8 defaults. |
| `lira new` | Required `--acceptance-criterion` (≥1) and `--task` (≥1) at `clap` level. Allocates next ID under project lock. Auto-assigns `T1`, `T2`, …. Emits `created` history. Supports `--description-stdin`. |
| `lira show <ID>` | Resolves task summary, parent, GitHub binding, comment count, history tail. |
| `lira ls` | Filesystem walk filtered by `--status`, `--assignee`, `--label`, `--limit`, `--offset`. (Cursor pagination is M5 if needed.) |
| `lira mv <ID> <status>` | Validates transition. For target `done`, runs `validate_completion_policy` unless `--force`. Atomic rename + history. |
| `lira update <ID>` | Mutates `title`, `priority`, `description`, `assignee`, `reporter`. |
| `lira archive <ID>` | Moves to `archived/`. No `rm`. |
| `lira validate` | Schema + AC + tasks + transition correctness across the workspace. |

### M1 exit gates

- `cargo test` green.
- A `MutationContext` test triggers a panic mid-mutation (via test-only injection point) and proves canonical YAML on disk is unchanged.
- Round-trip golden: `tests/snapshots/sample_ticket.yaml` (PRD §7.1) parses → emits byte-identically.
- Integration test: create ticket without AC → fails with `E_ACCEPTANCE_CRITERIA_REQUIRED`. Create with AC + tasks → `mv done` while tasks are `todo` → fails with `E_COMPLETION_POLICY`. Mark tasks done → `mv done` succeeds.
- `LIRA_HOME` test confirms no test touches `~/.lira/`.

---

## 4. Milestone 2 — Inside the ticket and between tickets

**Goal:** All ticket-internal mutations (tasks, comments, history) and all local cross-ticket relationships (links, child tickets, labels, agent claims). After M2 you have a complete local tracker.

### 4.1 Embedded tasks

`src/commands/task.rs`:

- `lira task add | list | show | status | tag add | tag remove | done | cancel`.
- All mutations route through `MutationContext`. Each appends one of: `task_added`, `task_status_changed`, `task_updated` history.
- `Task` struct has `#[serde(deny_unknown_fields)]`; CLI rejects any attempt to write extra fields with `E_INVALID_TASK_SCHEMA`.
- `task add` warns (not errors) when title is `>200` chars or contains `\n##` — soft hint to use `child add`.
- No `task rm`. Cancellation only.

### 4.2 Comments

`src/commands/comment.rs`:

- `lira comment <ID> <body>` and `--stdin` form.
- IDs are `local-c1`, `local-c2`, … monotonic per ticket.
- `lira comment sync <ID> <comment-id> --github` flips `sync.github.push = true`. M4 actually pushes.
- No edit/delete commands in v1.

### 4.3 History

- Auto-events flow through `MutationContext`; not an extra command.
- `lira history <ID>` prints the stream (`--json` returns the array).
- `lira history add <ID> --action <name> --message <text> --actor <name>` lets agents record `analysis_note`, `decision`, etc.

### 4.4 Links and dependencies

`src/commands/link.rs`:

- `lira link <ID> --jira VAN-1234`: stores typed Jira parent (no fetch yet).
- `lira link <ID> --parent-lira ORION-12`: typed lira parent + reverse-link file at `projects/<P>/links/lira/`.
- `lira link <ID> --parent-github org/repo#100`: typed GitHub tracking parent (separate from M4 peer binding).
- `lira link <ID> --blocks/--blocked-by/--relates-to/--duplicates <OTHER>`: mutates the `links` map.
- `lira child add <PARENT> <CHILD>` / `child remove`. Cycle detection rejects with a stable error code.
- Two-ticket mutations acquire locks in lexical `TicketId` order via a `with_two_locks` helper.

### 4.5 Labels and tags

`src/commands/label.rs`:

- `lira label add | remove` (alias `tag` at the ticket level) operates on `labels.local`.
- Task tags are managed separately by `lira task tag` from §4.1.
- `labels.github` is read-only locally until M4 syncs it.

### 4.6 Agent assignment

`src/commands/claim.rs`:

- `lira claim <ID> --agent <name>` writes `assignee` and emits `claimed` history. Fails with `E_CLAIM_HELD` if another agent owns it. `--force` requires `--reason`; the reason goes into history and the JSONL log.
- `lira release <ID>` clears `assignee`, emits `released`.
- `lira active --agent <name>` lists currently-owned tickets.
- `lira next --project P --agent <name>` returns highest-priority unclaimed candidate. Tie-break: priority desc, then `timestamps.created` asc.
- `lira summary --project P` returns counts by status, blocked count, recent activity.

### M2 exit gates

- Concurrent-claim integration test: two threads racing `claim` produce exactly one winner and one `E_CLAIM_HELD`.
- Cycle test: `child add A B` then `child add B A` rejects with a stable error code.
- Comment + history JSONL shapes match PRD Appendix C (insta snapshots).
- `task add` with extra fields injected via raw YAML on disk → `lira show` returns `E_INVALID_TASK_SCHEMA` from the validator.

---

## 5. Milestone 3 — Symphony-compatible local tracker

**Goal:** Expose lira as a local-first issue tracker for a Symphony-style runner. After M3, an external daemon can poll local candidates, claim exactly one ticket per run, render prompts from normalized issue data, reconcile current states, and write progress back through existing ticket commands.

### 5.1 Normalized issue projection

`src/model/issue.rs` defines:

```rust
pub struct NormalizedIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<u8>,
    pub state: String,
    pub branch_name: Option<String>,
    pub url: Option<String>,
    pub labels: Vec<String>,
    pub blocked_by: Vec<BlockerRef>,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}
```

Projection rules:

- `id` and `identifier` default to the lira ticket ID.
- `priority` maps `highest..lowest` to `1..5`; unknown becomes `None`.
- `state` is the ticket `status`.
- `labels` are normalized lowercase local labels, with GitHub labels included only when configured.
- `blocked_by` resolves local tickets where possible and returns null fields for unresolved external blockers.
- `created_at` and `updated_at` come from `timestamps`.

### 5.2 Candidate eligibility

`src/orchestration.rs` exposes:

```rust
pub fn candidate_issues(project: Option<ProjectKey>, filters: CandidateFilters) -> Result<Vec<NormalizedIssue>>;
pub fn is_candidate(ticket: &Ticket, workflow: &Workflow, blockers: &[BlockerRef]) -> CandidateDecision;
```

Eligibility mirrors PRD §12.11:

- status is active and not terminal,
- ticket is unclaimed when `exclude_claimed` is true,
- `orchestration.active_for_dispatch != Some(false)`,
- `todo` tickets with non-terminal local blockers are excluded,
- required issue fields exist.

Sorting is priority ascending, `created_at` ascending, then identifier lexicographic. Tests cover null priority and missing timestamps.

### 5.3 Commands

`src/commands/candidate.rs`:

- `lira candidates --project P [--state S] [--limit N] [--json]`
- `lira issue show <ID> --json`
- `lira issue current --ids <ID>... --json`

These commands are pure reads. They must not append logs, touch ticket YAML, allocate cursors, or acquire write locks.

`lira issue current` is the reconciliation helper: it returns the current normalized issue projection for each requested ID plus a structured not-found entry for missing tickets.

### 5.4 Claim as dispatch reservation

The M2 `claim` command becomes the only supported local dispatch reservation primitive.

Changes in M3:

- `lira claim <ID> --agent <runner>` adds `--reason dispatch` as an optional structured reason.
- JSON output includes `{ claimed: true, previous_owner: null, issue: NormalizedIssue }`.
- `E_CLAIM_HELD` details include the current owner and `claimed_at`.
- Concurrent claim tests are extended to run through `candidates → claim`, not claim alone.

No lease/expiry lands in M3. Long-held claims are visible through `active` and released manually or by a human-approved `--force --reason`.

### 5.5 `WORKFLOW.md` tracker helpers

`src/workflow_md.rs` parses Markdown with optional YAML front matter:

- If the file starts with `---`, parse front matter until the next `---`.
- Front matter must be a YAML map.
- lira validates only `tracker` fields it understands.
- Unknown top-level keys and runner-owned keys are ignored.

`src/commands/workflow_symphony.rs`:

- `lira workflow symphony export --project ORION --json`
- `lira workflow symphony validate ./WORKFLOW.md --project ORION --json`

`export` prints a suggested `tracker.kind: lira` block and never writes files unless a future explicit `--write` is added.

`validate` checks:

- `tracker.kind == "lira"`,
- project exists,
- configured active/terminal states exist in `workflow.yaml`,
- no terminal state is also active.

### M3 exit gates

- `candidates` returns only unclaimed active tickets and excludes terminal tickets.
- `candidates` excludes `todo` tickets blocked by non-terminal local blockers.
- Golden JSON for `NormalizedIssue` includes blocker refs and null priority handling.
- Repeated `candidates` and `issue current` calls leave YAML and JSONL logs unchanged.
- Race test: two simulated runners call `candidates → claim`; exactly one owns the ticket.
- `workflow symphony export` emits a stable `tracker.kind: lira` block.
- `workflow symphony validate` accepts a valid file and ignores `workspace`, `hooks`, `agent`, and `codex`.

---

## 6. Milestone 4 — GitHub bridge

**Goal:** Bind, create, adopt, import, push, pull, three-way sync, conflict detection, conflict resolution. One milestone, one engine. `push` and `pull` are degenerate cases of `sync` where one side has no changes — building the reconciliation engine first means we don't write it twice.

### 6.1 `GhClient`

`src/github/client.rs` is a struct, not a trait:

```rust
pub struct GhClient {
    fixture_root: Option<PathBuf>,    // set from LIRA_GH_FIXTURES env
}

impl GhClient {
    pub fn issue_view(&self, repo: &GithubRepo, n: u64) -> Result<RemoteIssue> { ... }
    pub fn issue_create(&self, repo: &GithubRepo, payload: &IssuePayload) -> Result<RemoteIssue> { ... }
    pub fn issue_edit(&self, repo: &GithubRepo, n: u64, edits: &Edits) -> Result<()> { ... }
    pub fn issue_comments(&self, repo: &GithubRepo, n: u64) -> Result<Vec<RemoteComment>> { ... }
    pub fn issue_comment_create(&self, repo: &GithubRepo, n: u64, body: &str) -> Result<RemoteComment> { ... }
    pub fn labels_list(&self, repo: &GithubRepo) -> Result<Vec<GithubLabel>> { ... }
    pub fn labels_create(&self, repo: &GithubRepo, label: &GithubLabel) -> Result<()> { ... }
    pub fn auth_status(&self) -> Result<()> { ... }
}
```

When `fixture_root` is `Some`, every method reads a deterministic JSON fixture at `<root>/<method>/<key>.json` instead of executing `gh`. Tests set `LIRA_GH_FIXTURES=tests/fixtures/gh/<scenario>` and assert on captured calls written to `<root>/_calls.jsonl`.

`gh` is invoked with argument arrays only (never shell strings). Stderr, exit code, and stdout JSON are parsed separately to map to: `E_GH_NOT_INSTALLED`, `E_GH_AUTH`, `E_GH_NOT_FOUND`, `E_GH_PERMISSION`, `E_GH_RATE_LIMIT`, `E_GH_PR_NOT_ISSUE`, `E_GH_CONFLICT`. Minimum supported `gh` version recorded in `README.md`.

### 6.2 Hash projections

`src/github/hash.rs`:

- `local_hash(&Ticket)`: blake3 over a normalized projection that excludes `timestamps.updated`, `github.last_synced`, `github.remote_etag`, `github.remote_body_hash`, `github.local_hash`, and history entries with `action == github_sync`. Output formatted as `blake3:<hex>`.
- `remote_body_hash(&RemoteIssue)`: blake3 over normalized JSON (sorted keys, trailing whitespace stripped, body normalized to LF line endings).
- Golden tests prove: editing only volatile fields does not change `local_hash`; editing any synced field does.

### 6.3 Body sections

`src/github/body.rs`:

- Render: composes `## Description`, `## Acceptance Criteria`, `## Tasks` sections (PRD §9.4). Tasks render as `- [ ] [T1] Title #tag1 #tag2`; `- [x]` when status is `done`.
- Parse: inverse of render; round-trip golden tests.
- Content outside the three reserved sections is preserved on round-trip when policy allows; parser surfaces a structured warning if user-edited prose lives where reserved sections expect to be.

### 6.4 Field policy engine

`src/github/policy.rs`:

- `FieldPolicy::evaluate(local, remote, last_synced) -> SyncPlan` returns a list of `FieldDecision::{Push, Pull, NoOp, Conflict}` per synced field (`title`, `body`, `state`, `labels`, `assignees`, `comments`, `milestones`).
- State mapping uses `state_reason_map` for `done → closed/completed`, `cancelled → closed/not_planned`.
- Label strategies: `union` (default), `mapped`, `local_only`, `remote_only`, `replace_remote`. Ignore-list filtering applied before strategy resolution in both directions.
- Auto-create remote labels gated on `auto_create_remote: true`.
- The plan object is what `--dry-run --json` returns from `push`/`pull`/`sync`.

### 6.5 Three-way reconciliation

`src/github/sync.rs`:

```rust
pub fn decide(local_changed: bool, remote_changed: bool, overlap: &[Field]) -> SyncAction
```

is the pure function at the core. Table-driven tests cover every cell of PRD §9.3:

| local | remote | overlap | action |
|---|---|---|---|
| no | no | — | NoOp |
| yes | no | — | Push(fields) |
| no | yes | — | Pull(fields) |
| yes | yes | empty | Merge |
| yes | yes | non-empty | Conflict(fields) |

`push` and `pull` are wrappers that assert the expected action shape and execute it; they reuse the same engine.

### 6.6 Conflicts

When `decide` returns `Conflict`:

1. Set `github.sync_state = conflict`.
2. Write `~/.lira/gh-cache/conflicts/<ID>.diff` (using `similar`).
3. Write `~/.lira/gh-cache/conflicts/<ID>.yaml` with `{ fields, local_hash, remote_etag, captured_at }`.
4. Append JSONL log entry with `result: "conflict"` and `fields`.
5. Exit non-zero with `E_GH_CONFLICT`.
6. Do not mutate canonical YAML or call any `issue_edit` until `gh resolve` is invoked.

`lira gh conflicts` lists tickets in conflict; `gh conflicts show <ID>` prints structured metadata; `gh diff <ID>` prints the human diff.

`gh resolve <ID> --prefer local|remote` updates markers and emits `conflict_resolved` history. `--interactive` is rejected in `--json` mode with `E_INTERACTIVE_DISABLED`. Resolution requires `--reason` so the audit log explains the choice.

### 6.7 Commands

| Command | Notes |
|---|---|
| `lira gh link <ID> <repo>#<n>` / `gh unlink <ID>` | `link` calls `issue_view` to validate; rejects PRs with `E_GH_PR_NOT_ISSUE`. Initializes all sync markers. `unlink` keeps a `disabled` historical record. |
| `lira gh status [<ID>]` | Sync state summary for one or all tickets. |
| `lira gh create <ID>` | Builds issue from local fields with rendered body sections; binds result. |
| `lira gh adopt <repo>#<n>` | Extracts AC and tasks from body sections. If absent, `--acceptance-criterion`/`--task` flags must supply them, otherwise `E_ACCEPTANCE_CRITERIA_REQUIRED` / `E_TASK_REQUIRED`. Allocates fresh local ID. |
| `lira gh import <repo>` | Bulk adopt with `--state`, `--label`, `--acceptance-criteria-file`, `--task-template`. Skips PRs unless `--include-prs`. |
| `lira gh push <ID>` | Wrapper: asserts `decide` returns `Push` or `NoOp`, then executes. Idempotent. Supports `--dry-run --json`. |
| `lira gh pull <ID>` | Wrapper: asserts `decide` returns `Pull` or `NoOp`, then executes. Remote-driven status changes still validate against `allowed_transitions`. |
| `lira gh sync <ID>` / `gh sync --all --project P` / `--repo <repo>` | Full reconciliation. Batch mode acquires locks in lexical ticket-ID order. `--continue-on-error` defaults to true for batch; per-ticket outcomes returned in summary. |
| `lira gh diff <ID>` | Human diff between local projection and current remote. |
| `lira gh conflicts | conflicts show | resolve` | §6.6. |
| `lira gh labels list|pull|push` | Per repo / per ticket label sync. |
| `lira gh users map --add | --remove` | Assignee user mapping in project config. |

### 6.8 Rate limit + retry

`gh` rate-limit errors map to `E_GH_RATE_LIMIT` and trigger exponential backoff with jitter (NFR-10). `--all` runs respect a per-run cap and surface a structured summary of skipped tickets.

### M4 exit gates

- All 21 GitHub-relevant integration tests in PRD §23.2 pass under fixture replay.
- Adopting an issue with no parseable AC/tasks and no flag overrides fails with `E_ACCEPTANCE_CRITERIA_REQUIRED`.
- Round-trip: local → `gh create` → `gh pull` → byte-identical body sections.
- Dry-run integration tests prove `gh push --dry-run --json` and `gh pull --dry-run --json` produce zero non-read fixture calls.
- Scripted "agent runs sync after long hiatus" with 50 simulated tickets produces a structured conflict list rather than silent overwrites.
- Hash projection golden tests pass (volatile-only stable, synced-field-only changes).

---

## 7. Milestone 5 — Polish: Jira, optional index, install

**Goal:** Read-only Jira parents, an optional SQLite index built only if filesystem search hurts, agent preflight, and the install story.

### 7.1 Jira read-only

`src/jira/client.rs`:

- `lira jira fetch VAN-1234` shells out to a configured Jira CLI (PRD §8.3 `jira:` block) via argument arrays.
- Caches `{ key, title, url, status, fetched_at }` only at `projects/<P>/links/jira/<KEY>.yaml`. No full bodies.
- `JiraClient` exposes `fetch_issue` and `auth_status` only — no write methods exist in code.
- `lira jira sync-parents` refreshes all cached parents; `--dry-run --json` lists keys it would fetch.
- `lira ticket list --parent-jira VAN-1234` is a query alias, not a separate scanner.
- Failure modes: `E_JIRA_NOT_INSTALLED`, `E_JIRA_AUTH`. Local commands keep working without Jira.

### 7.2 Optional SQLite index

Only build this if M2 filesystem walk crosses NFR-3 thresholds on Brian's actual workspace.

If built:

- `rusqlite` with bundled SQLite + FTS5.
- Schema in `src/search.rs::index`: `tickets`, `tasks`, `task_tags`, `labels_local`, `labels_github`, `links`, `comments_meta`, `tickets_fts`. Each indexed row carries `source_path`, `source_mtime`, `ticket_id` for drift detection.
- `MutationContext` writes through to SQLite as a best-effort step after the canonical YAML write succeeds. Index failure marks `~/.lira/index/stale.json` and surfaces in `doctor`; YAML is never rolled back.
- `lira reindex` rebuilds from a YAML walk.
- `lira search`, `lira query`, `lira count`, `lira board` switch to index-backed paths. Pre-index versions remain available behind `--no-index` for diagnostics.

If not built: filesystem walk + `regex` matching is enough at <1k tickets.

### 7.3 Agent preflight

`lira agent preflight --json` reports tool version, `LIRA_HOME` resolution, current ownership, stale-index state, presence/version of `gh` and configured Jira CLI, and any conflicts. Pure read; no mutations, no writes, no log entries.

### 7.4 Install and docs

- `cargo install --path tools/lira` is the supported install path. README has the one-liner.
- `README.md`: quickstart, install, the `LIRA_HOME` env var, link to `SKILL.md` and PRD.
- `SKILL.md` (separate task; replaces `Lira-SKILL.pdf` archival source) documents the JSON envelope, error codes, the agent loop, and pagination.
- `lira --completions {bash|zsh|fish}` via `clap_complete` for the shells the developer actually uses. No tap, no signed releases, no cross-build matrix in CI. Build for the host architecture.

### M5 exit gates

- Round-trip `jira fetch VAN-1234` → `lira show <ID>` displays Jira title in parent block.
- `lira agent preflight --json` returns the documented shape and makes zero mutations.
- `cargo install --path tools/lira` from a fresh clone produces a working binary.
- `SKILL.md` passes a docs smoke test that extracts every fenced `lira ...` command and verifies `clap` recognizes it.

---

## 8. Cross-cutting

### 8.1 Tests

- `tests/common.rs::TestWorkspace` spins up a tempdir and sets `LIRA_HOME` for the duration of the test. Drops the tempdir on `Drop`.
- `assert_cmd` runs the binary; `predicates` asserts JSON envelope shape.
- `insta` snapshots cover: ticket YAML round-trip, JSONL entries (per action), GitHub body render, conflict file shape, `gh sync` summary JSON.
- GitHub fixtures live under `tests/fixtures/gh/<scenario>/`. Tests set `LIRA_GH_FIXTURES` and assert on `_calls.jsonl`.
- A guard at the top of `tests/common.rs` panics if `LIRA_HOME` resolves to the developer's real home — this is enforced, not optional.

### 8.2 Errors

- `LiraError` is a `thiserror` enum. `error_code(&self)` returns the stable string. `suggestions(&self)` returns a `Vec<Suggestion>` of `{ command: Option<String>, reason: String }`.
- All variants must be exhaustively handled in `error_code()` — `match` ensures the compiler catches missing entries.

### 8.3 Observability

- Every mutation appends one JSONL entry (PRD Appendix C). Pure-read commands do not log unless invoked with `--log-reads` (diagnostic flag).
- No-op outcomes are still logged with `result: "no_op"` so the audit stream is continuous.
- `lira logs tail [--project P] [--since <date>]` reads JSONL with optional filtering. Stretch goal in M5.

### 8.4 Security

- `~/.lira/` is `0700`, files inside are `0600` on macOS/Linux.
- Logger redacts any field whose key matches `/token|authorization|password|secret/i` before serialization.
- `gh-cache/` is documented as sensitive in the README.
- All shellouts to `gh`/`jira` use argument arrays. No shell strings ever.
- `doctor` warns when canonical files or cache directories are world-readable.

### 8.5 Open questions (PRD §25) interim defaults

| PRD Q | Default until resolved |
|---|---|
| §25.5 reopening semantics | Move local to `todo` if workflow allows; otherwise raise as conflict |
| §25.6 agent conflict resolution | Always escalate to human; agents may not auto-resolve in v1 |
| §25.11 completion policy strictness | Fail without explicit human `--force --reason`; agents see structured blockers |
| §25.12 AC satisfaction model | Plain strings; per-criterion status is v2 |
| Others | Open a GitHub issue at the start of the relevant milestone |

---

## 9. Acceptance Mapping

PRD §21.1 MVP Core (19 items) → M1 + M2.
PRD §21.3 Symphony Compatibility (12 items) → M3.
PRD §21.2 GitHub (19 items) → M4.
PRD §22 milestones M1–M9 → this plan's M1–M5 (PRD M1–M2 → plan M1–M2; PRD M3 → plan M3; PRD M4 → plan M5 §7.2; PRD M5 → plan M5 §7.1; PRD M6–M7 → plan M4; PRD M8–M9 → plan M5).
PRD §23 testing requirements → §8.1, exercised at each milestone exit gate.

When all five milestones' exit gates pass, lira meets PRD §29's MVP definition plus the Symphony-compatible tracker surface.
