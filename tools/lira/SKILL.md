---
name: lira
description: >-
  Operate the lira CLI, a local agent-native task system where local tickets are
  sub-tickets of canonical Jira parents and are decomposed into acceptance
  criteria plus atomic tasks. Use when the user wants to break down a Jira
  ticket into agent work; create, claim, work, comment on, sync, query, or close
  local tickets; manage acceptance criteria and atomic tasks; resolve GitHub
  sync conflicts; or coordinate local task assignment through ~/.lira/.
---

# lira - Local Jira for Agents

`lira` is a Rust CLI that gives agents a durable, inspectable, local workspace
for decomposing Jira tickets into agent-executable subtickets. A local lira
ticket is normally a child of a canonical Jira ticket: Jira holds the work item
the team agreed on; lira holds the agent's local breakdown into acceptance
criteria and atomic tasks the agent can execute, comment on, and complete
without touching Jira directly.

Tickets live as YAML under `~/.lira/`, can optionally bind to a peer GitHub
Issue for outside visibility, and never write back to Jira in v1. Jira is
read-only context.

## Mental Model

The expected hierarchy has three layers:

1. Jira ticket, such as `VAN-1234`: the canonical, team-visible work item.
   Read-only from lira's perspective. Holds the high-level "what" and "why."
2. lira ticket, such as `ORION-48`: the local agent's decomposition of one Jira
   ticket into a single agent-executable scope. Has acceptance criteria and
   atomic tasks.
3. lira tasks, such as `T1`, `T2`: atomic to-dos inside one lira ticket. Each is
   small enough to finish in a single focused step.

Normal Jira-to-lira flow:

1. Fetch current Jira context with `lira jira fetch VAN-1234 --json`.
2. Create a local working ticket with `lira new ... --parent-jira VAN-1234`,
   including at least one `--acceptance-criterion` and one `--task`.
3. Claim, move, drive tasks, comment, and accumulate history.
4. Optionally mirror the ticket to a GitHub Issue with `lira gh create`.
5. Close the lira ticket when its local scope is done. The Jira parent remains
   untouched; humans update Jira from lira history or the synced GitHub Issue.

One Jira parent can have multiple lira children if the work splits across
agents, sprints, or scopes. Do not cram a multi-scope Jira ticket into one lira
ticket.

A lira ticket without a Jira parent is allowed but should be the exception, such
as agent-internal infrastructure work with no team-level counterpart. If the
user gives work without a Jira reference, ask whether one exists before creating
a parentless ticket.

## Preflight

Before doing real work, confirm lira is installed and the workspace exists:

```bash
lira --version
ls ~/.lira/ 2>/dev/null || echo "not initialized"
lira project list --json
```

If `~/.lira/` does not exist, ask the user before running `lira init`. That
directory is the canonical store for all their tickets across every project, not
scoped to the current repo.

## Core Rules

These are operational requirements, not style preferences:

1. Always pass `--json`. Every command supports stable, versioned JSON output
   with `schema_version`, `ok`, and either `result` or `error`. Parse those
   fields. Human-format output can change shape.
2. Acceptance criteria are mandatory. A ticket cannot exist without a non-empty
   `acceptance_criteria` list. Creation, GitHub adoption, and import all fail
   with `E_ACCEPTANCE_CRITERIA_REQUIRED` if missing.
3. Tasks are mandatory. Every ticket must have at least one entry in `tasks[]`.
   The same enforcement returns `E_TASK_REQUIRED`.
4. Tasks are atomic and minimal. A task has only `id`, `title`, `status`,
   `tags`, `created_on`, and `last_modified`. If a task needs comments,
   assignees, descriptions, estimates, or external links, promote it to a child
   ticket with `lira child add`.
5. Status lives in the directory. A ticket's location, such as
   `~/.lira/projects/ORION/tickets/in-progress/ORION-42.yaml`, is part of the
   source of truth alongside the `status` field. Use `lira mv` to change status.
   Never edit YAML or move files manually.
6. There is no hard delete. Cancel, archive, or release tickets. Removing a
   ticket file bypasses history, the index, and any GitHub binding.
