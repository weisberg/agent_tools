# Keeping a Jira-like Project Management System with notionli

This document describes how to run a Jira-like project management system in
Notion while using `notionli` as the agent-safe control plane. The goal is not
to clone every Jira feature. The goal is to make project work addressable,
scriptable, auditable, and safe for agents and humans sharing the same Notion
workspace.

`notionli` is a good fit when the tracker should live where planning notes,
decision records, specs, and meeting notes already live. It gives agents stable
commands for finding work, creating rows, updating properties, patching issue
pages, exporting reports, recording receipts, and rehearsing writes before
anything touches the workspace.

## Executive Summary

Use Notion for the human interface and `notionli` for deterministic operations.
Treat every issue as a Notion page inside an `Issues` data source. Store durable
identity in explicit properties such as `Key` and `ExternalID`. Use aliases for
stable data-source names, dry-run every write first, and rely on operation
receipts, snapshots, and audit logs for recovery.

The working model is:

- `db`: a Notion database container.
- `ds`: a Notion data source, the actual table that owns issue properties.
- `row`: a Notion page inside a data source, representing one issue.
- page body: the issue description, acceptance criteria, task checklist,
  decisions, links, and implementation notes.
- row properties: the structured board fields used for filtering, sorting,
  reporting, and automation.

The most important commands are:

```bash
notionli auth whoami
notionli sync pull
notionli search "Engineering Issues" --recent
notionli alias set issues data_source:<issues-data-source-id>
notionli ds schema issues
notionli page create --parent issues --title "PROJ-42 Add typed status writes" --set ExternalID=agent-tools:PROJ-42
notionli row update PROJ-42 --set AgentState="In Progress"
notionli row relate PROJ-42 "Blocks" PROJ-41
notionli page patch PROJ-42 --section "Progress Log" --append-md update.md
notionli ds export issues --format csv --out issues.csv
notionli snapshot create --out ./notion-snapshot
notionli op list
```

Writes are dry-run plans by default. Add global `--apply` only after the plan is
correct:

```bash
notionli --apply row update PROJ-42 --set AgentState="In Review"
```

## Typed Property Setter Behavior

`notionli` can inspect the raw Notion schema for select, status, and
multi-select properties through `ds schema`, including the available options
when Notion returns them. Mutating commands that know the target data source now
use that schema to encode `--set KEY=VALUE` as native Notion property payloads.

Schema-aware setters currently cover:

- `title`
- `rich_text`
- `number`
- `checkbox`
- `date`
- `select`
- `status`
- `multi_select`
- `relation`
- `people`
- `url`
- `email`
- `phone_number`

Select, status, and multi-select values are validated against available schema
options when options are present. Files remain handled through `notionli file
attach`; formula, rollup, created/edited, and unique ID properties are treated
as read-only. Relation and people setters expect IDs, not human names. Use
`row relate` when you want `notionli` to resolve another row target for you.

If no schema is available, `notionli` falls back to the legacy best-effort
encoding: booleans, numbers, dates, and rich text. For production workflows,
run `notionli ds schema <alias>` or target a data source directly so the schema
is known before writing native Notion fields.

## System Goals

A Jira-like Notion tracker should support:

- Stable issue keys such as `PROJ-42`.
- Idempotent creation and updates from agents, scripts, and imports.
- Backlog, sprint, review, blocked, and done workflows.
- Epics, stories, tasks, bugs, chores, and spikes.
- Assignees, owners, labels, due dates, story points, and priority.
- Blockers and parent/child relationships.
- Page bodies with acceptance criteria and implementation notes.
- Daily standup, sprint planning, sprint review, and release reporting.
- Local exports, snapshots, and audit trails.
- Safe automation through dry-run plans and policy files.

Non-goals:

- Replacing Jira's enterprise permission model.
- Replacing full workflow validators, notification schemes, or advanced
  portfolio reporting.
- Making Notion the only source of truth when a team already has mandated Jira.
  In that case, use Notion as a planning/mirror layer and keep `ExternalID`
  pointed at the Jira key.

## Recommended Workspace Layout

Create one project-management home page in Notion:

