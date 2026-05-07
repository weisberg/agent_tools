# PRD: lira — Local Jira for Agents

**Status:** Draft v0.5
**Author:** Brian Weisberg
**Last updated:** 2026-05-06
**Proposed repo:** `github.com/weisberg/agent_tools/tree/main/lira`
**Product name:** `lira` — Local Jira

---

## 0. Changes in v0.5

This version keeps the v0.4 local ticket model and updates lira to align with
the Symphony orchestration spec while preserving lira's local-first boundary:
lira is the durable local tracker/control-plane substrate, not the long-running
agent runner.

Reference: OpenAI Symphony service specification,
`https://github.com/openai/symphony/blob/main/SPEC.md`.

Major changes:

1. **lira now has an explicit Symphony compatibility posture.** It provides a local issue tracker surface that can be polled by an external orchestrator, daemon, script, or agent.
2. **The local ticket model maps to Symphony's normalized `Issue` model.** lira exposes stable projections with `id`, `identifier`, `title`, `description`, `priority`, `state`, `labels`, blockers, timestamps, URL, and optional branch/workspace hints.
3. **Claiming is specified as the local dispatch reservation primitive.** `lira claim` is the tracker-side equivalent of Symphony's `claimed` set, and must be atomic, observable, and recoverable.
4. **Candidate selection now mirrors Symphony scheduling rules.** lira defines active/terminal status sets, priority ordering, blocker gating, and claimed-ticket exclusion for `next` and candidate-list commands.
5. **Dependencies are promoted from generic links to dispatch inputs.** `blocked_by` must be available in the normalized issue projection so orchestrators can avoid starting blocked work.
6. **Workflow configuration is split by responsibility.** `workflow.yaml` remains lira's local state-machine contract; repository-owned `WORKFLOW.md` remains the optional agent-runner policy consumed by a Symphony-style daemon.
7. **Observability is aligned with orchestration.** claims, releases, candidate selection, status movement, task mutation, comments, history, and sync events remain visible through YAML and JSONL logs.
8. **Optional orchestration helpers are in scope.** lira may validate/export tracker metadata for a Symphony-style runner, but it must not require a daemon for local operation.

## 0.1 Changes in v0.4

This version keeps the v0.3 GitHub-first update and adds stricter local ticket structure for atomic agent execution.

Major changes:

1. **Every local ticket must have acceptance criteria.** `acceptance_criteria` is required, non-empty, and validated on create, update, GitHub adopt, GitHub import, and migration.
2. **Every local ticket must contain distinct atomic to-do items called `tasks`.** A ticket is the Jira-like issue container; `tasks[]` are the smallest local execution steps inside the ticket.
3. **Task metadata is intentionally minimal.** Each task has only `id`, `title`, `status`, `tags`, `created_on`, and `last_modified`.
4. **Ticket-level comments and history are first-class.** Agents can add Jira-like comments and append structured history entries to every local ticket. lira also appends automatic history for all mutations.
5. **Embedded tasks are separate from child tickets.** A task is an inline to-do item. A child ticket is another lira ticket linked through `child_tickets[]`.
6. **GitHub Issues remain first-class.** A local lira ticket can be bound to a GitHub Issue and synchronized in both directions.
7. **Jira and GitHub remain distinct external relationships.** Jira is an upward, read-only parent reference. GitHub Issues are peer sync targets by default, although a GitHub issue may also be used as a typed parent when it represents a tracking issue.
8. **GitHub sync is modeled as distributed state.** Sync uses `sync_state`, `last_synced`, `remote_etag` or `remote_body_hash`, and `local_hash` for three-way reconciliation.
9. **GitHub sync cache remains outside canonical YAML.** Volatile remote snapshots, ETags, body hashes, and conflict diffs live under `~/.lira/gh-cache/`.
10. **GitHub body sync can render acceptance criteria and tasks.** When configured, lira writes reserved Markdown sections into the GitHub Issue body for description, acceptance criteria, and tasks.
11. **Mutation logs are required.** Every local mutation and sync event is written to `~/.lira/logs/<DATE>.jsonl`.
12. **YAML remains canonical.** SQLite and GitHub caches are rebuildable implementation aids.

---

## 1. Executive Summary

`lira` is a Rust CLI that provides agent-native local ticket management with a Jira-compatible mental model, explicit GitHub Issues synchronization, and a Symphony-compatible local issue-tracker surface.

lira stores canonical ticket data as deterministic YAML files under:

```text
~/.lira/
```

The filesystem itself is part of the product model: a ticket’s status is reflected by the directory containing its YAML file. For example:

```text
~/.lira/projects/ORION/tickets/in-progress/ORION-42.yaml
```

means `ORION-42` is currently `in-progress`.

lira is local-first. Normal ticket creation, task updates, status movement, comments, labels, search, agent assignment, and orchestration reads do not require network access. External systems are explicit bridges:

| External system | Relationship | Sync behavior | v1 posture |
|---|---|---|---|
| Jira | Upward parent reference | Read-only | Context only |
| GitHub Issues | Peer issue binding | Bidirectional | Explicit sync commands |
| Symphony-style runner | Local tracker consumer | Read/claim/comment/move through CLI | Optional daemon outside lira |

Each local ticket is a Jira-like issue container. It must include:

1. **Acceptance criteria**: a required, non-empty list of externally verifiable conditions for completion.
2. **Tasks**: a required, non-empty list of distinct atomic to-do items. A task is intentionally smaller than a ticket and has only `id`, `title`, `status`, `tags`, `created_on`, and `last_modified`.
3. **Ticket-level comments and history**: Jira-like comments and append-only activity history that agents can write to and lira automatically maintains.

The product exists for local agents working on a developer’s machine. Agents need durable state between sessions, low-latency local mutations, inspectable files, safe task decomposition, safe bridges to canonical systems, and a polling-friendly control plane for automated runs. lira provides that layer.

---

## 2. Problem Statement

Agents operating on a local developer machine need a reliable task system that is:

1. **Local and fast.** Agents should not block on network latency or authentication for every update.
2. **Inspectable.** Developers should be able to open `~/.lira/` and understand the current state without a server.
3. **Diffable.** Tickets should be YAML files that can be reviewed, backed up, or committed to git.
4. **Stateful across sessions.** Work should survive agent restarts, terminal sessions, and model context resets.
5. **Safe for multiple agents.** Concurrent agents must not corrupt files or silently steal work.
6. **Concrete enough for execution.** Every ticket needs acceptance criteria and atomic tasks so agents know what “done” means and what work remains.
7. **Connected to canonical systems.** Local tickets should be traceable to Jira and synchronized with GitHub Issues.
8. **Agent-friendly.** Commands must produce stable, structured JSON and predictable error codes.
9. **Orchestrator-friendly.** A local runner should be able to poll lira, atomically claim eligible work, inspect blockers, append progress, and recover state after restart without a database server.

Existing tools do not fully solve this:

| Tool | Gap |
|---|---|
| Jira directly | Remote, permissioned, slower, high blast radius for agents |
| GitHub Issues alone | Remote-only, no local agent metadata, no local status-as-directory FSM, no local atomic task model |
| `gh issue` alone | Useful for one-shot operations, but not a durable local state store |
| Markdown TODO lists | No schema, no workflow validation, no sync semantics, no audit history |
| SQLite-only local trackers | Less inspectable than YAML and harder for agents to patch safely |

lira sits between scratch markdown, remote issue systems, and always-on agent runners. It is local enough for agent loops, structured enough for reliable automation, connected enough to synchronize with GitHub and reference Jira, and explicit enough to serve as the issue-tracker layer for a Symphony-style orchestrator.

---

## 3. Goals and Non-Goals

### 3.1 Goals

| ID | Goal |
|---|---|
| G1 | Provide a Jira-shaped local ticket model stored as YAML. |
| G2 | Store all canonical data locally under `~/.lira/`. |
| G3 | Encode ticket status as directory location. |
| G4 | Implement the tool in Rust. |
| G5 | Provide a CLI suitable for humans, agents, and scripts. |
| G6 | Support stable JSON output for every command. |
| G7 | Require every local ticket to include non-empty acceptance criteria. |
| G8 | Require every local ticket to include one or more embedded atomic `tasks[]`. |
| G9 | Keep embedded task metadata minimal: `id`, `title`, `status`, `tags`, `created_on`, and `last_modified` only. |
| G10 | Support Jira-like comments and append-only history on every local ticket. |
| G11 | Support local projects, tickets, labels, tags, priorities, assignees, links, dependencies, child tickets, history, and time tracking. |
| G12 | Support typed parent references to Jira, GitHub, or another lira ticket. |
| G13 | Treat Jira as a read-only upward parent reference in v1. |
| G14 | Treat GitHub Issues as first-class peer sync targets. |
| G15 | Allow a local ticket to link to exactly one peer GitHub Issue. |
| G16 | Support bidirectional GitHub sync for title, body, state, labels, assignees, comments, and milestones according to field policy. |
| G17 | Support GitHub issue tags/labels as first-class syncable data. |
| G18 | Prevent silent overwrites during GitHub sync through three-way reconciliation and conflict files. |
| G19 | Use `gh` CLI delegation in v1 to avoid credential storage in lira. |
| G20 | Provide a rebuildable search/index cache while keeping YAML canonical. |
| G21 | Support deterministic YAML output for clean diffs. |
| G22 | Support advisory locks and atomic writes. |
| G23 | Compose with agent tooling and future MCP/plugin integrations. |
| G24 | Expose a normalized local `Issue` projection compatible with Symphony's tracker model. |
| G25 | Support atomic local claims as dispatch reservations for orchestrators and agents. |
| G26 | Provide candidate-selection commands that honor active statuses, terminal statuses, claims, priority, creation time, and blockers. |
| G27 | Treat lira links, especially `blocked_by`, as scheduling inputs for local orchestration. |
| G28 | Keep orchestration state recoverable from local YAML, filesystem layout, and JSONL logs without requiring a persistent scheduler database. |
| G29 | Support optional validation/export helpers for repository-owned `WORKFLOW.md` and Symphony-style runner configuration without making a daemon mandatory. |
| G30 | Preserve the boundary that lira stores/tracks work while a separate runner launches Codex sessions and manages per-issue workspaces. |

### 3.2 Non-Goals for v1

| ID | Non-goal |
|---|---|
| NG1 | Bidirectional Jira sync. Jira is read-only in v1. |
| NG2 | GitHub Projects, Project v2 fields, or GitHub board sync. |
| NG3 | Real-time GitHub webhooks. Sync is command-driven in v1. |
| NG4 | A web UI. CLI first. |
| NG5 | Multi-user collaboration directly on the same `~/.lira/` directory. Sharing happens through git, GitHub sync, or future promotion flows. |
| NG6 | A required daemon or database server. |
| NG7 | Storing raw Jira or GitHub credentials in ticket YAML. |
| NG8 | Complex custom workflow engines. Custom statuses and transition maps are supported; arbitrary workflow scripting is not v1. |
| NG9 | Deleting remote GitHub issues or comments. |
| NG10 | Syncing edits or deletes to existing comments in v1. Comment sync is append-only. |
| NG11 | Treating embedded `tasks[]` as independent tickets. Tasks are local to their parent ticket. |
| NG12 | Giving embedded tasks their own comments, history, assignees, links, descriptions, priorities, estimates, dependencies, or external sync metadata. That level of detail belongs on a ticket. |
| NG13 | Creating or importing local tickets without acceptance criteria. All creation paths must provide or extract criteria. |
| NG14 | A required Symphony daemon, scheduler, or long-running process. lira must remain useful as a plain CLI. |
| NG15 | Launching Codex app-server sessions, managing live turns, tracking token usage, or killing stalled workers inside lira v1. |
| NG16 | Owning per-issue source-code workspaces. lira may expose workspace hints, but workspace creation and hooks belong to an external runner. |
| NG17 | Replacing repository-owned `WORKFLOW.md` runner policy. lira may validate or export tracker metadata, but the runner prompt and runtime policy stay with the repository. |