7. Claim before mutating someone else's work. `lira claim <ID> --agent <name>`
   fails if another agent owns the ticket. Respect that signal unless the user
   explicitly tells you to steal with `--force`.
8. The completion guard is real. `lira mv <ID> done` fails with
   `E_COMPLETION_POLICY` unless acceptance criteria exist and all tasks are
   terminal (`done` or `cancelled`). Drive tasks to terminal first.
9. Default to a Jira parent. Parentless tickets are a deliberate exception, not
   a default.
10. YAML is canonical. SQLite at `~/.lira/index/tickets.sqlite` and
    `~/.lira/gh-cache/` are caches. If they look wrong, run `lira reindex`; do
    not hand-edit.

## Create From Jira

Start by pulling the Jira parent so you have its title, description, and context
to decompose against:

```bash
lira jira fetch VAN-1234 --json
```

Create the local subticket with `--parent-jira` and at least one
`--acceptance-criterion` and one `--task`. Both flags are repeatable:

```bash
lira new "Implement CUPED variance reduction in experiment-analyst" \
  --project ORION \
  --parent-jira VAN-1234 \
  --type task \
  --priority high \
  --assignee athena-analyst \
  --acceptance-criterion "Variance reduced by >=30% on holdout dataset." \
  --acceptance-criterion "Report includes both baseline and adjusted variance." \
  --task "Add SQL covariate extraction." \
  --task "Implement CUPED adjustment calculation." \
  --task "Add tests for null covariates and treatment groups." \
  --json
```

The response returns the new ticket ID, such as `ORION-48`, and default status,
usually `backlog`. Capture the ID for follow-up calls.

## Decompose Work

At creation time, translate the Jira parent's prose into two distinct artifacts:

- Acceptance criteria: observable, testable outcomes a reviewer could check
  without watching the agent work. Pull from Jira language when possible.
- Tasks: atomic implementation steps the agent can execute one at a time. These
  are the agent's decomposition and usually do not appear in Jira.

A criterion is what done looks like to an outside observer. A task is one move
toward getting there.

If the Jira parent is too broad for one lira ticket, create multiple sibling lira
tickets sharing the same `--parent-jira`, each with its own focused scope,
criteria, and tasks.

If the user gives only a Jira key and rough direction, draft criteria and tasks
yourself and show them before submitting `lira new`. Let the user edit if they
want. Do not ask the user to enumerate every task.

## Pick Up Work

```bash
lira next --project ORION --agent athena-analyst --json
lira claim ORION-48 --agent athena-analyst --json
lira mv ORION-48 in-progress --json
lira active --agent athena-analyst --json
```

`lira next` returns the highest-priority unclaimed candidate. If none exist, the
result is empty. Surface that to the user; do not invent work or claim
arbitrarily.

Use `lira active --agent <name> --json` at the start of a session to recover
state for an agent that already owns work.

## Work Tasks

Each task mutation automatically appends a history entry on the parent ticket
and updates `timestamps.updated`. Do not manage history manually for ordinary
task moves.

```bash
lira task list ORION-48 --json
lira task status ORION-48 T1 in-progress --json
lira task status ORION-48 T1 done --json
lira task add ORION-48 "Document new CUPED config knobs." --tag docs --json
lira task tag add ORION-48 T2 statistics --json
lira task cancel ORION-48 T3 --json
```

Task statuses are `todo`, `in-progress`, `blocked`, `done`, and `cancelled`.

When a task cannot be completed as written, do not silently retitle it. Choose
one of:

- Add a tighter sibling with `lira task add`.
- Cancel the original with `lira task cancel` and add a replacement.
- Promote to a child ticket with `lira child add` if the work has outgrown
  atomic task shape.

Cancellation is recorded in history; retitling hides the change.

## Comments And History

Comments are free-form, Jira-style, human-readable notes attached to the ticket.
Tasks intentionally have no comments.

History is the structured, machine-readable, append-only event stream. lira
writes history automatically for mutations; agents add structured entries with
`lira history add` for durable analysis notes or decisions.