```text
Project Management
|-- Issues
|-- Projects
|-- Sprints
|-- Releases
|-- People
|-- Templates
|-- Dashboards
`-- Archive
```

Minimum viable setup:

- `Issues`: one row per issue.
- `Projects`: one row per project or product area.
- `Sprints`: one row per sprint or timebox.
- `Archive`: a page or database for completed/obsolete material.

Useful optional setup:

- `Releases`: one row per release train, milestone, or launch.
- `People`: one row per person if Notion people properties are not enough.
- `Components`: one row per subsystem.
- `Risks`: one row per cross-cutting risk or dependency.
- `Decision Log`: a page or data source for durable decisions.

After creating or locating the data sources, register aliases:

```bash
notionli search "Issues" --recent
notionli alias set issues data_source:<issues-data-source-id>
notionli alias set projects data_source:<projects-data-source-id>
notionli alias set sprints data_source:<sprints-data-source-id>
notionli alias list
```

Agents and scripts should use aliases instead of raw UUIDs. Raw IDs are still
best for one-off exact targeting, but aliases make recurring workflows readable.

## Issue Schema

The `Issues` data source is the center of the system. Keep the schema boring and
stable. Agents do best when property names are exact, short, and durable.

Recommended properties:

| Property | Suggested Notion type | Setter behavior | Purpose |
|---|---|---|---|
| `Name` | title | use `page create --title` / `page rename` | Human-readable title, usually `KEY Summary`. |
| `Key` | rich text or unique ID | rich text is safe | Stable display key such as `PROJ-42`. |
| `ExternalID` | rich text | safe | Idempotency key such as `agent-tools:PROJ-42`, `jira:PROJ-42`, or `github:org/repo#123`. |
| `AgentState` | rich text | safe fallback | Optional workflow mirror. |
| `Status` | status | schema-aware | Native Notion board status. |
| `AgentType` | rich text | safe fallback | Optional issue-type mirror. |
| `Type` | select | schema-aware | Native issue type. |
| `Priority` | number | safe | Sortable priority, for example 0 critical through 4 low. |
| `PriorityName` | select or rich text | rich text safe | Human label such as Critical, High, Medium, Low. |
| `StoryPoints` | number | safe | Estimate used for planning. |
| `Rank` | number | safe | Backlog ordering. Lower is higher priority. |
| `Due` | date | safe | Due date or target date. |
| `Done` | checkbox | safe | Fast boolean completion marker. |
| `OwnerText` | rich text | safe | Current-compatible assignee field. |
| `Assignee` | people | schema-aware by user ID | Native Notion person assignment. |
| `SprintText` | rich text | safe | Current-compatible sprint marker. |
| `Sprint` | relation to `Sprints` | use `row relate` carefully | Native sprint relation. |
| `EpicText` | rich text | safe | Current-compatible epic key. |
| `Epic` | relation to `Issues` | use `row relate` carefully | Native epic/story relation. |
| `Blocks` | relation to `Issues` | use `row relate` carefully | Issues blocked by this issue. |
| `BlockedByText` | rich text | safe | Current-compatible blocker keys. |
| `LabelsText` | rich text | safe | Comma-separated labels for current writes. |
| `Labels` | multi-select | schema-aware | Native Notion labels. |
| `Branch` | rich text | safe | Git branch name. |
| `PR` | rich text | safe | Pull request URL or number. |
| `CI` | rich text | safe | CI status summary or URL. |
| `LastAgent` | rich text | safe | Agent/session that last touched the issue. |
| `LastUpdate` | date | safe | Last explicit tracker update date. |

For a stricter Notion-native tracker, add native option properties and keep
their values constrained:

```text
Status:
  Backlog
  Ready
  In Progress
  Blocked
  In Review
  Done
  Canceled

Type:
  Epic
  Story
  Task
  Bug
  Chore
  Spike

PriorityName:
  Critical
  High
  Medium
  Low
```

Inspect options before relying on them:

```bash
notionli ds schema issues
notionli ds schema issues | jq -r '.schema.Status.status.options[].name'
notionli ds schema issues | jq -r '.schema.Type.select.options[].name'
notionli ds schema issues | jq -r '.schema.Labels.multi_select.options[].name'
```

The exact JSON shape comes from Notion. Some workspaces use `select.options`,
some use `status.options` and status groups, and relation properties include
the related data-source identifiers.

## Issue Page Body Template

Properties are for filtering and automation. The page body is for the work.
Every issue page should have predictable headings so `page section` and
`page patch --section` can update only the intended area.

Suggested issue body:

```markdown
# PROJ-42 Add typed status writes

## Summary

One paragraph explaining the user-visible problem and desired outcome.

## Acceptance Criteria

- [ ] Status/select properties can be written without raw API payloads.
- [ ] Invalid option names fail before mutation.
- [ ] Dry-run output shows the exact typed payload.

## Implementation Notes

- Use `ds schema` to resolve property type and options.
- Preserve fallback `--set` behavior for simple properties when no schema is available.

## Dependencies

- Blocks: PROJ-41
- Blocked by:

## Progress Log

- 2026-05-10: Created.

## Review Notes

## Release Notes
```