---

## 4. Target Users and Personas

| Persona | Primary needs |
|---|---|
| Human operator | Plan work, inspect tickets, review tasks, resolve conflicts, promote important work to canonical systems. |
| Local coding agent | Claim work, read acceptance criteria, complete atomic tasks, update progress, move statuses, create child tickets, sync linked GitHub issues. |
| Scrum-master agent | Create sprint tickets, define acceptance criteria, break work into tasks, assign work, run standups, sync GitHub at boundaries, detect blockers. |
| Analyst/research agent | Read assigned tickets, add notes, complete task checklists, produce artifacts, close work after validation. |
| Local orchestrator | Poll lira for eligible work, atomically claim tickets, start external agent runs, reconcile status, and write progress back through lira commands. |
| Automation scripts | Query, count, import, export, and reconcile tickets through stable JSON. |
| External tools | Index, embed, render, or transform ticket data without depending on a server. |

---

## 5. Core Concepts

### 5.1 Ticket

A ticket is the local Jira-like issue container and the primary unit for assignment, external linking, GitHub sync, comments, and history.

One ticket equals one canonical YAML file.

Each local ticket must include:

1. Non-empty `acceptance_criteria`.
2. Non-empty `tasks[]` containing distinct atomic to-do items.
3. Ticket-level comments and history.

Important terminology:

| Term | Meaning |
|---|---|
| Ticket | Jira-like issue container represented by one YAML file. |
| Embedded task | Atomic to-do item inside a ticket, represented as an entry in `tasks[]`. |
| Issue type `task` | Jira/GitHub-style ticket type. This is separate from embedded `tasks[]`. |
| Child ticket | Another lira ticket linked as a child. This is separate from embedded `tasks[]`. |

Ticket IDs are generated per project using a prefix and monotonic counter:

```text
ORION-42
LIRA-7
AUTH-103
```

### 5.2 Embedded Task

A task is a distinct, atomic to-do item inside a local ticket. Tasks are stored inline in the ticket YAML under `tasks[]`; they are not separate tickets and do not have their own YAML files.

Each task is addressable as:

```text
<TICKET-ID>:<TASK-ID>
ORION-42:T1
ORION-42:T2
```

A task must be small enough for an agent to complete or mark blocked without needing additional decomposition. If a task needs nested work, comments, ownership, dependencies, or external sync, it should be split into multiple sibling tasks or promoted to a child ticket.

Task fields in v1:

| Field | Required | Notes |
|---|---:|---|
| `id` | Yes | Unique within the ticket, stable, usually `T1`, `T2`, ... |
| `title` | Yes | Atomic to-do statement. |
| `status` | Yes | One of the configured task statuses. |
| `tags` | Yes | Zero or more lightweight task tags. |
| `created_on` | Yes | Creation timestamp. |
| `last_modified` | Yes | Last modified timestamp. |

Tasks must not contain comments, history, assignees, estimates, descriptions, dependencies, external links, priorities, acceptance criteria, or GitHub sync state in v1. Those concepts belong to the parent ticket.

Default task statuses:

```text
todo | in-progress | blocked | done | cancelled
```

### 5.3 Acceptance Criteria

Every local ticket must have non-empty `acceptance_criteria`.

Acceptance criteria describe what must be true for the ticket to be considered complete. They are ticket-level, testable statements, not implementation tasks.

Rules:

1. `acceptance_criteria` is required for every ticket.
2. The list must contain at least one non-empty criterion.
3. Criteria should be written as observable or testable outcomes.
4. `lira new`, `lira gh adopt`, and `lira gh import` must not create a ticket without acceptance criteria.
5. A ticket cannot move to `done` unless acceptance criteria exist and all required ticket tasks are terminal, unless `--force` is used.

### 5.4 Comments and History

Every local ticket supports Jira-style comments and history.

Comments are human- or agent-authored notes attached to the ticket. History is an append-only event stream of ticket mutations. lira writes automatic history for all state-changing commands, and agents may append explicit structured history entries when useful for auditability.

Examples of automatic history actions:

```text
created
ticket_moved
claimed
released
comment_added
task_added
task_updated
task_status_changed
acceptance_criteria_added
github_sync
conflict_detected
```

### 5.5 Project

A project is a namespace for tickets, workflow, counters, default GitHub repository, label mappings, task statuses, and sync policy.

Each project has its own directory:

```text
~/.lira/projects/<PROJECT>/
```

### 5.6 Status as Directory

Ticket status is both:

1. A field in YAML: `status: in-progress`
2. The ticket’s directory: `tickets/in-progress/ORION-42.yaml`

The two must always match. `lira doctor` and `lira validate` detect drift.

Default ticket statuses:

| Status | Meaning |
|---|---|
| `backlog` | Captured but not ready |
| `todo` | Ready to work |
| `in-progress` | Actively being worked |
| `blocked` | Cannot proceed |
| `in-review` | Awaiting review or validation |
| `done` | Complete |
| `cancelled` | No longer planned |
| `archived` | Hidden from active views |

CLI compatibility note: commands may accept underscore aliases such as `in_progress`, but YAML and directories should use kebab-case by default.

### 5.7 External Parent

A ticket may declare one typed parent.

Supported parent types:

```text
jira | github | lira | null
```

Examples:

```yaml
parent:
  type: jira
  key: VAN-1234
  url: https://vanguard.atlassian.net/browse/VAN-1234
  title: Q2 experimentation platform improvements
```

```yaml
parent:
  type: lira
  key: ORION-12
```

```yaml
parent:
  type: github
  repo: example-org/platform
  issue_number: 100
  url: https://github.com/example-org/platform/issues/100
```

Important distinction:

- A **GitHub parent** is a tracking relationship.
- A **GitHub peer binding** is a 1:1 synchronization relationship stored in the ticket’s `github` block.

These are separate so a local lira ticket can be a child of one GitHub tracking issue while also syncing to a different peer GitHub issue if needed.

### 5.8 GitHub Peer Sync

A ticket may be bound to exactly one GitHub Issue through the `github` block.

This binding is peer-style: the local ticket and the GitHub Issue represent the same work item and can synchronize according to field policies.

Sync state values:

| State | Meaning |
|---|---|
| `unbound` | No peer GitHub issue is linked |
| `synced` | Local and remote match according to policy |
| `local-ahead` | Local changed since last sync; remote did not |
| `remote-ahead` | Remote changed since last sync; local did not |
| `conflict` | Both sides changed overlapping synced fields |
| `disabled` | Historical binding exists but sync is disabled |

### 5.9 Agent Metadata

The ticket schema reserves `agent_metadata` for local-only agent data.

This data is never synced to GitHub or Jira.

Example:

```yaml
agent_metadata:
  created_by: scrum-master
  last_touched_by: athena-analyst
  context_tokens: 1024
  effort_points: 3
  scratch_refs: []
```

### 5.10 Symphony Compatibility Boundary

Symphony treats an issue tracker as the control plane for agent work: an
orchestrator polls eligible issues, claims work, launches isolated agent runs,
and writes progress back through tracker tooling. lira's v1 responsibility is
the local issue tracker portion of that model.

lira must provide:

1. A durable local ticket source of truth.
2. A normalized issue projection suitable for polling and prompt rendering.
3. Atomic claim/release operations for dispatch reservation.
4. Candidate selection that excludes terminal, claimed, and blocked work.
5. Structured comments, history, and JSONL logs for operator visibility.
6. Local recovery from YAML and filesystem state after process restart.

lira must not require:

1. A resident scheduler process.
2. A database server.
3. Network access for tracker reads or local claims.
4. A Codex app-server process.
5. Per-ticket source-code workspace management.

An external Symphony-style runner may use lira as its issue tracker by calling
`lira candidates`, `lira claim`, `lira issue show`, `lira comment`,
`lira history add`, and `lira mv` with `--json`.

### 5.11 Normalized Issue Projection

For orchestration, lira exposes each ticket as a normalized issue object. This
object is a JSON projection, not a second canonical YAML schema.

| Symphony field | lira source |
|---|---|
| `id` | Ticket ID, for example `ORION-42` |
| `identifier` | Same as ticket ID unless a future external tracker ID is explicitly configured |
| `title` | `title` |
| `description` | `description` |
| `priority` | Numeric projection of lira priority; lower numbers sort earlier |
| `state` | `status` |
| `branch_name` | Optional `orchestration.branch_name` or GitHub branch metadata |
| `url` | Local file path, GitHub issue URL, or parent URL according to command policy |
| `labels` | Normalized union of local labels and optionally mapped GitHub labels |
| `blocked_by` | `links.blocked_by[]` resolved to blocker refs where possible |
| `created_at` | `timestamps.created` |
| `updated_at` | `timestamps.updated` |

Default priority projection:

| lira priority | Symphony priority |
|---|---:|
| `highest` | 1 |
| `high` | 2 |
| `medium` | 3 |
| `low` | 4 |
| `lowest` | 5 |
| missing or unknown | null |

Blocker refs should include the blocker ticket's `id`, `identifier`, `state`,
`created_at`, and `updated_at` when the blocker is local. If the blocker is
external or missing, lira should still return the best available identifier and
leave unknown fields null.

---

## 6. Directory Structure

Recommended root structure:

```text
~/.lira/
├── config.yaml
├── projects/
│   ├── ORION/
│   │   ├── project.yaml
│   │   ├── counters.yaml
│   │   ├── workflow.yaml
│   │   ├── tickets/
│   │   │   ├── backlog/
│   │   │   ├── todo/
│   │   │   ├── in-progress/
│   │   │   ├── blocked/
│   │   │   ├── in-review/
│   │   │   ├── done/
│   │   │   ├── cancelled/
│   │   │   └── archived/
│   │   ├── links/
│   │   │   ├── jira/
│   │   │   │   └── VAN-1234.yaml
│   │   │   └── github/
│   │   │       └── example-org/
│   │   │           └── agent_tools/
│   │   │               └── 142.yaml
│   │   ├── sync/
│   │   │   └── github/
│   │   │       └── state.yaml
│   │   ├── agents/
│   │   └── views/
│   └── LIRA/
├── archive/
│   └── 2026-Q1/
├── index/
│   └── tickets.sqlite
├── gh-cache/
│   ├── etags/
│   │   └── ORION-42.json
│   ├── remote-snapshots/
│   │   └── ORION-42.json
│   └── conflicts/
│       ├── ORION-42.diff
│       └── ORION-42.yaml
├── locks/
│   ├── ORION.lock
│   └── ORION-42.lock
└── logs/
    └── 2026-05-03.jsonl
```

### 6.1 Canonical vs. Cache Data

| Location | Canonical? | Purpose |
|---|---:|---|
| `projects/*/tickets/*/*.yaml` | Yes | Ticket source of truth, including acceptance criteria, tasks, comments, and history |
| `projects/*/project.yaml` | Yes | Project metadata and defaults |
| `projects/*/workflow.yaml` | Yes | Workflow statuses and transitions |
| `projects/*/links/*/*.yaml` | Rebuildable but user-facing | Reverse links for browsing and lookup |
| `index/tickets.sqlite` | No | Search and listing cache |
| `gh-cache/*` | No | Remote snapshots, ETags, body hashes, conflicts |
| `logs/*.jsonl` | Append-only audit | Mutation and sync history |
| `locks/*` | No | Concurrency control |

`gh-cache/` is intentionally outside canonical ticket YAML because remote sync metadata changes frequently and would otherwise create noisy YAML diffs.

Embedded `tasks[]`, `acceptance_criteria[]`, `comments[]`, and `history[]` are canonical ticket data and live inside the ticket YAML file.

---

## 7. Data Model

### 7.1 Ticket YAML Schema

Example ticket:

```yaml
id: ORION-42
schema_version: 3
project: ORION
type: task
status: in-progress
priority: high

title: Implement CUPED variance reduction in experiment-analyst
summary_aliases:
  - Implement CUPED variance reduction in experiment-analyst

description: |
  Add CUPED-based variance reduction to experiment-analyst.

  The implementation should support SQL Server experiment datasets and expose
  before/after variance metrics in the generated analysis report.

assignee:
  type: agent
  id: athena-analyst
  name: Athena Analyst

reporter:
  type: user
  id: brian
  name: Brian

parent:
  type: jira
  key: VAN-1234
  url: https://vanguard.atlassian.net/browse/VAN-1234
  title: Q2 experimentation platform improvements

# Peer GitHub issue binding. This is not parentage.
github:
  repo: weisberg/agent_tools
  issue_number: 142
  url: https://github.com/weisberg/agent_tools/issues/142
  node_id: I_kwDOABCDEF
  sync_state: synced
  last_synced: 2026-05-03T14:30:00Z
  remote_etag: W/"abc123def"
  remote_body_hash: sha256:7e2c...
  local_hash: sha256:9f1a...
  field_policy: default
  remote_state: open
  remote_state_reason: null

labels:
  local:
    - rust
    - experimentation
    - cuped
  github:
    - enhancement
    - area/analytics
    - priority/high

github_labels:
  - name: enhancement
    color: a2eeef
    description: New feature or request
    default: true
  - name: area/analytics
    color: 5319e7
    description: Analytics work
    default: false
  - name: priority/high
    color: d93f0b
    description: High priority
    default: false

components:
  - experiment-analyst
  - sqlservd

# Required. Every local ticket must have at least one acceptance criterion.
acceptance_criteria:
  - CUPED reduces variance by at least 30% on a holdout dataset.
  - Generated report includes baseline variance and adjusted variance.
  - Unit tests cover missing covariates and null treatment groups.

# Required. Atomic to-do items embedded in this ticket.
# These are not child tickets and intentionally have only basic metadata.
tasks:
  - id: T1
    title: Add SQL covariate extraction.
    status: done
    tags:
      - sql
      - implementation
    created_on: 2026-05-01T10:10:00Z
    last_modified: 2026-05-02T08:45:00Z
  - id: T2
    title: Implement CUPED adjustment calculation.
    status: in-progress
    tags:
      - rust
      - statistics
    created_on: 2026-05-01T10:12:00Z
    last_modified: 2026-05-03T14:20:00Z
  - id: T3
    title: Add tests for null covariates and treatment groups.
    status: todo
    tags:
      - tests
    created_on: 2026-05-01T10:15:00Z
    last_modified: 2026-05-01T10:15:00Z

links:
  blocks:
    - ORION-50
  blocked_by: []
  relates_to:
    - ORION-30
  duplicates: []

# Child tickets are separate lira tickets. They are not embedded tasks.
child_tickets:
  - ORION-43
  - ORION-44

time_tracking:
  estimate_hours: 4
  logged_hours: 2.5
  remaining_hours: 1.5

# Ticket-level Jira-like comments. Embedded tasks do not have comments.
comments:
  - id: local-c1
    source:
      provider: local
    author:
      type: agent
      id: athena-analyst
    timestamp: 2026-05-02T09:00:00Z
    body: |
      Pulled the holdout dataset. Variance baseline = 0.0234.
    sync:
      github:
        push: true
        github_id: 12345678
        pushed_at: 2026-05-02T09:10:00Z

# Ticket-level activity history. Agents may append structured entries.
history:
  - at: 2026-05-01T10:00:00Z
    actor:
      type: agent
      id: scrum-master
    action: created
    from: null
    to: backlog
  - at: 2026-05-03T14:20:00Z
    actor:
      type: agent
      id: athena-analyst
    action: task_status_changed
    task_id: T2
    from: todo
    to: in-progress
  - at: 2026-05-03T14:30:00Z
    actor:
      type: system
      id: lira
    action: github_sync
    result: pushed
    fields:
      - state
      - labels

timestamps:
  created: 2026-05-01T10:00:00Z
  updated: 2026-05-03T14:30:00Z
  started: 2026-05-02T09:00:00Z
  completed: null
  archived: null

agent_metadata:
  created_by: scrum-master
  last_touched_by: athena-analyst
  context_tokens: 1024
  effort_points: 3
  embed_hash: null

orchestration:
  branch_name: brian/orion-42-cuped
  workspace_hint: null
  active_for_dispatch: true
  last_claimed_by: athena-analyst
  last_claimed_at: 2026-05-02T09:00:00Z
  last_released_at: null

metadata:
  source: local
  schema_version: 3
```

### 7.2 Canonical Field Names

| Concept | Canonical field | Accepted aliases |
|---|---|---|
| Ticket title | `title` | `summary`, `name` |
| Status | `status` | none |
| Type | `type` | `issue_type` |
| Description | `description` | `body` |
| Local labels | `labels.local` | `labels` flat array in legacy files |
| GitHub labels | `labels.github` | `github.labels` for detailed cache |
| Acceptance criteria | `acceptance_criteria` | `ac` |
| Embedded tasks | `tasks` | `todo_items` migration alias only |
| Child tickets | `child_tickets` | `sub_tasks` legacy alias |

`title` is canonical because it maps directly to GitHub Issue title. CLI commands should accept `--summary` for Jira compatibility.

### 7.3 Actor Schema

```yaml
type: user | agent | system | github_user
id: string
name: string | null
email: string | null
```

### 7.4 Parent Reference Schema

```yaml
type: jira | github | lira
key: string | null
repo: string | null
issue_number: integer | null
url: string | null
title: string | null
```

### 7.5 Task Schema

Every local ticket must have a non-empty `tasks` list.

```yaml
id: string
title: string
status: todo | in-progress | blocked | done | cancelled
tags: string[]
created_on: datetime
last_modified: datetime
```

Validation rules:

1. Task IDs must be unique within the parent ticket.
2. Task titles must be non-empty and should describe one atomic action.
3. Task status must be one of the configured project task statuses.
4. Task tags are simple strings and should be normalized like ticket labels.
5. A task object must not include fields other than `id`, `title`, `status`, `tags`, `created_on`, and `last_modified` in v1.
6. Updating a task must update the parent ticket’s `timestamps.updated` value and append a ticket-level history event.

### 7.6 Acceptance Criteria Schema

Every local ticket must have a non-empty `acceptance_criteria` list. Each criterion must be a non-empty, externally verifiable statement.

```yaml
acceptance_criteria:
  - string
```

Validation rules:

1. The list must contain at least one item.
2. Empty or whitespace-only criteria are invalid.
3. Creation, GitHub adoption, GitHub import, and migration commands must fail if they cannot provide or extract acceptance criteria.
4. Acceptance criteria belong to the ticket, not to embedded tasks.

### 7.7 GitHub Binding Schema

```yaml
repo: owner/repo
issue_number: integer
url: string
node_id: string | null
sync_state: synced | local-ahead | remote-ahead | conflict | unbound | disabled
last_synced: datetime | null
remote_etag: string | null
remote_body_hash: string | null
local_hash: string | null
field_policy: string
remote_state: open | closed | null
remote_state_reason: completed | not_planned | reopened | null
```

### 7.8 GitHub Label Schema

```yaml
name: string
color: string | null
description: string | null
default: boolean | null
```

### 7.9 Comment Schema

```yaml
id: string
source:
  provider: local | github
  repo: string | null
  issue_number: integer | null
  github_id: integer | null
  url: string | null
author: actor
github_author: string | null
timestamp: datetime
updated: datetime | null
body: string
sync:
  github:
    push: boolean
    github_id: integer | null
    pushed_at: datetime | null
    pulled_at: datetime | null
    sync_state: local | pushed | pulled | synced
```

### 7.10 History Event Schema

```yaml
at: datetime
actor: actor
action: string
from: string | null
to: string | null
task_id: string | null
result: string | null
fields: string[] | null
details: map | null
```

History is append-only. lira automatically creates history for all state-changing commands. Agents may append additional structured history entries using `lira history add`.

### 7.11 Orchestration Metadata Schema

`orchestration` is optional local-only metadata for external runners. It must
not be required for local ticket operation and must not be synced to Jira.
GitHub sync should ignore it unless an explicit future field policy opts in.

```yaml
branch_name: string | null
workspace_hint: string | null
active_for_dispatch: boolean | null
last_claimed_by: string | null
last_claimed_at: datetime | null
last_released_at: datetime | null
```

Rules:

1. `branch_name` is a hint for external runners and may be derived from GitHub or a runner policy.
2. `workspace_hint` is advisory only. lira does not create or own source-code workspaces in v1.
3. `active_for_dispatch: false` excludes the ticket from candidate commands even if its status is active. This is a local override for humans.
4. Claim state remains canonical in the assignment/agent fields and history. `orchestration.last_*` fields are convenience metadata only.
5. The normalized issue projection must work even when the `orchestration` block is absent.

---

## 8. Project, Workflow, and Global Configuration

### 8.1 Project Schema

```yaml
key: ORION
schema_version: 3
name: Orion Analytics Sub-Agents
description: Local tickets for the Orion team's agent workflows.
created: 2026-04-15T00:00:00Z
archived: false

default_assignee:
  type: user
  id: brian

counter:
  next_ticket_number: 48
  ticket_prefix: ORION

allowed_statuses:
  - backlog
  - todo
  - in-progress
  - blocked
  - in-review
  - done
  - cancelled
  - archived

task_statuses:
  - todo
  - in-progress
  - blocked
  - done
  - cancelled

defaults:
  status: backlog
  task_status: todo
  priority: medium
  type: task
  github_field_policy: default

completion_policy:
  require_acceptance_criteria: true
  require_all_tasks_terminal_for_done: true
  terminal_task_statuses:
    - done
    - cancelled

jira_link:
  base_url: https://vanguard.atlassian.net
  default_parent: VAN-1234

github_link:
  default_repo: weisberg/agent_tools
  auto_create: false
  closed_statuses:
    - done
    - cancelled
  open_statuses:
    - backlog
    - todo
    - in-progress
    - blocked
    - in-review
  state_reason_map:
    done: completed
    cancelled: not_planned
  field_policy: default

orchestration:
  enabled: true
  active_statuses:
    - todo
    - in-progress
  terminal_statuses:
    - done
    - cancelled
    - archived
  handoff_statuses:
    - in-review
  candidate_blocked_statuses:
    - blocked
  dispatch:
    exclude_claimed: true
    todo_requires_unblocked: true
    priority_order:
      - highest
      - high
      - medium
      - low
      - lowest
```

### 8.2 Workflow Schema

```yaml
schema_version: 3
project: ORION
default_status: backlog

statuses:
  - id: backlog
    name: Backlog
    terminal: false
  - id: todo
    name: To Do
    terminal: false
  - id: in-progress
    name: In Progress
    terminal: false
  - id: blocked
    name: Blocked
    terminal: false
  - id: in-review
    name: In Review
    terminal: false
  - id: done
    name: Done
    terminal: true
  - id: cancelled
    name: Cancelled
    terminal: true
  - id: archived
    name: Archived
    terminal: true

task_statuses:
  - id: todo
    name: To Do
    terminal: false
  - id: in-progress
    name: In Progress
    terminal: false
  - id: blocked
    name: Blocked
    terminal: false
  - id: done
    name: Done
    terminal: true
  - id: cancelled
    name: Cancelled
    terminal: true

allowed_transitions:
  backlog:
    - todo
    - cancelled
    - archived
  todo:
    - in-progress
    - blocked
    - cancelled
    - archived
  in-progress:
    - in-review
    - blocked
    - todo
    - done
  blocked:
    - todo
    - in-progress
    - cancelled
  in-review:
    - done
    - in-progress
  done:
    - archived
  cancelled:
    - archived
  archived: []

orchestration:
  active_statuses:
    - todo
    - in-progress
  terminal_statuses:
    - done
    - cancelled
    - archived
  handoff_statuses:
    - in-review
  dispatcher:
    exclude_claimed: true
    exclude_blocked: true
    stable_sort:
      - priority
      - created_at
      - identifier
```