```bash
lira comment ORION-48 "Variance baseline = 0.0234 on holdout slice." --json

lira history add ORION-48 \
  --action analysis_note \
  --message "CUPED assumptions validated against pre-treatment period." \
  --actor athena-analyst \
  --json
```

For long bodies, pipe via stdin:

```bash
lira comment ORION-48 --stdin --json <<'EOF'
Detailed multi-line note...
With supporting context.
EOF
```

If a comment should also be pushed to the bound GitHub Issue, mark it for sync:

```bash
lira comment sync ORION-48 local-c7 --github --json
```

GitHub comment sync is append-only in v1. Edits and deletes are not propagated.

## Close Tickets

```bash
lira mv ORION-48 in-review --json
lira mv ORION-48 done --json
```

`mv done` fails with `E_COMPLETION_POLICY` if any task is non-terminal or if
acceptance criteria are missing. Fix the underlying state by finishing or
cancelling each task. `--force` is a human override, not an agent shortcut.

## Links And Dependencies

The Jira parent is normally set at creation via `--parent-jira`. Use link
commands for everything else: lira-to-lira parents, blocking relationships,
related tickets, and child tickets.

lira distinguishes two GitHub relationships:

- GitHub parent (`--parent-github`): an upward tracking relationship, usually
  pointing at a GitHub tracking issue that aggregates work. Local-only; not
  synced.
- GitHub peer binding: the ticket's `github` block, a bidirectional 1:1 sync
  relationship with a single GitHub Issue.

A ticket can have a Jira parent and a GitHub peer binding to a different GitHub
Issue at the same time.

```bash
lira link ORION-48 --jira VAN-1234 --json
lira link ORION-48 --parent-lira ORION-12 --json
lira link ORION-48 --parent-github example-org/repo#100 --json
lira link ORION-48 --blocks ORION-50 --json
lira link ORION-48 --relates-to ORION-30 --json
lira child add ORION-48 ORION-49 --json
```

For visibility into Jira-parented work:

```bash
lira jira sync-parents --json
lira ticket list --parent-jira VAN-1234 --json
```

## GitHub Sync

GitHub Issues are first-class peer sync targets. lira shells out to `gh` and
never stores raw tokens. If `gh` is missing or unauthenticated, sync commands
return `E_GH_NOT_INSTALLED` or `E_GH_AUTH`, and local-only commands keep
working. Do not block local progress on GitHub availability.

Bind GitHub Issues:

```bash
# Create a fresh GitHub Issue from an existing local ticket and bind it.
lira gh create ORION-48 --repo weisberg/agent_tools --json

# Bind a local ticket to an existing GitHub Issue.
lira gh link ORION-48 weisberg/agent_tools#142 --json

# Adopt a single GitHub Issue into a new local ticket.
lira gh adopt weisberg/agent_tools#142 \
  --project ORION \
  --acceptance-criterion "Reproduce and fix the reported regression." \
  --task "Reproduce locally from the issue body." \
  --json

# Bulk-adopt GitHub Issues.
lira gh import weisberg/agent_tools \
  --project ORION \
  --state open \
  --label bug \
  --acceptance-criteria-file ./default-ac.yaml \
  --task-template "Triage imported GitHub issue." \
  --json
```

`gh adopt` and `gh import` enforce the same mandatory acceptance criteria and
tasks rules. If the GitHub body does not contain parseable `## Acceptance
Criteria` and `## Tasks` sections, provide them via flags.

Push, pull, and sync:

```bash
lira gh pull ORION-48 --json
lira gh push ORION-48 --json
lira gh sync ORION-48 --json
lira gh sync --all --project ORION --json
lira gh status ORION-48 --json
```

`sync_state` values are `synced`, `local-ahead`, `remote-ahead`, `conflict`,
`unbound`, and `disabled`. Always check `sync_state` before assuming either side
is authoritative.

## Conflict Handling

`lira gh sync` detects conflicts via three-way reconciliation against
`last_synced`, `remote_etag` or `remote_body_hash`, and `local_hash`. When it
returns a conflict, stop and report it. Do not blindly resolve.