Register a reusable template:

```bash
notionli template register issue --from ./issue-template.md
notionli template list
```

Use `template apply` when creating a free-standing page. For rows inside the
`Issues` data source, prefer `page create --parent issues --title ... --md ...`
so the row has a correct title and body in one operation.

## Initial Setup Workflow

1. Authenticate and confirm workspace access:

```bash
notionli auth whoami
notionli doctor round-trip <disposable-shared-page>
```

2. Hydrate the local cache:

```bash
notionli sync pull
notionli sync status
```

3. Find the project-management data sources:

```bash
notionli search "Issues" --recent
notionli search "Sprints" --recent
notionli search "Projects" --recent
```

4. Register aliases:

```bash
notionli alias set issues data_source:<issues-data-source-id>
notionli alias set sprints data_source:<sprints-data-source-id>
notionli alias set projects data_source:<projects-data-source-id>
notionli resolve issues
```

5. Inspect schema:

```bash
notionli ds schema issues
notionli ds get issues
```

6. Smoke-test a disposable issue without applying:

```bash
notionli page create \
  --parent issues \
  --title "TEST-1 Disposable notionli PM smoke" \
  --set ExternalID=smoke:TEST-1 \
  --set Key=TEST-1 \
  --set AgentState=Backlog \
  --set AgentType=Task \
  --set Priority=3 \
  --set Done=false
```

7. Apply the smoke only in a disposable workspace or after review:

```bash
notionli --apply page create \
  --parent issues \
  --title "TEST-1 Disposable notionli PM smoke" \
  --set ExternalID=smoke:TEST-1 \
  --set Key=TEST-1 \
  --set AgentState=Backlog \
  --set AgentType=Task \
  --set Priority=3 \
  --set Done=false
```

8. Confirm the row can be found and patched:

```bash
notionli search "TEST-1" --recent
notionli alias set TEST-1 page:<created-page-id>
notionli row update TEST-1 --set AgentState="In Progress"
notionli page patch TEST-1 --section "Progress Log" --append-text "Smoke update."
```

9. Archive or trash the smoke row:

```bash
notionli page trash TEST-1 --confirm-title "TEST-1 Disposable notionli PM smoke"
notionli --apply page trash TEST-1 --confirm-title "TEST-1 Disposable notionli PM smoke"
```

## Creating Issues

Use `page create` for reliable row creation because it has a first-class
`--title` path. Put the stable key at the beginning of the title.

Dry run:

```bash
notionli page create \
  --parent issues \
  --title "PROJ-42 Add typed status writes" \
  --md ./issues/PROJ-42.md \
  --set ExternalID=agent-tools:PROJ-42 \
  --set Key=PROJ-42 \
  --set AgentState=Backlog \
  --set AgentType=Story \
  --set Priority=1 \
  --set StoryPoints=3 \
  --set OwnerText=Priya \
  --set LastUpdate=today
```

Apply:

```bash
notionli --apply page create \
  --parent issues \
  --title "PROJ-42 Add typed status writes" \
  --md ./issues/PROJ-42.md \
  --set ExternalID=agent-tools:PROJ-42 \
  --set Key=PROJ-42 \
  --set AgentState=Backlog \
  --set AgentType=Story \
  --set Priority=1 \
  --set StoryPoints=3 \
  --set OwnerText=Priya \
  --set LastUpdate=today
```

After creation, set an alias for frequently edited issues:

```bash
notionli alias set PROJ-42 page:<created-page-id>
```

For idempotent imports from another system, `row upsert` is the intended shape:

```bash
notionli row upsert issues \
  --key ExternalID=jira:PROJ-42 \
  --set AgentState=Backlog \
  --set Priority=1 \
  --set LastUpdate=today
```

Because `row upsert` still uses a simple generated query for matching, validate
the upsert key against your schema before using it for production issue
creation. A common safe pattern is:

- use `page create` for new rows with titles and bodies;
- use `ExternalID` to avoid duplicates in scripts;
- use `row update` for simple fields after the row exists;
- use `ds import --upsert-key` or batches after smoke-testing the matching
  behavior for your chosen `ExternalID` property.

## Updating Issues

Simple field update:

```bash
notionli row update PROJ-42 \
  --set AgentState="In Progress" \
  --set LastAgent=codex \
  --set LastUpdate=today
```

Apply after reviewing the dry-run:

```bash
notionli --apply row update PROJ-42 \
  --set AgentState="In Progress" \
  --set LastAgent=codex \
  --set LastUpdate=today
```

Rename the issue:

```bash
notionli page rename PROJ-42 "PROJ-42 Add schema-aware property setters"
notionli --apply page rename PROJ-42 "PROJ-42 Add schema-aware property setters"
```

Append a progress note:

```bash
printf '%s\n' "- 2026-05-10: Implemented parser tests." > update.md
notionli page patch PROJ-42 --section "Progress Log" --append-md update.md
notionli --apply page patch PROJ-42 --section "Progress Log" --append-md update.md
```

Replace acceptance criteria from a file:

```bash
notionli page patch PROJ-42 \
  --section "Acceptance Criteria" \
  --replace-md acceptance.md
```

Fetch the issue page in agent-safe form:

```bash
notionli page fetch PROJ-42 --format agent-safe --budget 4000
```

Read just one section:

```bash
notionli page section PROJ-42 "Acceptance Criteria"
notionli page outline PROJ-42 --with-block-ids
```

## Workflow States

Use a small state machine. Too many states make agents uncertain and humans
argue with the board instead of doing work.

Recommended states:

| State | Meaning | Entry requirement | Exit requirement |
|---|---|---|---|
| `Backlog` | Captured but not ready. | Title and rough summary. | Acceptance criteria and priority. |
| `Ready` | Ready for implementation. | Clear scope, owner, no unresolved blockers. | Work starts or scope changes. |
| `In Progress` | Actively being worked. | Owner or agent claimed the work. | Implementation ready for review or blocked. |
| `Blocked` | Cannot proceed. | Blocker recorded in body or relation. | Blocker resolved or accepted. |
| `In Review` | Implementation complete enough to inspect. | Evidence attached: PR, diff, tests, or notes. | Accepted, changes requested, or canceled. |
| `Done` | Complete. | Acceptance criteria satisfied and review complete. | Reopen only with a note. |
| `Canceled` | Intentionally stopped. | Reason recorded. | Usually terminal. |

The native transition command is:

```bash
notionli row update PROJ-42 --set Status="In Review" --set LastUpdate=today
```

If you keep a rich-text mirror for external sync or simple exports, update both:

```bash
notionli row update PROJ-42 --set Status="In Review" --set AgentState="In Review"
```

## Epics, Parents, and Blockers

Use relations for human-friendly Notion navigation, but also keep text mirrors
for easy agent writes and external sync.

Suggested relationship fields:

- `Epic`: relation to `Issues`.
- `Parent`: relation to `Issues`.
- `Blocks`: relation to `Issues`.
- `Blocked By`: relation to `Issues`.
- `EpicText`: rich text mirror such as `PROJ-1`.
- `BlockedByText`: rich text mirror such as `PROJ-40, PROJ-41`.

Set text mirrors:

```bash
notionli row update PROJ-42 --set EpicText=PROJ-1 --set BlockedByText=PROJ-41
```

Set a relation:

```bash
notionli row relate PROJ-42 "Blocked By" PROJ-41
notionli --apply row relate PROJ-42 "Blocked By" PROJ-41
```

Be careful with relation updates. The current `row relate` command writes a
relation payload for the named property and related row. Before using it on a
multi-relation property that already has values, fetch the row and verify
whether the command appends or replaces the relation list in your workspace.
For blocker-heavy workflows, preserve `BlockedByText` as the agent-safe mirror.

## Sprints

The `Sprints` data source should be small and durable.

Recommended sprint properties:

| Property | Type | Purpose |
|---|---|---|
| `Name` | title | `Sprint 2026-20`, `Iteration 8`, or similar. |
| `Start` | date | Start date. |
| `End` | date | End date. |
| `Goal` | rich text | One-sentence sprint goal. |
| `State` | rich text or status | Planned, Active, Closed. |
| `Capacity` | number | Optional point or day capacity. |

Create a sprint:

```bash
notionli page create \
  --parent sprints \
  --title "Sprint 2026-20" \
  --set Start=2026-05-11 \
  --set End=2026-05-22 \
  --set State=Planned \
  --set Capacity=24
```

Assign an issue by text mirror:

```bash
notionli row update PROJ-42 --set SprintText="Sprint 2026-20"
```

Assign by relation after validating relation behavior:

```bash
notionli row relate PROJ-42 Sprint "Sprint 2026-20"
```

Export sprint candidates:

```bash
notionli ds export issues --format csv --where SprintText="Sprint 2026-20" --out sprint-2026-20.csv
```