### 8.3 Global Config Schema

```yaml
schema_version: 3

default_project: ORION
default_user: brian
editor: $EDITOR

output:
  default_format: human
  json_indent: 2

storage:
  root: ~/.lira
  permissions:
    root_dir: "0700"
    yaml_file: "0600"

index:
  enabled: true
  path: ~/.lira/index/tickets.sqlite
  rebuild_on_start: false

github:
  enabled: true
  cli: gh
  default_repo: weisberg/agent_tools
  poll_interval_seconds: null
  auth:
    method: gh_cli
  cache:
    path: ~/.lira/gh-cache
  sync:
    default_mode: bidirectional
    default_conflict_policy: manual

jira:
  enabled: true
  cli: jira
  default_base_url: https://vanguard.atlassian.net

symphony:
  enabled: false
  workflow_file: ./WORKFLOW.md
  expose_candidates: true
  runner_owned_fields:
    - workspace.root
    - hooks
    - codex
```

### 8.4 Symphony and Runner Configuration Boundary

lira has two local configuration responsibilities for Symphony-style use:

1. **Tracker state-machine config** lives in `~/.lira/projects/<PROJECT>/workflow.yaml`.
2. **Runner policy** lives in repository-owned `WORKFLOW.md` and is consumed by an external runner.

lira may validate that a `WORKFLOW.md` exists and that its front matter names a
supported tracker kind, but lira must not require `WORKFLOW.md` for normal local
ticket operations.

Recommended `WORKFLOW.md` tracker front matter when lira is the tracker:

```yaml
---
tracker:
  kind: lira
  project: ORION
  active_states:
    - todo
    - in-progress
  terminal_states:
    - done
    - cancelled
    - archived
workspace:
  root: ~/.codex/symphony-workspaces
agent:
  max_concurrent_agents: 4
codex:
  command: codex app-server
---
```

The `tracker.kind: lira` extension is local to this project unless adopted by a
future upstream Symphony spec. Unknown `WORKFLOW.md` keys must be ignored by lira
unless the relevant helper command explicitly validates them.

Dynamic reload, polling interval, workspace hooks, Codex process management,
retry timers, stall detection, token accounting, and per-issue workspace cleanup
belong to the external runner. lira's obligation is to make each tracker read
and mutation deterministic, structured, lock-safe, and recoverable.

### 8.5 GitHub Field Policies

Field policies control how each GitHub field syncs.

```yaml
github_field_policies:
  default:
    title:
      sync: bidirectional
      conflict: manual

    body:
      sync: bidirectional
      conflict: prefer_local
      sections:
        description: true
        acceptance_criteria: true
        tasks: true

    state:
      sync: bidirectional
      mapping:
        local_to_remote:
          done:
            state: closed
            state_reason: completed
          cancelled:
            state: closed
            state_reason: not_planned
          "*":
            state: open
        remote_to_local:
          closed_completed: done
          closed_not_planned: cancelled
          open: keep

    labels:
      sync: bidirectional
      strategy: union
      auto_create_remote: true
      ignore:
        - wontfix
      local_to_github:
        priority.high: priority/high
        priority.medium: priority/medium
        type.bug: bug
        type.feature: enhancement
      github_to_local:
        priority/high: priority.high
        priority/medium: priority.medium
        bug: type.bug
        enhancement: type.feature

    assignees:
      sync: bidirectional
      cardinality: single
      user_map:
        athena-analyst: weisberg-bot
        experiment-analyst: weisberg-bot
        scrum-master: weisberg

    comments:
      sync: bidirectional
      mode: append_only
      author_map_required: true

    milestones:
      sync: pull_only
```

Supported field sync modes:

| Mode | Meaning |
|---|---|
| `none` | Never sync the field |
| `pull_only` | GitHub to local only |
| `push_only` | Local to GitHub only |
| `bidirectional` | Sync both directions with conflict handling |
| `append_only` | Add missing items but never edit or delete existing ones |

Supported label strategies:

| Strategy | Meaning |
|---|---|
| `union` | Merge local and remote labels |
| `local_only` | Push local labels, ignore remote labels |
| `remote_only` | Pull remote labels, do not push local labels |
| `mapped` | Sync only labels with configured mappings |
| `replace_remote` | Replace remote labels; requires explicit configuration or `--replace-labels` |

---

## 9. GitHub Sync Model

### 9.1 Relationship Types

A lira ticket can interact with GitHub in two ways:

1. **Typed parent reference** in `parent`.
2. **Peer sync binding** in `github`.

The peer sync binding is the primary GitHub integration.

### 9.2 Sync Markers

Each synced ticket stores three markers:

| Marker | Purpose |
|---|---|
| `last_synced` | Timestamp of the last successful sync |
| `remote_etag` or `remote_body_hash` | Fingerprint of last-seen GitHub state |
| `local_hash` | Hash of the last-synced local YAML projection |

The local hash should exclude volatile fields such as `timestamps.updated`, `github.last_synced`, `github.remote_etag`, `github.remote_body_hash`, and history entries caused only by sync bookkeeping.

If the selected `gh` transport does not expose an ETag for a call, lira must compute a deterministic `remote_body_hash` from the normalized remote issue payload.

### 9.3 Three-Way Reconciliation

`lira gh sync <ID>` compares current local state and current remote state against the last synced markers.

| Local changed? | Remote changed? | Result |
|---:|---:|---|
| No | No | No-op; set or keep `sync_state: synced` |
| Yes | No | Push changed fields according to policy |
| No | Yes | Pull changed fields according to policy |
| Yes | Yes | Check field overlap; if overlapping, conflict; otherwise merge if safe |

When a conflict is detected:

1. Set `github.sync_state: conflict`.
2. Write a conflict diff under `~/.lira/gh-cache/conflicts/<ID>.diff`.
3. Write structured conflict metadata under `~/.lira/gh-cache/conflicts/<ID>.yaml`.
4. Log the conflict to `~/.lira/logs/<DATE>.jsonl`.
5. Exit non-zero.
6. Do not mutate local canonical fields or remote GitHub fields unless the user explicitly resolves the conflict.

### 9.4 Acceptance Criteria and Tasks in GitHub Body

When GitHub body sync is enabled, lira should render reserved Markdown sections in the GitHub Issue body so required local ticket data remains visible remotely:

```markdown
## Description
...

## Acceptance Criteria
- CUPED reduces variance by at least 30% on a holdout dataset.
- Generated report includes baseline variance and adjusted variance.

## Tasks
- [x] [T1] Add SQL covariate extraction. #sql #implementation
- [ ] [T2] Implement CUPED adjustment calculation. #rust #statistics
```

Pull, adopt, and import commands should parse these sections when present. If a GitHub Issue does not contain parseable acceptance criteria or tasks, lira must require them through command flags or files. It must not create a local ticket with empty acceptance criteria or no tasks.

Embedded tasks remain local lira tasks. They do not become separate GitHub Issues in v1.

### 9.5 ETag Cache Schema

```json
{
  "ticket_id": "ORION-42",
  "repo": "weisberg/agent_tools",
  "issue_number": 142,
  "etag": "W/\"abc123def\"",
  "remote_updated_at": "2026-05-03T14:25:00Z",
  "remote_body_hash": "sha256:7e2c...",
  "local_synced_hash": "sha256:9f1a...",
  "fetched_at": "2026-05-03T14:30:00Z"
}
```

---

## 10. GitHub Labels and Tags

GitHub calls these **labels**. lira should use `labels` internally and support `tag` as a CLI alias.

### 10.1 Storage

lira stores local and GitHub labels separately:

```yaml
labels:
  local:
    - rust
    - cuped
    - agent-task
  github:
    - enhancement
    - area/analytics
    - priority/high
```

Detailed GitHub label metadata is cached in `github_labels`:

```yaml
github_labels:
  - name: priority/high
    color: d93f0b
    description: High-priority issue
    default: false
```

Task tags are stored separately inside each task and do not automatically become GitHub labels unless a field policy maps them.

### 10.2 Effective Labels

For display and filtering, lira computes effective labels from:

1. `labels.local`
2. `labels.github`
3. Field policy mappings
4. Ignored labels
5. Optionally, task tags when `--include-task-tags` is requested

The effective set is not required to be written to canonical YAML because it is derivable.

### 10.3 Label Sync Rules

| Rule | Requirement |
|---|---|
| Separation | Local labels, GitHub labels, and task tags must remain separately addressable. |
| Union default | Default GitHub label sync strategy is `union`. |
| No silent replace | lira must not replace all remote GitHub labels unless explicitly configured. |
| Auto-create | lira may create missing GitHub labels only if `auto_create_remote: true`. |
| Ignore list | Labels in the policy ignore list do not sync in either direction. |
| Filtering | Users and agents can filter by local label, GitHub label, effective label, or task tag. |
| CLI alias | `lira ticket tag` is an alias for `lira ticket label`; `lira task tag` manages task tags. |

---

## 11. GitHub Comments Sync

Comment sync is append-only by default in v1.

### 11.1 Pull Behavior

When pulling comments from GitHub:

1. New remote comments are copied into local `comments[]`.
2. Existing local mirrors are identified by `github_id`.
3. Comment edits on GitHub are detected but not applied in v1 unless explicitly enabled in a future version.
4. Remote deletes are not propagated in v1.

### 11.2 Push Behavior

When pushing comments to GitHub:

1. Only local comments marked for GitHub push are pushed.
2. lira records the returned GitHub comment ID.
3. Re-running the push must be idempotent.
4. Existing GitHub comments are not edited or deleted in v1.

Example local comment marked for push:

```yaml
comments:
  - id: local-c7
    source:
      provider: local
    author:
      type: agent
      id: coding-agent-1
    timestamp: 2026-05-03T13:30:00Z
    body: Implemented locally. Tests are passing.
    sync:
      github:
        push: true
        github_id: null
        pushed_at: null
        sync_state: local
```

---

## 12. CLI Surface

All commands must support:

```bash
--json
--yaml
--format human|json|yaml
--project <PROJECT>
--no-color
--quiet
--verbose
```

All commands must:

1. Exit non-zero on error.
2. Emit structured errors when `--json` is used.
3. Avoid interactive prompts when `--json` is used.
4. Support stdin where appropriate.
5. Be idempotent where practical.

### 12.1 Initialization and Maintenance

```bash
lira init
lira doctor
lira validate
lira reindex
lira config get [KEY]
lira config set <KEY> <VALUE>
```

### 12.2 Project Commands

```bash
lira project create ORION --name "Orion Analytics Sub-Agents" --prefix ORION
lira project list
lira project show ORION
lira project archive ORION
```

### 12.3 Ticket Lifecycle Commands

```bash
lira new "Add CUPED to experiment-analyst" \
  --project ORION \
  --type task \
  --priority high \
  --acceptance-criterion "CUPED reduces variance by at least 30% on a holdout dataset." \
  --acceptance-criterion "Generated report includes baseline and adjusted variance." \
  --task "Add SQL covariate extraction." \
  --task "Implement CUPED adjustment calculation."

lira show ORION-42
lira ls --project ORION
lira mv ORION-42 in-progress
lira update ORION-42 --title "Implement CUPED variance reduction"
lira archive ORION-42
```

Completion guard:

```bash
lira mv ORION-42 done
```

must fail unless:

1. `acceptance_criteria[]` is non-empty.
2. All required embedded tasks are in terminal statuses, normally `done` or `cancelled`.

`--force` may override this for humans, but JSON-mode agent usage should receive a structured warning or failure according to policy.

### 12.4 Assignment and Agent Commands

```bash
lira claim ORION-42 --agent athena-analyst
lira release ORION-42
lira active --agent athena-analyst
lira next --project ORION --agent athena-analyst
lira summary --project ORION
```