```bash
lira gh conflicts --json
lira gh conflicts show ORION-48 --json
lira gh diff ORION-48
cat ~/.lira/gh-cache/conflicts/ORION-48.diff
```

Resolve only after understanding which side should win:

```bash
lira gh resolve ORION-48 --prefer local --json
lira gh resolve ORION-48 --prefer remote --json
```

If a human is in the loop, surface the diff and affected fields, such as
`["body"]`, and ask before choosing. If local has tasks or acceptance criteria
that the remote body lacks, `--prefer local` is often right, but confirm.

## Search, Query, And Board

```bash
lira ls --project ORION --json
lira show ORION-48 --json
lira search "token refresh" --json
lira query --status in-progress --label rust --json
lira query --task-status blocked --json
lira query --task-tag sql --json
lira count --project ORION --group-by status --json
lira board --project ORION --json
lira active --agent athena-analyst --json
```

For large workspaces use `--limit` and `--cursor` to paginate. Do not fetch
everything when you only need a slice.

## Validation And Recovery

```bash
lira doctor --json
lira validate --json
lira reindex --json
```

Run `lira doctor` after anything unusual: an interrupted command, a manual edit
by the user, suspected drift between status field and directory, or a GitHub
operation that errored mid-flight. The index can always be rebuilt; canonical
YAML cannot.

## Filesystem Inspection

The directory layout is part of the product. When debugging or grepping, read it
directly:

```bash
ls ~/.lira/projects/ORION/tickets/in-progress/
cat ~/.lira/projects/ORION/tickets/in-progress/ORION-48.yaml
tail ~/.lira/logs/$(date -u +%Y-%m-%d).jsonl
ls ~/.lira/gh-cache/conflicts/
```

Read freely, but write only through lira commands. Direct writes bypass locks
(`~/.lira/locks/`), atomic-rename semantics, and the JSONL audit log.

## Common Errors

Check `error.error_code` on JSON failures.

| Code | Response |
|---|---|
| `E_ACCEPTANCE_CRITERIA_REQUIRED` | Add `--acceptance-criterion` flags and retry. |
| `E_TASK_REQUIRED` | Add `--task` flags and retry. |
| `E_COMPLETION_POLICY` | Drive remaining tasks to `done` or `cancelled`; do not auto-force. |
| `E_INVALID_TRANSITION` | Run `lira project show <P> --json` to see allowed transitions for the current status. |
| `E_INVALID_TASK_STATUS` / `E_INVALID_TASK_SCHEMA` | Re-read the task schema rules. Tasks have only six fields. |
| `E_LOCK_UNAVAILABLE` | Another process holds the lock; wait briefly and retry, or run `lira doctor` if stale. |
| `E_GH_NOT_INSTALLED` / `E_GH_AUTH` | Local-only ops still work. Tell the user to install `gh` or run `gh auth login`. |
| `E_GH_CONFLICT` | Three-way conflict; read the diff and ask the user before resolving. |
| `E_GH_RATE_LIMIT` | Back off and retry later. Do not hammer. |
| `E_TICKET_NOT_FOUND` / `E_PROJECT_NOT_FOUND` | Verify with `lira ls` or `lira project list`. |
| `E_INVALID_YAML` / `E_SCHEMA_VALIDATION` | Run `lira doctor`; usually points at a file the user edited by hand. |

The full list is in `lira_prd_v0_4.md` section 18.3.

## Anti-Patterns

Avoid:

- Editing ticket YAML directly.
- Removing ticket files directly; use `lira mv <ID> cancelled` or
  `lira archive <ID>`.
- Treating embedded tasks as separate tickets.
- Skipping `--json`.
- Forcing past the completion guard.
- Adding comments or assignees to tasks.
- Hand-editing `~/.lira/gh-cache/`.
- Silently stealing a claim.
- Running `lira gh sync --all` after a long hiatus without checking for
  conflicts.
- Creating a parentless lira ticket when a Jira parent exists.
- Trying to write back to Jira. `lira jira fetch` and `lira jira sync-parents`
  only pull.
- Cramming a multi-scope Jira ticket into one lira ticket. Create siblings
  sharing the same `--parent-jira` instead.