For live Notion filtering with `ds query`, string filters are compiled as
select-style conditions. That is useful for native select/status fields. For
rich-text mirror fields, prefer cache-backed `ds export --where` or use raw
Notion filters through `ds query --filter '<json>'`.

## Backlog Grooming

Backlog grooming should be a read-plan-write loop.

1. Pull or refresh local state:

```bash
notionli sync pull
```

2. Export the backlog:

```bash
notionli ds export issues \
  --format csv \
  --where AgentState=Backlog \
  --out backlog.csv
```

3. Find stale or duplicate work:

```bash
notionli search --duplicates
notionli search --stale
notionli ds deduplicate issues --by ExternalID --keep newest
```

4. Re-rank candidates with dry-run updates:

```bash
notionli row update PROJ-42 --set Rank=10 --set Priority=1
notionli row update PROJ-43 --set Rank=20 --set Priority=2
```

5. Apply only after the plan is reviewed:

```bash
notionli --apply row update PROJ-42 --set Rank=10 --set Priority=1
notionli --apply row update PROJ-43 --set Rank=20 --set Priority=2
```

For many updates, use a JSONL batch.

`rank-updates.jsonl`:

```jsonl
{"op":"row.update","target":"PROJ-42","set":{"Rank":10,"Priority":1,"LastUpdate":"today"}}
{"op":"row.update","target":"PROJ-43","set":{"Rank":20,"Priority":2,"LastUpdate":"today"}}
```

Preview:

```bash
notionli batch apply rank-updates.jsonl
```

Apply:

```bash
notionli --apply batch apply rank-updates.jsonl
```

## Daily Standup

A daily standup view should answer:

- What changed since yesterday?
- What is in progress?
- What is blocked?
- What needs review?
- What should be pulled next?

Useful commands:

```bash
notionli sync pull --since 2026-05-09
notionli query save in-progress --source issues --where 'AgentState="In Progress"' --sort 'Priority asc'
notionli query save blocked --source issues --where 'AgentState=Blocked' --sort 'Priority asc'
notionli query save review --source issues --where 'AgentState="In Review"' --sort 'Priority asc'
notionli query run in-progress
notionli query run blocked
notionli query run review
```

If these saved queries target rich-text mirror fields, use `ds export --where`
instead of `query run`, or use raw Notion filter JSON through `ds query
--filter`. The saved-query path delegates to `ds query`, whose simple string
conditions are best suited for native select/status properties.

Append standup notes to an issue:

```bash
cat > standup-note.md <<'EOF'
- 2026-05-10: Validating native status/select property updates.
EOF

notionli page patch PROJ-42 --section "Progress Log" --append-md standup-note.md
notionli --apply page patch PROJ-42 --section "Progress Log" --append-md standup-note.md
```

Meeting notes can also be mined for actions:

```bash
notionli meeting list
notionli meeting get <meeting-block-id> --actions
```

Convert extracted actions into issue rows with `page create`, `row upsert`, or
batch files after reviewing the generated actions.

## Sprint Planning

Sprint planning should produce:

- a sprint goal;
- a committed issue list;
- estimates and owners;
- explicit blockers;
- a receipt of what changed.

Suggested flow:

```bash
notionli sync pull
notionli ds export issues --format csv --where AgentState=Ready --out ready.csv
notionli page create --parent sprints --title "Sprint 2026-20" --set State=Planned
```

Create `sprint-plan.jsonl`:

```jsonl
{"op":"row.update","target":"PROJ-42","set":{"SprintText":"Sprint 2026-20","AgentState":"Ready","Rank":10,"LastUpdate":"today"}}
{"op":"row.update","target":"PROJ-43","set":{"SprintText":"Sprint 2026-20","AgentState":"Ready","Rank":20,"LastUpdate":"today"}}
{"op":"page.patch","target":"PROJ-42","section":"Progress Log","append_text":"Planned into Sprint 2026-20."}
```

Preview and apply:

```bash
notionli batch apply sprint-plan.jsonl
notionli --apply batch apply sprint-plan.jsonl
notionli op list --limit 10
```

If the team uses native Notion sprint relations, relation updates can be added
after validating relation append behavior:

```jsonl
{"command":["row","relate","PROJ-42","Sprint","Sprint 2026-20"]}
```

## Sprint Review and Closeout

At review time:

1. Export completed and incomplete issues:

```bash
notionli ds export issues --format csv --where SprintText="Sprint 2026-20" --out sprint-2026-20-all.csv
notionli ds export issues --format csv --where Done=true --out done.csv
```