### 12.5 Embedded Task Commands

Task commands mutate the embedded `tasks[]` list on a ticket. They do not create child tickets.

```bash
lira task add ORION-42 "Add SQL covariate extraction." --tag sql --tag implementation
lira task list ORION-42
lira task show ORION-42 T1
lira task status ORION-42 T1 in-progress
lira task tag add ORION-42 T1 sql
lira task tag remove ORION-42 T1 sql
lira task done ORION-42 T1
lira task cancel ORION-42 T1
```

No hard delete is available for tasks in normal workflows. A task can be cancelled, and history records the cancellation.

### 12.6 Comments and History

Comments are free-form Jira-like discussion or progress notes on the ticket. History is structured, append-only activity. lira automatically writes history for mutations, and agents may append custom history events when they need to record a structured activity.

```bash
lira comment ORION-42 "Implemented the local change."
lira comment ORION-42 --stdin
lira comment sync ORION-42 local-c7 --github

lira history ORION-42
lira history add ORION-42 --action analysis_note --message "Validated CUPED assumptions." --actor athena-analyst
```

### 12.7 Links and Dependencies

```bash
lira link ORION-42 --jira VAN-1234
lira link ORION-42 --parent-lira ORION-12
lira link ORION-42 --parent-github example-org/platform#100
lira link ORION-42 --blocks ORION-50
lira link ORION-42 --relates-to ORION-30
lira child add ORION-42 ORION-43
lira child remove ORION-42 ORION-43
```

### 12.8 Jira Bridge

Jira is read-only in v1.

```bash
lira jira fetch VAN-1234
lira jira sync-parents
lira ticket list --parent-jira VAN-1234
```

### 12.9 GitHub Bridge

The GitHub command namespace should be `gh` for brevity, with `github` as an alias.

#### Binding

```bash
lira gh link ORION-42 --repo weisberg/agent_tools --issue 142
lira gh link ORION-42 weisberg/agent_tools#142
lira gh unlink ORION-42
lira gh status [ORION-42]
```

#### Creating and adopting

```bash
lira gh create ORION-42 --repo weisberg/agent_tools

lira gh adopt weisberg/agent_tools#142 \
  --project ORION \
  --acceptance-criterion "Issue behavior is reproduced and fixed." \
  --task "Review GitHub issue body and reproduce locally."

lira gh import weisberg/agent_tools \
  --project ORION \
  --state open \
  --label bug \
  --acceptance-criteria-file ./default-ac.yaml \
  --task-template "Triage imported GitHub issue."
```

Terminology:

| Command | Meaning |
|---|---|
| `gh create` | Create a GitHub Issue from an existing local lira ticket and bind it. |
| `gh adopt` | Create a local lira ticket from one existing GitHub Issue and bind it. |
| `gh import` | Adopt many GitHub Issues into local lira tickets. |

`gh adopt` and `gh import` must extract or require acceptance criteria and at least one task. They must not create incomplete local tickets.

#### Sync

```bash
lira gh pull ORION-42
lira gh push ORION-42
lira gh sync ORION-42
lira gh sync --all --project ORION
lira gh sync --repo weisberg/agent_tools
lira gh diff ORION-42
```

#### Conflicts

```bash
lira gh conflicts
lira gh conflicts show ORION-42
lira gh resolve ORION-42 --prefer local
lira gh resolve ORION-42 --prefer remote
lira gh resolve ORION-42 --interactive
```

#### Labels and users

```bash
lira gh labels list weisberg/agent_tools
lira gh labels pull --repo weisberg/agent_tools
lira gh labels push ORION-42
lira gh users map --add athena-analyst weisberg-bot
lira gh users map --remove athena-analyst
```

### 12.10 Search, Query, and Board Commands

```bash
lira search "token refresh"
lira query --status in-progress --label rust
lira query --task-status blocked
lira query --task-tag sql
lira count --project ORION --group-by status
lira board --project ORION
```

### 12.11 Symphony and Local Orchestration Commands

These commands expose lira as a local issue tracker for a Symphony-style runner.
They are pure local operations unless explicitly combined with `gh` or `jira`
commands.

```bash
lira candidates --project ORION --json
lira candidates --project ORION --state todo --limit 10 --json
lira issue show ORION-42 --json
lira issue current --ids ORION-42 ORION-50 --json
lira workflow symphony export --project ORION --json
lira workflow symphony validate ./WORKFLOW.md --project ORION --json
```

`lira candidates` returns normalized issue objects sorted by:

1. priority ascending after numeric projection,
2. `timestamps.created` oldest first,
3. ticket identifier lexicographically.

A ticket is candidate-eligible only if all are true:

1. It has `id`, `title`, `status`, and `timestamps.created`.
2. Its status is in the project's `orchestration.active_statuses`.
3. Its status is not in `orchestration.terminal_statuses`.
4. It is not claimed when `exclude_claimed` is true.
5. It does not have `orchestration.active_for_dispatch: false`.
6. If its status is `todo`, all `links.blocked_by[]` tickets are terminal or absent according to project policy.

`lira issue show` and `lira issue current` return normalized issue projections
for prompt rendering and reconciliation. They do not mutate tickets.

`lira workflow symphony export` emits a suggested `tracker.kind: lira`
configuration block for a repository-owned `WORKFLOW.md`. It must not overwrite
files by default.

`lira workflow symphony validate` checks only the tracker portion lira
understands. It must ignore runner-owned keys such as `workspace`, `hooks`,
`agent`, and `codex`.

---

## 13. Functional Requirements

### 13.1 Core Requirements

| ID | Requirement | Priority |
|---|---|---|
| FR-1 | `lira init` creates `~/.lira/` and the default directory structure. | Must |
| FR-2 | lira stores canonical data as YAML files. | Must |
| FR-3 | lira does not require network access for local-only commands. | Must |
| FR-4 | `lira project create` creates project config, counters, workflow, task statuses, and status directories. | Must |
| FR-5 | `lira new` creates a YAML file in the default status directory and increments the project counter. | Must |
| FR-6 | Ticket IDs are monotonically increasing within a project. | Must |
| FR-7 | Every local ticket must include a non-empty `acceptance_criteria` list. | Must |
| FR-8 | Every local ticket must include a non-empty `tasks` list. | Must |
| FR-9 | Each embedded task must be atomic and distinct within the parent ticket. | Must |
| FR-10 | Embedded task metadata is limited to `id`, `title`, `status`, `tags`, `created_on`, and `last_modified`. | Must |
| FR-11 | Task IDs must be unique within their parent ticket. | Must |
| FR-12 | Task changes must update the parent ticket timestamp and append ticket-level history. | Must |
| FR-13 | `lira mv` validates transitions and keeps status field and directory consistent. | Must |
| FR-14 | `lira mv <ID> done` must enforce completion policy for acceptance criteria and task statuses unless forced. | Must |
| FR-15 | `lira show` resolves child tickets, dependencies, external links, task summary, comments, and history. | Must |
| FR-16 | lira supports ticket comments, labels, priorities, assignees, dependencies, child tickets, and history. | Must |
| FR-17 | Agents can add Jira-like comments to every local ticket. | Must |
| FR-18 | Agents can append structured history events to every local ticket. | Must |
| FR-19 | lira automatically appends history for create, update, move, claim, release, comment, task mutation, link, and sync operations. | Must |
| FR-20 | lira supports `tag` as a CLI alias for `label`, and supports task tags separately. | Should |
| FR-21 | lira supports typed parents: Jira, GitHub, or lira. | Must |
| FR-22 | `lira link --jira` validates Jira key shape and stores a typed parent reference. | Must |
| FR-23 | All commands support stable, versioned JSON output. | Must |
| FR-24 | JSON-mode errors include stable `error_code`. | Must |
| FR-25 | `lira validate` detects drift between status field and directory location. | Must |
| FR-26 | `lira validate` detects missing acceptance criteria, missing tasks, invalid task fields, and invalid task statuses. | Must |
| FR-27 | Concurrent writes are serialized through advisory locks. | Must |
| FR-28 | Writes are atomic: write temp file, fsync where practical, rename. | Must |
| FR-29 | Mutations write to `~/.lira/logs/<DATE>.jsonl`. | Must |
| FR-30 | CLI supports stdin piping for descriptions and comments. | Should |
| FR-31 | SQLite index is fully rebuildable from YAML. | Must |
| FR-32 | lira must never hard-delete tickets or tasks in normal operation. | Must |
| FR-33 | lira supports deterministic YAML output. | Must |
| FR-34 | Ticket creation, GitHub adoption, GitHub import, and migration commands fail when required acceptance criteria or tasks are missing. | Must |

### 13.2 GitHub Binding and Sync Requirements

| ID | Requirement | Priority |
|---|---|---|
| GH-1 | lira allows each ticket to bind to one peer GitHub Issue. | Must |
| GH-2 | lira stores GitHub repo, issue number, URL, node ID, remote state, sync state, and sync markers. | Must |
| GH-3 | `lira gh link` validates that the GitHub Issue exists when network/auth is available. | Must |
| GH-4 | `lira gh link` fetches current issue state and initializes sync markers. | Must |
| GH-5 | `lira gh create` creates a GitHub Issue from local fields and binds the result. | Must |
| GH-6 | `lira gh create` includes acceptance criteria and tasks in the GitHub body when body section sync is enabled. | Should |
| GH-7 | `lira gh adopt` creates a local ticket from one existing GitHub Issue. | Must |
| GH-8 | `lira gh import` adopts multiple GitHub Issues into local tickets. | Should |
| GH-9 | `lira gh adopt` and `lira gh import` must extract or require acceptance criteria and tasks before creating local tickets. | Must |
| GH-10 | `lira gh pull` updates local fields from GitHub according to field policy. | Must |
| GH-11 | `lira gh push` updates GitHub fields from local data according to field policy. | Must |
| GH-12 | `lira gh sync` performs three-way reconciliation. | Must |
| GH-13 | `lira gh sync` decides among no-op, push, pull, merge, or conflict. | Must |
| GH-14 | When both sides changed overlapping fields, sync sets `sync_state: conflict` and does not mutate local or remote fields. | Must |
| GH-15 | Sync conflicts write a diff file under `~/.lira/gh-cache/conflicts/`. | Must |
| GH-16 | lira supports conflict resolution with `--prefer local`, `--prefer remote`, and interactive resolution. | Should |
| GH-17 | GitHub labels are stored separately from local labels and task tags. | Must |
| GH-18 | GitHub label sync follows configured strategy: union, mapped, local-only, remote-only, or explicit replace. | Must |
| GH-19 | Missing GitHub labels may be auto-created only when `auto_create_remote: true`. | Must |
| GH-20 | Labels in the ignore list do not sync in either direction. | Must |
| GH-21 | State sync respects local-to-remote and remote-to-local status mappings. | Must |
| GH-22 | Closing a GitHub Issue maps to `done` or `cancelled` based on `state_reason` and policy. | Must |
| GH-23 | Reopening a GitHub Issue maps to the configured open status or keeps local status according to policy. | Must |
| GH-24 | All remote-triggered local status changes still validate against allowed transitions unless forced. | Must |
| GH-25 | Comment sync is append-only by default in v1. | Must |
| GH-26 | New local comments without `github_id` can be pushed if marked for GitHub sync. | Should |
| GH-27 | New remote comments without local mirrors are pulled. | Must |
| GH-28 | Edits to existing comments are not synced in v1. | Must |
| GH-29 | Deletes of GitHub comments are not synced in v1. | Must |
| GH-30 | Assignee sync uses `user_map`. | Must |
| GH-31 | Unmapped local assignees fail loudly when push would require a GitHub login. | Must |
| GH-32 | Unmapped remote assignees are pulled as raw GitHub logins with a `gh:` prefix. | Should |
| GH-33 | `lira gh push` is idempotent when there are no intermediate changes. | Must |
| GH-34 | All sync events are logged to JSONL with enough detail to replay or audit. | Must |
| GH-35 | `lira gh adopt` and `lira gh import` allocate fresh local IDs and never reuse GitHub issue numbers as local IDs. | Must |
| GH-36 | GitHub commands support `--json`. | Must |
| GH-37 | GitHub commands fail clearly when `gh` is unavailable, unauthenticated, or lacks permissions. | Must |
| GH-38 | lira can list tickets by GitHub repo, issue number, sync state, and GitHub label. | Must |
| GH-39 | lira distinguishes GitHub Issues from pull requests during import and sync. | Must |
| GH-40 | Embedded lira tasks must not be created as separate GitHub Issues in v1. | Must |

### 13.3 Jira Requirements

| ID | Requirement | Priority |
|---|---|---|
| JIRA-1 | lira supports Jira parent references. | Must |
| JIRA-2 | Jira references are read-only in v1. | Must |
| JIRA-3 | lira does not require Jira credentials for local-only operation. | Must |
| JIRA-4 | Optional Jira fetch delegates to an external Jira CLI or configured bridge. | Should |
| JIRA-5 | lira can list all local tickets under a Jira parent. | Must |
| JIRA-6 | lira can cache Jira parent title and URL as non-authoritative metadata. | Should |

### 13.4 Symphony Compatibility Requirements

| ID | Requirement | Priority |
|---|---|---|
| SYM-1 | lira exposes a normalized issue projection with `id`, `identifier`, `title`, `description`, `priority`, `state`, `branch_name`, `url`, `labels`, `blocked_by`, `created_at`, and `updated_at`. | Must |
| SYM-2 | lira projects define active and terminal statuses for orchestration separately from general workflow transitions. | Must |
| SYM-3 | Candidate commands exclude terminal tickets, claimed tickets, dispatch-disabled tickets, and tickets blocked by non-terminal local blockers. | Must |
| SYM-4 | Candidate commands sort by priority, creation time, and identifier in a stable order. | Must |
| SYM-5 | `lira claim` is atomic and fails with `E_CLAIM_HELD` when another agent or runner owns the ticket, unless an explicit human force policy is used. | Must |
| SYM-6 | Claim, release, status movement, comments, history, and task changes are recoverable from ticket YAML plus JSONL logs after an orchestrator restart. | Must |
| SYM-7 | lira does not require a daemon, network access, or a scheduler database to list candidates or claim local work. | Must |
| SYM-8 | lira can return current issue projections for a list of IDs so an external runner can reconcile active runs. | Must |
| SYM-9 | lira can export or validate the tracker portion of a repository-owned `WORKFLOW.md` using a `tracker.kind: lira` extension. | Should |
| SYM-10 | lira does not launch Codex app-server, manage agent subprocesses, detect stalls, track token usage, or clean per-issue source-code workspaces in v1. | Must |
| SYM-11 | lira JSONL logs include enough information for an operator to audit why work was claimed, released, moved, blocked, or completed. | Must |
| SYM-12 | lira comments and history are suitable for an external runner to write proof-of-work summaries, CI status, PR links, and human-review handoffs. | Should |

---

## 14. Non-Functional Requirements

| ID | Requirement |
|---|---|
| NFR-1 | `lira ls --json` over 1,000 tickets should complete in under 100 ms with the index. |
| NFR-2 | `lira ls --json` over 10,000 tickets should complete in under 200 ms with the index. |
| NFR-3 | Creating, moving, task-updating, commenting on, or updating a single ticket should normally complete in under 100 ms excluding sync. |
| NFR-4 | Single stripped binary should target under 10 MB where practical. |
| NFR-5 | Cross-platform support: macOS, Linux, and Windows. |
| NFR-6 | Same logical input should produce byte-stable YAML output. |
| NFR-7 | No hard deletes in normal workflows. |
| NFR-8 | No telemetry by default. |
| NFR-9 | All mutations and sync events are observable through JSONL logs. |
| NFR-10 | GitHub sync must respect rate limits and back off on rate-limit errors. |
| NFR-11 | No sync command may force-overwrite a remote field changed since `last_synced` without explicit `--force`. |
| NFR-12 | No interactive prompts in `--json` mode. |
| NFR-13 | Agent-facing JSON schemas are versioned. |
| NFR-14 | Commands that return collections support bounded output and cursors. |
| NFR-15 | lira must continue to work offline for local-only commands even when GitHub or Jira tools are unavailable. |
| NFR-16 | Task operations must remain lightweight and should not require reindexing the whole workspace. |
| NFR-17 | Orchestrator-facing read commands must be safe to call repeatedly from a polling loop. |
| NFR-18 | Candidate and issue projection output must be stable enough for an external runner to diff, log, and render prompts without additional parsing heuristics. |
| NFR-19 | lira must preserve local-first behavior even when used by an always-on runner: the runner is optional, replaceable, and outside canonical storage. |

---

## 15. Rust Architecture

Recommended workspace:

```text
lira/
├── Cargo.toml
├── crates/
│   ├── lira-core/        # ticket model, tasks, workflows, validation, sync projections
│   ├── lira-store/       # YAML I/O, path logic, locking, atomic writes
│   ├── lira-index/       # SQLite + FTS5 rebuildable index
│   ├── lira-jira/        # read-only Jira parent fetch
│   ├── lira-github/      # GitHub binding and sync through gh CLI
│   ├── lira-format/      # human, JSON, YAML renderers
│   ├── lira-agent/       # agent-specific helpers and JSON schemas
│   ├── lira-symphony/    # normalized issue projection and local orchestration helpers
│   └── lira-cli/         # clap binary
└── xtask/
```

### 15.1 Core Module Responsibilities

| Module | Responsibilities |
|---|---|
| `lira-core` | Data models, embedded tasks, acceptance criteria validation, workflows, transition validation, field policies, hash projections |
| `lira-store` | File paths, YAML read/write, deterministic serialization, advisory locks, atomic moves |
| `lira-index` | Rebuildable SQLite/FTS cache, search, query, counts |
| `lira-jira` | Jira parent fetch/cache, read-only bridge |
| `lira-github` | GitHub issue view/create/edit/comment/labels, sync, conflicts, user mapping |
| `lira-format` | Text tables, JSON schemas, YAML output |
| `lira-agent` | Claim/release, next-ticket selection, agent summaries |
| `lira-symphony` | Normalized issue projection, candidate eligibility, blocker resolution, `WORKFLOW.md` tracker validation/export |
| `lira-cli` | CLI parsing, command dispatch, exit codes |

### 15.2 lira-github Responsibilities

`lira-github` must:

1. Shell out to `gh` for GitHub operations in v1.
2. Validate `gh auth status` before commands requiring auth.
3. Wrap issue view/create/edit/comment operations.
4. Wrap label list/create operations.
5. Normalize remote issue payloads.
6. Render and parse reserved GitHub body sections for description, acceptance criteria, and tasks when policy enables them.
7. Compute `remote_body_hash` and `local_hash`.
8. Store or update `remote_etag` when available.
9. Apply field policies.
10. Perform three-way reconciliation.
11. Generate conflict metadata and diff files.
12. Surface structured errors.

### 15.3 Suggested Crates

| Purpose | Crate |
|---|---|
| CLI parsing | `clap` |
| Serialization | `serde`, `serde_json`, `serde_yaml` or YAML emitter with deterministic support |
| Time | `time` or `chrono` |
| Paths | `directories` |
| Errors | `thiserror`, `anyhow` |
| Locking | `fs2` or platform-specific advisory locks |
| SQLite index | `rusqlite` with bundled SQLite where appropriate |
| Full-text search | SQLite FTS5 |
| Hashing | `blake3` or `sha2` |
| Diffs | `similar` |
| Temp files | `tempfile` |
| Table output | `comfy-table` |

### 15.4 Why `gh` CLI for v1

lira should delegate GitHub transport and auth to `gh` in v1 because:

1. It avoids storing credentials in lira.
2. It reuses the user’s existing GitHub authentication.
3. It supports common GitHub Enterprise and SSO setups through the user’s configured CLI.
4. It keeps lira focused on local state, schema, and sync semantics.

A native HTTP client can be added later behind a feature flag if performance or deployment requirements demand it.

---

## 16. Locking, Atomicity, and Consistency

### 16.1 Locking Strategy

Write operations must acquire locks.

Recommended lock granularity:

| Operation | Lock |
|---|---|
| Create ticket | Project lock |
| Move ticket | Ticket lock + project lock if index/reverse links change |
| Update ticket | Ticket lock |
| Add/update task | Ticket lock |
| Add comment | Ticket lock |
| Add history | Ticket lock |
| Reindex | Global index lock |
| GitHub sync one ticket | Ticket lock + GitHub cache lock for that ticket |
| Batch sync | Project lock or ordered ticket locks to avoid deadlocks |

Lock files:

```text
~/.lira/locks/ORION.lock
~/.lira/locks/ORION-42.lock
```

### 16.2 Atomic Write Procedure

For every file write:

1. Acquire relevant lock.
2. Read latest state.
3. Validate schema and transition.
4. Validate acceptance criteria and task schema.
5. Serialize deterministic YAML or JSON.
6. Write to temporary file in the same filesystem.
7. Flush and fsync where practical.
8. Atomically rename into place.
9. Update indexes and reverse links.
10. Append JSONL log event.
11. Release lock.

### 16.3 Status Move Procedure

When moving a ticket:

1. Validate target status exists.
2. Validate transition unless `--force` is used.
3. If target is `done`, enforce completion policy for acceptance criteria and tasks unless forced.
4. Update `status` field.
5. Update timestamps.
6. Add history event.
7. Move YAML file to `tickets/<status>/<ID>.yaml` atomically.
8. Mark GitHub `sync_state: local-ahead` if status maps to GitHub state and policy syncs state.
9. Update index and log event.

---

## 17. Indexing and Search

YAML remains canonical. SQLite is only a rebuildable cache.

Index path:

```text
~/.lira/index/tickets.sqlite
```

Indexed fields:

1. Ticket ID
2. Project
3. Type
4. Status
5. Priority
6. Title
7. Description
8. Acceptance criteria
9. Embedded task titles, statuses, and tags
10. Assignee
11. Reporter
12. Parent type and key
13. GitHub repo and issue number
14. GitHub sync state
15. Local labels
16. GitHub labels
17. Effective labels
18. Components
19. Created/updated timestamps
20. Full-text title, description, acceptance criteria, task titles, comments

Commands:

```bash
lira search "token refresh"
lira query --status in-progress --label rust
lira query --task-status blocked
lira query --task-tag sql
lira count --project ORION --group-by status
lira reindex
```

---

## 18. Output and Error Handling

### 18.1 Successful JSON Output