2. Fetch issue sections for review notes:

```bash
notionli page section PROJ-42 "Review Notes"
notionli page section PROJ-42 "Release Notes"
```

3. Move accepted issues to done:

```bash
notionli row update PROJ-42 --set AgentState=Done --set Done=true --set LastUpdate=today
notionli --apply row update PROJ-42 --set AgentState=Done --set Done=true --set LastUpdate=today
```

4. Carry over unfinished work:

```bash
notionli row update PROJ-43 --set SprintText="Sprint 2026-21" --set AgentState=Ready
```

5. Close the sprint:

```bash
notionli row update "Sprint 2026-20" --set State=Closed
```

## Releases

The release data source is optional but useful for larger efforts.

Recommended release properties:

| Property | Type | Purpose |
|---|---|---|
| `Name` | title | Release name or version. |
| `TargetDate` | date | Planned release date. |
| `State` | rich text/status | Planned, Stabilizing, Released, Canceled. |
| `Branch` | rich text | Release branch. |
| `NotesPage` | rich text/url | Link to release notes page. |

Mark an issue for a release:

```bash
notionli row update PROJ-42 --set ReleaseText=v1.4.0
```

Append release notes:

```bash
notionli page patch PROJ-42 --section "Release Notes" --append-text "Adds typed Notion property writes."
```

Export release issues:

```bash
notionli ds export issues --format md --where ReleaseText=v1.4.0 --out release-v1.4.0.md
```

## Reports and Dashboards

Use Notion views for human dashboards. Use `notionli` exports for deterministic
agent and CI reports.

Common reports:

```bash
notionli ds export issues --format csv --out all-issues.csv
notionli ds export issues --format jsonl --where AgentState=Blocked --out blocked.jsonl
notionli ds export issues --format md --where AgentState="In Review" --out review.md
notionli sync diff
notionli audit list
```

For board-like terminal inspection, use the Notion UI or export to a local tool.
`notionli` should stay focused on stable operations and structured output.

## Automation Patterns

### Batch Files

Use JSONL batch files for multi-issue updates. They are easy to review, easy to
regenerate, and compatible with dry-run planning.

Supported structured operations include:

- `alias.set`
- `alias.remove`
- `select`
- `page.patch`
- `row.create`
- `row.update`
- `row.upsert`
- `comment.add`

Raw command arrays are also supported:

```jsonl
{"command":["row","update","PROJ-42","--set","AgentState=Done","--set","Done=true"]}
```

Prefer structured operations when generating batches:

```jsonl
{"op":"row.update","target":"PROJ-42","set":{"AgentState":"Done","Done":true,"LastUpdate":"today"}}
{"op":"page.patch","target":"PROJ-42","section":"Progress Log","append_text":"Accepted in review."}
```

### Workflows

Workflows live under the active `notionli` home in `workflows/`, or they can be
addressed by direct file path. They may be JSON, JSONL, YAML, or YML. Variables
use `{{NAME}}` replacement and are supplied with `--set NAME=value`.

Example `workflows/start-work.yml`:

```yaml
steps:
  - op: row.update
    target: "{{ISSUE}}"
    set:
      AgentState: "In Progress"
      LastAgent: "{{AGENT}}"
      LastUpdate: today
  - op: page.patch
    target: "{{ISSUE}}"
    section: "Progress Log"
    append_text: "Started by {{AGENT}}."
```

Preview:

```bash
notionli workflow run start-work --set ISSUE=PROJ-42 --set AGENT=codex
```

Apply:

```bash
notionli --apply workflow run start-work --set ISSUE=PROJ-42 --set AGENT=codex
```

### Templates

Use templates for repeated page bodies. Keep template variables simple:

```markdown
# {{KEY}} {{SUMMARY}}

## Summary

{{DESCRIPTION}}

## Acceptance Criteria

- [ ] {{ACCEPTANCE_CRITERION}}

## Progress Log

- {{DATE}}: Created by {{CREATOR}}.
```

Register:

```bash
notionli template register issue --from ./issue-template.md
```

Apply:

```bash
notionli template apply issue \
  --parent issues \
  --set KEY=PROJ-42 \
  --set SUMMARY="Add typed status writes" \
  --set DATE=2026-05-10 \
  --set CREATOR=codex
```

For data-source rows, `page create --parent issues --title ... --md ...` is
usually more direct because it sets the title property correctly.

## Policies and Safety

Use policy files to restrict what an agent or automation can do.

Example `notionli-pm-policy.json`:

```json
{
  "allow": [
    "resolve",
    "search",
    "ds.schema",
    "ds.export",
    "ds.query",
    "page.fetch",
    "page.section",
    "page.outline",
    "page.patch",
    "row.update",
    "row.upsert",
    "comment.add",
    "op.list",
    "op.show",
    "audit.list"
  ],
  "deny": [
    "page.trash",
    "row.trash",
    "ds.bulk-archive",
    "ds.schema.apply",
    "bulk.rename"
  ]
}
```

Check a command:

```bash
notionli policy check notionli-pm-policy.json row update PROJ-42 --set AgentState=Done
```

Enforce for an invocation:

```bash
notionli --policy notionli-pm-policy.json row update PROJ-42 --set AgentState=Done
```

Recommended policy stance:

- Read-only agents: allow search, resolve, schema, fetch, export, query.
- Planning agents: read-only plus dry-run batch/workflow commands.
- Project-management agents: allow `row.update`, `page.patch`, and comments.
- Admin agents: allow schema changes, bulk archive, trash, and restore only in
  dedicated sessions.

## Audit, Receipts, and Recovery

Every applied write should leave a receipt or audit trail.

Review recent operations:

```bash
notionli op list --limit 20
notionli op show <operation-id>
notionli audit list
notionli audit show <operation-id>
```

Undo when an operation has an inverse:

```bash
notionli op undo <operation-id>
notionli --apply op undo <operation-id>
```

Take snapshots before large planning or migration sessions:

```bash
notionli snapshot create --out ./snapshots/pre-sprint-planning
notionli snapshot create --out ./snapshots/post-sprint-planning
notionli snapshot diff ./snapshots/pre-sprint-planning ./snapshots/post-sprint-planning
```

Restore a row or page from a snapshot only after inspecting the plan:

```bash
notionli snapshot restore-row <row-id> --from ./snapshots/pre-sprint-planning
notionli --apply snapshot restore-row <row-id> --from ./snapshots/pre-sprint-planning
```

## External Tracker Sync

If Jira remains the source of truth, use Notion as the planning and context
layer. Keep these conventions:

- `ExternalID=jira:PROJ-42`
- `Key=PROJ-42`
- title starts with `PROJ-42`
- page body contains Notion-native planning notes, decisions, and local context
- Jira-only fields are mirrored only if agents need them

Import a JSONL file generated from Jira:

```jsonl
{"Key":"PROJ-42","ExternalID":"jira:PROJ-42","AgentState":"Backlog","Priority":1,"OwnerText":"Priya"}
{"Key":"PROJ-43","ExternalID":"jira:PROJ-43","AgentState":"Ready","Priority":2,"OwnerText":"Marco"}
```

Preview import:

```bash
notionli ds import issues --jsonl-file jira-export.jsonl --upsert-key ExternalID
```

Apply after validation:

```bash
notionli --apply ds import issues --jsonl-file jira-export.jsonl --upsert-key ExternalID
```

For current production use, smoke-test this import path against your schema.
If title creation or native select/status updates are required, generate a
batch that uses `page create` for new issues and `row update` for known rows.

## Agent Operating Contract

Agents working in this tracker should follow this contract:

1. Resolve aliases before writing:

```bash
notionli resolve issues
notionli resolve PROJ-42
```

2. Inspect schema before writing unfamiliar properties:

```bash
notionli ds schema issues
```

3. Fetch issue context before modifying page body:

```bash
notionli page fetch PROJ-42 --format agent-safe --budget 4000
```

4. Dry-run every mutation:

```bash
notionli row update PROJ-42 --set AgentState="In Review"
```

5. Apply only after the plan matches the user's intent:

```bash
notionli --apply row update PROJ-42 --set AgentState="In Review"
```

6. Record why a meaningful transition happened:

```bash
notionli page patch PROJ-42 \
  --section "Progress Log" \
  --append-text "Moved to In Review after tests passed."
```

7. Check receipts:

```bash
notionli op list --limit 5
```

8. Never use title search as the only selector when multiple rows may match.
Prefer page IDs, aliases, or stable `ExternalID`-based workflows.

## Definition of Ready and Done

Definition of Ready:

- `Name` starts with the issue key.
- `Key` and `ExternalID` are set.
- `AgentState` is `Ready` or native `Status` is `Ready`.
- Summary is clear.
- Acceptance criteria exist.
- Priority is set.
- Owner or ownership rule is clear.
- Blockers are empty or explicitly accepted.

Definition of Done:

- All acceptance criteria are satisfied.
- Tests, review evidence, or validation notes are linked.
- `AgentState=Done` and `Done=true`.
- Progress log records the completion.
- Release notes are present when user-visible behavior changed.
- Follow-up issues are created or linked.

Example closeout:

```bash
cat > done-note.md <<'EOF'
- 2026-05-10: Completed. Added typed property schema inspection docs and verified command examples.
EOF

notionli page patch PROJ-42 --section "Progress Log" --append-md done-note.md
notionli row update PROJ-42 --set AgentState=Done --set Done=true --set LastUpdate=today
```

## Maintenance Schedule

Daily:

- `notionli sync pull`
- review blocked and in-review work
- append progress notes for active issues
- move stale active issues back to `Ready` or `Blocked`

Weekly:

- `notionli ds export issues --format csv --out issues-weekly.csv`
- `notionli search --duplicates`
- `notionli ds deduplicate issues --by ExternalID --keep newest`
- close stale done work into archive if the team uses archiving
- snapshot before and after sprint planning

Monthly:

- review schema drift with `ds schema`
- validate policy files
- audit recent operations
- export a full JSONL backup
- prune or archive canceled work

Commands:

```bash
notionli sync pull
notionli ds export issues --format jsonl --out backups/issues-$(date +%Y-%m-%d).jsonl
notionli snapshot create --out snapshots/notion-$(date +%Y-%m-%d)
notionli audit list
```

## Common Failure Modes

| Symptom | Likely cause | Fix |
|---|---|---|
| `E_AUTH_MISSING` | No token or OAuth profile. | Configure `NOTION_API_KEY`, `~/.config/NOTION_API_KEY`, OAuth, or `--token-cmd`. |
| Row cannot be resolved | Alias/cache missing or title ambiguous. | Run `sync pull`, `search`, then `alias set KEY page:<id>`. |
| Native `Status` update fails | Schema was unavailable or option name did not match exactly. | Run `ds schema`, confirm the option, and retry with the exact name. |
| Query misses rich-text mirror fields | `ds query --where` compiles strings as select filters. | Use `ds export --where` from cache or raw `--filter` JSON. |
| Duplicate issues appear | No stable `ExternalID` or non-idempotent creation path. | Use `ExternalID`, deduplicate by key, and prefer upsert/import workflows after schema smoke tests. |
| Relation update loses previous values | Relation payload replaced the relation list. | Fetch before relation writes; keep text mirrors; add append semantics before relying on multi-relations. |
| Plan looks too broad | Selector resolved too many or wrong rows. | Use aliases, page IDs, exact keys, and small batch files. |
| Agent writes fields it should not touch | Policy too broad. | Use `--policy` with allow/deny rules. |

## Recommended notionli Enhancements for First-Class PM

The project-management workflow becomes much stronger with these future
features:

- A typed `--set-json` escape hatch for advanced properties.
- Relation append/remove operations that preserve existing relation values.
- A `row transition` command that validates state changes against a workflow
  file.
- A `row claim` command for agent ownership and WIP limits.
- A key allocator, for example `notionli issue next-key PROJ`.
- Built-in saved views for `blocked`, `ready`, `in-review`, and `stale`.
- Report helpers for sprint burndown, cycle time, and throughput.
- A Jira bridge that maps Jira keys to `ExternalID` and stores sync receipts.

Until those exist, the safe pattern is to inspect schema first, keep mirrors
where external systems benefit from them, and rehearse all writes with dry-run
plans.

## Minimal Production Checklist

- [ ] `Issues`, `Projects`, and `Sprints` data sources exist.
- [ ] `issues`, `projects`, and `sprints` aliases are registered.
- [ ] `ExternalID` is required by convention for every issue.
- [ ] Issue titles start with `KEY`.
- [ ] Native `Status`, `Type`, `Labels`, `Priority`, and date/number fields are
      inspected with `ds schema` before automation writes.
- [ ] Optional mirror fields such as `AgentState`, `OwnerText`, and
      `BlockedByText` exist only where external sync or simple exports need
      them.
- [ ] Issue page template has stable headings.
- [ ] Smoke create/update/patch/trash has passed in a disposable row.
- [ ] Policy files exist for read-only, planning, PM-writer, and admin modes.
- [ ] `sync pull`, `ds export`, and `snapshot create` are part of the routine.
- [ ] Agents always dry-run first and inspect receipts after applying.

With those pieces in place, Notion becomes a practical, human-friendly project
hub, and `notionli` gives agents the stable rails they need to operate it
without treating the workspace like an opaque web app.