Example:

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "id": "ORION-48",
    "status": "in-progress",
    "tasks": {
      "total": 3,
      "done": 1,
      "blocked": 0
    },
    "github_sync": {
      "repo": "weisberg/agent_tools",
      "issue_number": 201,
      "sync_state": "local-ahead"
    }
  }
}
```

### 18.2 Error JSON Output

Example:

```json
{
  "schema_version": 3,
  "ok": false,
  "error": {
    "error_code": "E_TASK_REQUIRED",
    "message": "Ticket ORION-42 must contain at least one task.",
    "details": {
      "ticket_id": "ORION-42"
    },
    "suggestions": [
      "Run `lira task add ORION-42 \"Describe the atomic task\"`.",
      "Or recreate the ticket with `--task` flags."
    ]
  }
}
```

### 18.3 Error Codes

| Code | Meaning |
|---|---|
| `E_PROJECT_NOT_FOUND` | Project does not exist |
| `E_TICKET_NOT_FOUND` | Ticket does not exist |
| `E_INVALID_STATUS` | Ticket status is not part of workflow |
| `E_INVALID_TRANSITION` | Ticket status move is not allowed |
| `E_ACCEPTANCE_CRITERIA_REQUIRED` | Ticket creation/import lacked required acceptance criteria |
| `E_TASK_REQUIRED` | Ticket creation/import lacked at least one embedded atomic task |
| `E_TASK_NOT_FOUND` | Embedded task ID was not found on the ticket |
| `E_INVALID_TASK_STATUS` | Task status is not configured or valid |
| `E_INVALID_TASK_SCHEMA` | Task contains unsupported fields or invalid metadata |
| `E_COMPLETION_POLICY` | Ticket cannot move to `done` because acceptance criteria or task completion policy failed |
| `E_LOCK_UNAVAILABLE` | Could not acquire lock |
| `E_INVALID_YAML` | YAML parse failed |
| `E_SCHEMA_VALIDATION` | Required field missing or invalid |
| `E_DUPLICATE_TICKET_ID` | Duplicate ID detected |
| `E_INDEX_STALE` | Index is stale or corrupt |
| `E_GH_NOT_INSTALLED` | `gh` CLI was not found |
| `E_GH_AUTH` | GitHub authentication missing or invalid |
| `E_GH_NOT_FOUND` | GitHub issue or repo not found |
| `E_GH_RATE_LIMIT` | GitHub rate limit or secondary rate limit encountered |
| `E_GH_PERMISSION` | GitHub token lacks required permission |
| `E_GH_CONFLICT` | Sync conflict detected |
| `E_GH_PR_NOT_ISSUE` | Import target is a PR and PR import was not requested |
| `E_CLAIM_HELD` | Ticket is already claimed by another agent or runner |
| `E_NO_CANDIDATES` | No eligible local tickets matched candidate filters |
| `E_BLOCKED_BY_DEPENDENCY` | Ticket is blocked by a non-terminal dependency |
| `E_WORKFLOW_FILE_NOT_FOUND` | Requested `WORKFLOW.md` could not be read |
| `E_WORKFLOW_PARSE` | Requested `WORKFLOW.md` front matter could not be parsed |
| `E_UNSUPPORTED_TRACKER_KIND` | `WORKFLOW.md` tracker kind is not supported by lira helper commands |
| `E_FILESYSTEM` | Filesystem error |

---

## 19. Agent UX Principles

1. **Stable JSON schemas.** Every JSON output includes `schema_version`.
2. **Idempotent commands where possible.** Especially `gh push`, `gh sync`, `claim`, `release`, and `reindex`.
3. **Stable errors.** Every error has a durable `error_code`.
4. **No prompts in JSON mode.** Agents must never hang waiting for input.
5. **Bounded output.** Collection commands support `--limit`, `--cursor`, and `--page-size`.
6. **Structured suggestions.** JSON errors provide machine-readable next-step suggestions.
7. **Explicit sync summaries.** Sync commands return what was pushed, pulled, skipped, or conflicted.
8. **Local-only by default.** Commands never contact GitHub or Jira unless the command namespace or flag explicitly requires it.
9. **No silent claim stealing.** Agent assignment changes must fail if another agent owns the ticket unless `--force` is used.
10. **Conflict safety.** Agents may detect and report conflicts. Destructive conflict resolution should be policy-controlled and may require human approval.
11. **Atomic task discipline.** Agents should break a ticket into small `tasks[]` and mark tasks complete before moving the ticket to `done`.
12. **Ticket-level discussion.** Agents add comments and history at the ticket level, not on embedded tasks.
13. **Poll-loop friendliness.** Repeated read commands must not mutate YAML, advance cursors implicitly, or create noisy logs.
14. **Runner boundary clarity.** lira outputs tracker state and accepts tracker writes; runner prompts, source workspaces, live Codex sessions, retry timers, and stall handling live outside lira.

Example sync summary:

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "id": "ORION-48",
    "action": "sync",
    "sync_state": "synced",
    "pushed": ["state", "labels"],
    "pulled": [],
    "conflicts": [],
    "no_op": false
  }
}
```

---

## 20. Security and Privacy

| Requirement | Details |
|---|---|
| Local permissions | `~/.lira/` should be created with `0700`; YAML and cache files should use `0600` where supported. |
| No telemetry | lira does not collect or transmit telemetry by default. |
| GitHub credentials | lira does not store raw GitHub tokens. It delegates to `gh`. |
| Jira credentials | lira does not store raw Jira credentials. Any Jira fetch delegates to an external configured tool. |
| Sensitive cache | `gh-cache/` may contain issue bodies and must be treated as sensitive. |
| Logs | Logs must avoid writing secrets. GitHub URLs and issue numbers are acceptable; tokens are not. |
| Sync safety | lira must refuse to overwrite changed remote fields without explicit conflict resolution or force. |
| Local-only operation | lira must work for local task management without network access. |

---

## 21. Acceptance Criteria

### 21.1 MVP Core Acceptance Criteria

The MVP is ready when:

1. `lira init` creates `~/.lira/` with expected directories.
2. A user can create a project.
3. A user can create tickets as YAML files.
4. Every created ticket has non-empty acceptance criteria.
5. Every created ticket has at least one embedded atomic task.
6. Embedded tasks have only `id`, `title`, `status`, `tags`, `created_on`, and `last_modified`.
7. Tickets are organized by status directory.
8. Moving a ticket moves its YAML file and updates the `status` field.
9. Ticket history updates on create, move, claim, release, comment, task mutation, link, and sync.
10. Users and agents can add Jira-like comments to tickets.
11. Agents can append structured history events to tickets.
12. Users can add labels/tags, assignees, priorities, dependencies, and child tickets.
13. Agents can claim and release tickets without silently overwriting each other.
14. JSON output exists for all core commands.
15. `lira doctor` validates directory structure, YAML schema, missing acceptance criteria, missing tasks, invalid task fields, status drift, duplicate IDs, stale locks, and stale index state.
16. Write operations use locks and atomic writes.
17. Deterministic YAML output is implemented.
18. No hard-delete command exists in normal workflows.
19. SQLite index can be rebuilt entirely from YAML.

### 21.2 GitHub Acceptance Criteria

GitHub v1 is ready when:

1. A user can bind `ORION-42` to `owner/repo#123`.
2. A bound ticket stores GitHub repo, issue number, URL, node ID, remote state, labels, and sync markers.
3. A user can create a GitHub Issue from a local ticket with `lira gh create`.
4. A user can adopt a GitHub Issue into a new local ticket with `lira gh adopt`.
5. Adopted or imported GitHub issues create local tickets only when acceptance criteria and at least one task are extracted or supplied.
6. A user can pull title, body, state, labels, assignees, comments, milestone, and timestamps from GitHub according to field policy.
7. A user can push title, body, state, labels, assignees, selected comments, acceptance criteria, and task sections to GitHub according to field policy.
8. GitHub labels are stored separately from local labels and task tags.
9. Label sync uses the configured strategy and does not accidentally delete unrelated remote labels.
10. `lira gh push` is idempotent when nothing changed.
11. `lira gh sync` performs three-way reconciliation.
12. lira detects conflicts before overwriting local or remote fields.
13. lira writes conflict metadata and diff files.
14. A user can list conflicts.
15. A user can resolve conflicts by preferring local or remote values.
16. Batch sync returns a structured summary.
17. GitHub sync events are written to JSONL logs.
18. lira works for local-only commands when `gh` is missing or unauthenticated.
19. lira reports missing `gh`, missing auth, missing permissions, and rate limits with structured errors.

### 21.3 Symphony Compatibility Acceptance Criteria

The local orchestration surface is ready when:

1. `lira candidates --json` returns only eligible normalized issue objects.
2. Candidate sorting is deterministic and follows priority, creation time, and identifier.
3. Claimed tickets are excluded from candidates by default.
4. Tickets blocked by non-terminal local blockers are excluded from `todo` candidates.
5. `lira issue show <ID> --json` returns the normalized issue projection without mutating ticket YAML.
6. `lira issue current --ids ... --json` returns current states for reconciliation.
7. `lira claim` serializes concurrent dispatch attempts so exactly one runner wins.
8. Claim and release events are visible in ticket history and JSONL logs.
9. `lira workflow symphony export` emits a `tracker.kind: lira` block suitable for a repository-owned `WORKFLOW.md`.
10. `lira workflow symphony validate` checks tracker fields while ignoring runner-owned `workspace`, `hooks`, `agent`, and `codex` fields.
11. All orchestration helper commands work without network access.
12. No lira command in this surface launches Codex, creates source-code workspaces, or manages live runner state.

---

## 22. Milestones

| Milestone | Scope | Target |
|---|---|---|
| M1 — Skeleton | `init`, `new`, `ls`, `show`, `mv`, required acceptance criteria, required tasks, status-as-directory, `--json` | Week 1–2 |
| M2 — Relationships and ticket activity | `link`, `comment`, `history`, `assign`, `label`, `tag`, embedded task commands, `child_tickets`, dependencies | Week 3 |
| M3 — Symphony-compatible local tracker | Normalized issue projection, candidates, blocker-aware selection, atomic dispatch claims, `WORKFLOW.md` tracker export/validate | Week 4 |
| M4 — Index and search | SQLite + FTS5 if needed, `search`, `query`, `count`, task filters, `reindex` | Week 5 |
| M5 — Jira bridge | `lira jira fetch`, parent caching, Jira reverse links | Week 6 |
| M6 — GitHub bridge: binding and one-way | `gh link`, `gh create`, `gh adopt`, `gh push`, `gh pull`, labels pull/push, body sections for acceptance criteria and tasks | Week 7 |
| M7 — GitHub bridge: bidirectional sync | `gh sync`, three-way reconciliation, conflicts, `gh resolve`, label and user mapping | Week 8–9 |
| M8 — Agent integration | Agent command polish, plugin/MCP-ready JSON, bounded pagination, sync summaries, orchestration logs | Week 10 |
| M9 — Distribution | Cross-builds, release packaging, Homebrew tap, shell completions | Week 11 |

GitHub spans two milestones because one-way binding/push/pull is useful before bidirectional reconciliation is fully robust.

---

## 23. Testing Requirements

### 23.1 Unit Tests

1. Ticket ID generation.
2. Workflow transition validation.
3. Deterministic YAML serialization.
4. Ticket schema validation.
5. Required acceptance criteria validation.
6. Embedded task schema validation.
7. Embedded task uniqueness and task status validation.
8. Unsupported task fields are rejected.
9. Status-directory drift detection.
10. Local hash projection excludes volatile fields.
11. GitHub field policy evaluation.
12. Label mapping and ignore lists.
13. State mapping and state reason mapping.
14. Comment append-only logic.
15. Ticket-level history append logic.
16. Task mutation updates ticket timestamp and history.
17. JSON error serialization.
18. Normalized issue projection maps all required Symphony issue fields.
19. Candidate eligibility excludes claimed, terminal, dispatch-disabled, and locally blocked tickets.
20. Candidate sorting is stable across repeated reads.

### 23.2 Integration Tests

1. `lira init` creates expected structure.
2. Project creation creates all status directories and task status config.
3. Ticket creation writes correct YAML with acceptance criteria and tasks.
4. Ticket creation fails when acceptance criteria or tasks are missing.
5. Moving tickets moves files and updates history.
6. Moving a ticket to `done` fails if task completion policy is not satisfied.
7. Task add/update/status commands mutate only `tasks[]`, parent timestamps, and history.
8. Comment and history commands append to ticket-level arrays.
9. Concurrent claims do not overwrite.
10. Reindex rebuilds SQLite from YAML.
11. `lira doctor` detects invalid YAML, missing acceptance criteria, missing tasks, invalid task fields, stale locks, duplicate IDs, and status drift.
12. GitHub link creates correct binding metadata with a mocked `gh` client.
13. GitHub pull updates local fields according to policy.
14. GitHub adopt/import rejects or skips issues without extractable or supplied acceptance criteria and tasks.
15. GitHub push emits correct `gh` commands or API payloads through the mocked client.
16. GitHub sync no-ops when local and remote are unchanged.
17. GitHub sync pushes when only local changed.
18. GitHub sync pulls when only remote changed.
19. GitHub sync conflicts when both changed overlapping fields.
20. Conflict files are created and resolution updates markers.
21. Sync logs are written to JSONL.
22. Concurrent orchestration claims produce exactly one winner and structured failures for losers.
23. `issue current` returns current states for multiple IDs without mutating YAML.
24. `workflow symphony validate` accepts a valid `tracker.kind: lira` block and ignores runner-owned keys.

### 23.3 Golden Tests

Golden tests should verify stable output for:

1. YAML tickets with acceptance criteria and tasks.
2. JSON command output.
3. Human-readable tables.
4. Conflict diffs.
5. JSONL log events.
6. GitHub body rendering for description, acceptance criteria, and tasks.
7. Normalized issue projection output.
8. Candidate list output with blockers and claims.

---

## 24. Success Metrics

| Metric | Target |
|---|---|
| Agent adoption | At least 3 local agents actively read/write lira within 4 weeks of agent integration. |
| Task discipline | At least 90% of tickets moved to `done` have all embedded tasks terminal. |
| Acceptance criteria coverage | 100% of local tickets have non-empty acceptance criteria. |
| Local list latency | p95 of `lira ls --json` under 100 ms on a 1,000-ticket workspace. |
| Large workspace list latency | p95 of `lira ls --json` under 200 ms on a 10,000-ticket workspace with index. |
| Reliability | Zero data-loss incidents over 90 days. |
| Agent ergonomics | At least 80% of agent ticket operations succeed without retry over 2 weeks. |
| GitHub idempotency | At least 99% of `lira gh sync` invocations on unchanged tickets produce no-op. |
| Sync conflict rate | Under 2% of `lira gh sync` invocations end in conflict over 30 days. |
| Round-trip sync latency | Median local → remote → local single-ticket sync under 10 seconds. |
| Human inspectability | A developer can identify project, status, owner, parent, GitHub link, acceptance criteria, and remaining tasks from the YAML path and file alone. |
| Orchestrator readiness | A local runner can poll candidates, claim work, reconcile states, and write progress using only lira JSON commands. |
| Dispatch safety | Zero duplicate claims in concurrency tests and local dogfood runs. |
| Poll-loop stability | Repeated candidate polling produces no mutations and no noisy YAML diffs. |

---

## 25. Open Questions

1. **GitHub Projects.** v1 ignores Projects. Should v2 map GitHub Projects to lira projects, statuses, saved views, or none of the above?
2. **Comment edit sync.** v1 is append-only. If edits become necessary, should conflict detection apply or should GitHub remain authoritative?
3. **Webhook listener.** Should a future daemon listen for GitHub webhooks for near-real-time remote-ahead detection?
4. **Multi-repo projects.** Project config has one default GitHub repo, but tickets can specify any repo. Is that sufficient?
5. **Reopening semantics.** When a GitHub Issue moves from closed to open, should lira always move to `todo`, restore the previous active status, or use project policy?
6. **Agent conflict resolution.** Should agents be allowed to resolve conflicts automatically under field policies, or should conflicts always escalate to a human?
7. **Custom workflows.** How much workflow customization is needed before it becomes a workflow engine?
8. **GitHub PRs.** Should PRs be linkable separately from issues in v1, or only in v2?
9. **Markdown body structure.** Should lira reserve GitHub Issue body sections for local metadata, or keep metadata entirely outside the body?
10. **Git storage.** Should `~/.lira/` optionally initialize as a git repository for local history?
11. **Task completion policy.** Should a ticket be allowed to move to `done` while any embedded task is not `done` or `cancelled`? Proposed default: warn for humans, fail for agents unless `--force`.
12. **Acceptance criteria satisfaction.** Should acceptance criteria remain plain strings, or should v2 add per-criterion status? Proposed v1: plain strings only; tasks track execution progress.
13. **Runner packaging.** Should a future `lira watch` daemon exist as an optional companion, or should Symphony-style orchestration always remain outside lira?
14. **Tracker extension naming.** Should `tracker.kind: lira` remain local project convention, or should it be proposed upstream as a Symphony tracker adapter?
15. **Workspace hints.** Should lira store runner workspace paths after a run, or should paths remain entirely runner-owned and appear only in comments/history?
16. **Claim expiry.** Should long-held claims have optional leases for unattended runners, or should release/force remain manual policy?

---

## 26. Appendix A — Sample Agent Session

```bash
# scrum-master plans a sprint task and creates a local ticket
lira new "Add CUPED to experiment-analyst" \
  --project ORION \
  --type task \
  --priority high \
  --parent-jira VAN-1234 \
  --assignee athena-analyst \
  --acceptance-criterion "CUPED reduces variance by at least 30% on a holdout dataset." \
  --acceptance-criterion "Generated report includes baseline and adjusted variance." \
  --task "Add SQL covariate extraction." \
  --task "Implement CUPED adjustment calculation." \
  --task "Add tests for null covariates and treatment groups." \
  --json
```

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "id": "ORION-48",
    "status": "backlog",
    "acceptance_criteria_count": 2,
    "tasks": {
      "total": 3,
      "todo": 3
    }
  }
}
```

```bash
# Create the peer GitHub issue and bind it
lira gh create ORION-48 --repo weisberg/agent_tools --json
```

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "id": "ORION-48",
    "github": {
      "repo": "weisberg/agent_tools",
      "issue_number": 201,
      "url": "https://github.com/weisberg/agent_tools/issues/201",
      "sync_state": "synced"
    }
  }
}
```

```bash
# Agent picks it up and works through atomic tasks
lira claim ORION-48 --agent athena-analyst --json
lira mv ORION-48 in-progress --json
lira task status ORION-48 T1 done --json
lira task status ORION-48 T2 in-progress --json
lira comment ORION-48 "Implemented SQL covariate extraction; CUPED calculation is in progress." --json
lira history add ORION-48 \
  --action analysis_note \
  --message "Variance baseline captured from holdout dataset." \
  --actor athena-analyst \
  --json
```

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "id": "ORION-48",
    "from": "backlog",
    "to": "in-progress",
    "github_sync": "local-ahead"
  }
}
```

```bash
# Push state, labels, acceptance criteria, and task section to GitHub
lira gh push ORION-48 --json
```

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "id": "ORION-48",
    "pushed": ["state", "labels", "body"],
    "sync_state": "synced"
  }
}
```

```bash
# At sprint review, reconcile all GitHub-bound tickets
lira gh sync --all --project ORION --json
```

```json
{
  "schema_version": 3,
  "ok": true,
  "result": {
    "summary": {
      "synced": 12,
      "pulled": 1,
      "pushed": 3,
      "conflicts": 1
    },
    "conflicts": [
      {
        "id": "ORION-44",
        "fields": ["body"],
        "diff_path": "~/.lira/gh-cache/conflicts/ORION-44.diff"
      }
    ]
  }
}
```

```bash
# Human reviews and resolves conflict
lira gh diff ORION-44
lira gh resolve ORION-44 --prefer local --json
```

---

## 27. Appendix B — GitHub Field Mapping Reference

| Local field | GitHub field | Direction | Notes |
|---|---|---|---|
| `title` | `title` | Bidirectional | Plain string. |
| `description` | reserved issue body section | Bidirectional | Markdown; conflict policy applies. |
| `acceptance_criteria[]` | reserved issue body section | Policy-based | Required locally; rendered and parsed when body sections are enabled. |
| `tasks[]` | Markdown task list in issue body | Local-first, policy-based | Atomic local to-dos; not separate GitHub Issues in v1. |
| `status` | `state` + `state_reason` | Bidirectional | Uses project mapping. |
| `labels.local` | `labels` | Policy-based | Mapped or unioned into GitHub labels. |
| `labels.github` | `labels` | Policy-based | Pulled from GitHub labels. |
| `github_labels[]` | label metadata | Pull mostly | Name, color, description, default. |
| `assignee` | `assignees[]` | Bidirectional | Uses `user_map`; default cardinality single. |
| `comments[]` | issue comments | Append-only | Edits/deletes not synced in v1. |
| `parent.type == jira` | none | Local-only | No GitHub equivalent. |
| `parent.type == github` | issue body header or relation | Pull-only on adopt | Optional “Tracks: #N” pattern can be recognized on import. |
| `github.repo` + `issue_number` | issue identity | Bidirectional metadata | Peer binding. |
| `child_tickets` | issue body relation section | Local-only in v1 | Legacy `sub_tasks` alias accepted during migration. |
| `milestone` | `milestone` | Pull-only in v1 | Pushing milestones is v2. |
| `time_tracking` | none | Local-only | No GitHub equivalent. |
| `agent_metadata` | none | Local-only | Never synced. |
| `history[]` | none | Local-only | GitHub comments may be mirrored, but lira history is local audit data. |

---

## 28. Appendix C — JSONL Log Examples

### Local ticket move

```json
{
  "schema_version": 3,
  "at": "2026-05-03T14:31:00Z",
  "actor": { "type": "agent", "id": "athena-analyst" },
  "ticket_id": "ORION-48",
  "action": "ticket_moved",
  "from": "backlog",
  "to": "in-progress",
  "result": "ok"
}
```

### Task status change

```json
{
  "schema_version": 3,
  "at": "2026-05-03T14:31:30Z",
  "actor": { "type": "agent", "id": "athena-analyst" },
  "ticket_id": "ORION-48",
  "task_id": "T1",
  "action": "task_status_changed",
  "from": "todo",
  "to": "done",
  "result": "ok"
}
```

### Comment added

```json
{
  "schema_version": 3,
  "at": "2026-05-03T14:32:00Z",
  "actor": { "type": "agent", "id": "athena-analyst" },
  "ticket_id": "ORION-48",
  "comment_id": "local-c7",
  "action": "comment_added",
  "result": "ok"
}
```

### GitHub sync push

```json
{
  "schema_version": 3,
  "at": "2026-05-03T14:33:00Z",
  "actor": { "type": "system", "id": "lira" },
  "ticket_id": "ORION-48",
  "action": "github_sync",
  "repo": "weisberg/agent_tools",
  "issue_number": 201,
  "result": "pushed",
  "fields": ["state", "labels", "body"],
  "sync_state": "synced"
}
```

### GitHub sync conflict

```json
{
  "schema_version": 3,
  "at": "2026-05-03T14:40:00Z",
  "actor": { "type": "system", "id": "lira" },
  "ticket_id": "ORION-44",
  "action": "github_sync",
  "repo": "weisberg/agent_tools",
  "issue_number": 198,
  "result": "conflict",
  "fields": ["body"],
  "diff_path": "~/.lira/gh-cache/conflicts/ORION-44.diff"
}
```

---

## 29. Appendix D — MVP Definition

The MVP of lira is a Rust CLI that lets local users and agents create, track, assign, comment on, break into atomic tasks, move, and complete Jira-like tickets stored as YAML under `~/.lira/`, with statuses represented by directories and all mutations recorded in JSONL logs.

Every local ticket must have acceptance criteria and at least one embedded atomic task. Agents can use lira to add comments and structured history to each local ticket, mirroring the Jira issue comment and activity-history experience while keeping execution state local and inspectable.

The MVP also includes a Symphony-compatible local tracker surface: agents and external runners can list dispatch-eligible candidates, read normalized issue projections, atomically claim work, reconcile current ticket state, and write progress through comments, history, task updates, and status moves without requiring a daemon or network access.

The MVP also includes a first-class GitHub bridge that can bind local tickets to GitHub Issues, push and pull selected fields, synchronize GitHub labels, render acceptance criteria and tasks into GitHub Issue bodies when configured, pull and push append-only comments, and detect conflicts through three-way reconciliation.

The product is successful when an agent can use lira as its local operating task system, a Symphony-style runner can use it as a local issue tracker, and a human can still understand the entire state by inspecting files.
