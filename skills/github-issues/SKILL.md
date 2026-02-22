---
name: github-issues
description: >
  Use this skill whenever you need to create, read, update, close, comment on, label, assign,
  milestone, pin, lock, transfer, or otherwise manage GitHub Issues using the official GitHub CLI
  (gh). Covers the full issue lifecycle including search, bulk operations, branch development
  from issues, GitHub Projects integration, and the REST API escape hatch for anything gh
  issue subcommands don't expose directly.
requires:
  - gh CLI authenticated (gh auth status)
  - GITHUB_TOKEN or gh auth login completed
  - Run from inside a repo OR pass --repo OWNER/REPO to every command
---

# GitHub Issues Skill

## Setup & Authentication

```bash
# Verify authentication before any issue work
gh auth status

# Authenticate interactively (browser)
gh auth login

# Authenticate via token (CI/agent environments)
export GITHUB_TOKEN=ghp_xxxxxxxxxxxx
gh auth status   # verify it worked

# Set a default repo so --repo can be omitted
gh repo set-default OWNER/REPO

# Confirm the default
gh repo set-default --view
```

## Critical Rules

1. **Always verify auth first.** Every `gh` command fails silently or with cryptic errors if `GITHUB_TOKEN` is missing or expired. Run `gh auth status` at the start of any session.
2. **`--repo` is required outside a git repo.** If you are not inside a cloned repository directory, pass `--repo OWNER/REPO` to every command.
3. **`gh issue edit` replaces, not appends, for labels/assignees.** Use `--add-label` / `--remove-label` and `--add-assignee` / `--remove-assignee` to make surgical changes. Using `--label` will overwrite all existing labels.
4. **Issue numbers are integers, not strings.** Always pass bare numbers: `gh issue view 42`, not `#42`.
5. **`--json` output is machine-readable.** Prefer `--json field1,field2 --jq '.expression'` over parsing human-readable output in scripts.
6. **Closing vs. deleting.** `gh issue close` is reversible. `gh issue delete` is permanent and requires confirmation. Never delete unless explicitly asked.
7. **`gh api` is the escape hatch.** Any feature not available via `gh issue` subcommands is reachable via `gh api`.

---

## Creating Issues

### Basic creation
```bash
gh issue create \
  --title "Descriptive title here" \
  --body "Full issue body in markdown"
```

### Full creation with all metadata
```bash
gh issue create \
  --title "Add CUPED variance reduction to experiment pipeline" \
  --body-file issue-body.md \        # read body from file
  --label "enhancement,analytics" \  # comma-separated labels
  --assignee "@me" \                 # assign to yourself
  --assignee "other-user" \          # assign to another user
  --milestone "Sprint 4" \           # link to milestone
  --project "Analytics Backlog"      # add to GitHub Project
```

### Create from a template
```bash
# List available templates
gh issue create --web   # opens browser, templates available there

# Use body-file to apply a local template
gh issue create \
  --title "Bug: Funnel drop on step 3" \
  --body-file .github/ISSUE_TEMPLATE/bug_report.md
```

### Create and immediately open in browser
```bash
gh issue create --title "Quick note" --body "..." --web
```

### Capture the new issue number in a script
```bash
ISSUE_URL=$(gh issue create --title "..." --body "..." 2>&1 | tail -1)
ISSUE_NUM=$(echo "$ISSUE_URL" | grep -oE '[0-9]+$')
echo "Created issue #$ISSUE_NUM"
```

---

## Viewing Issues

### View a single issue
```bash
gh issue view 42
gh issue view 42 --comments          # include all comment threads
gh issue view 42 --web               # open in browser
```

### View as JSON (preferred for agent parsing)
```bash
# All fields
gh issue view 42 --json number,title,state,body,labels,assignees,milestone,comments,url,createdAt,updatedAt,closedAt,author

# Specific field extraction with jq
gh issue view 42 --json labels --jq '.labels[].name'
gh issue view 42 --json assignees --jq '[.assignees[].login]'
gh issue view 42 --json state --jq '.state'
```

### Check your own issue status dashboard
```bash
gh issue status                       # assigned to you, mentioning you, opened by you
gh issue status --repo OWNER/REPO
```

---

## Listing & Searching Issues

### Basic listing
```bash
gh issue list                         # open issues in current repo
gh issue list --state closed
gh issue list --state all
gh issue list --limit 50              # default is 30
```

### Filter by metadata
```bash
gh issue list --label "bug"
gh issue list --label "bug" --label "priority:high"   # AND logic
gh issue list --assignee "@me"
gh issue list --assignee "username"
gh issue list --author "username"
gh issue list --milestone "Sprint 4"
gh issue list --mention "@me"
```

### Full-text and GitHub search syntax
```bash
# GitHub search syntax via --search
gh issue list --search "memory leak"
gh issue list --search "is:open label:bug no:assignee sort:created-asc"
gh issue list --search "is:open milestone:\"Sprint 4\" sort:updated-desc"
gh issue list --search "involves:username created:>2026-01-01"

# Search across all repos
gh search issues "CUPED variance reduction" --owner myorg
gh search issues "label:sprint-backlog" --repo OWNER/REPO
gh search issues "is:open assignee:@me" --limit 100
```

### JSON output for scripting
```bash
# Get all open issue numbers and titles
gh issue list --state open --limit 100 \
  --json number,title,labels,assignees \
  --jq '.[] | {number, title, labels: [.labels[].name]}'

# Get issues assigned to me as a simple list
gh issue list --assignee "@me" --json number,title --jq '.[] | "#\(.number) \(.title)"'

# Count issues by label
gh issue list --state open --limit 200 \
  --json labels \
  --jq '[.[].labels[].name] | group_by(.) | map({label: .[0], count: length})'
```

---

## Editing Issues

### Safe label management (add/remove, not replace)
```bash
gh issue edit 42 --add-label "in-progress"
gh issue edit 42 --remove-label "sprint-backlog"
gh issue edit 42 --add-label "in-progress" --remove-label "sprint-backlog"  # atomic swap
```

### Assignee management
```bash
gh issue edit 42 --add-assignee "@me"
gh issue edit 42 --add-assignee "teammate"
gh issue edit 42 --remove-assignee "teammate"
```

### Update title or body
```bash
gh issue edit 42 --title "Updated: more precise title"
gh issue edit 42 --body "Completely replaced body content"
gh issue edit 42 --body-file updated-body.md
```

### Update milestone
```bash
gh issue edit 42 --milestone "Sprint 5"
gh issue edit 42 --milestone ""          # remove from milestone
```

### Update project
```bash
gh issue edit 42 --project "Analytics Board"
gh issue edit 42 --project ""            # remove from project
```

---

## Commenting

### Add a plain comment
```bash
gh issue comment 42 --body "Status update: query is running, ETA 30 min."
```

### Comment from a file
```bash
gh issue comment 42 --body-file standup-update.md
```

### Edit a comment (requires comment ID — use API)
```bash
# List comments to find IDs
gh api repos/OWNER/REPO/issues/42/comments --jq '.[] | {id, body: .body[:80]}'

# Edit specific comment
gh api repos/OWNER/REPO/issues/comments/COMMENT_ID \
  --method PATCH \
  --field body="Updated comment text"
```

### Delete a comment
```bash
gh api repos/OWNER/REPO/issues/comments/COMMENT_ID --method DELETE
```

---

## Closing & Reopening Issues

### Close with a reason
```bash
gh issue close 42                        # close (no reason)
gh issue close 42 --reason completed     # completed | not_planned | duplicate
gh issue close 42 --reason not_planned
gh issue close 42 --reason duplicate
```

### Close with a final comment
```bash
gh issue close 42 \
  --comment "Closing: analysis delivered to stakeholders. Outputs at s3://bucket/path."
```

### Reopen
```bash
gh issue reopen 42
gh issue reopen 42 --comment "Reopening: discovered edge case in the data."
```

---

## Label Management

### Create labels for the standard agile workflow
```bash
gh label create "backlog"           --color "C5DEF5" --description "In backlog, not yet scheduled"
gh label create "sprint-backlog"    --color "BFD4F2" --description "Scheduled for current sprint"
gh label create "in-progress"       --color "F9D0C4" --description "Actively being worked on"
gh label create "in-review"         --color "FBCA04" --description "Work complete, under review"
gh label create "blocked"           --color "E4E669" --description "Blocked, needs action"
gh label create "needs-clarification" --color "D4C5F9" --description "Waiting on clarification"
gh label create "done"              --color "0E8A16" --description "Complete and verified"
```

### List labels
```bash
gh label list
gh label list --json name,color,description
```

### Edit a label
```bash
gh label edit "blocked" --description "Updated description" --color "FF0000"
```

### Clone labels from another repo
```bash
gh label clone SOURCE_OWNER/SOURCE_REPO --repo DEST_OWNER/DEST_REPO --force
```

### Delete a label
```bash
gh label delete "wontfix" --yes
```

---

## Milestones (Epics)

Milestones are managed via the GitHub API — `gh issue` doesn't have milestone subcommands.

### Create a milestone
```bash
gh api repos/OWNER/REPO/milestones \
  --method POST \
  --field title="Sprint 4" \
  --field description="Two-week sprint: funnel analysis and experiment setup" \
  --field due_on="2026-03-07T00:00:00Z"
```

### List milestones
```bash
gh api repos/OWNER/REPO/milestones \
  --jq '.[] | {number, title, open_issues, closed_issues, due_on, state}'
```

### Update a milestone
```bash
gh api repos/OWNER/REPO/milestones/MILESTONE_NUMBER \
  --method PATCH \
  --field state="closed"
```

### Assign an issue to a milestone by milestone number
```bash
gh api repos/OWNER/REPO/issues/42 \
  --method PATCH \
  --field milestone=MILESTONE_NUMBER
```

---

## Branch Development from Issues

```bash
# Create a branch linked to an issue (auto-names branch)
gh issue develop 42

# Create with a specific branch name
gh issue develop 42 --name "analytics/42-funnel-drop-analysis"

# Create branch on a specific base
gh issue develop 42 --base main --name "analytics/42-funnel-drop-analysis"

# List branches linked to an issue
gh issue develop 42 --list

# Create branch and immediately check it out
gh issue develop 42 --checkout
```

---

## Pinning & Locking

### Pin / Unpin (highlights issue at top of issue list)
```bash
gh issue pin 42
gh issue unpin 42
```

### Lock / Unlock (prevent new comments)
```bash
gh issue lock 42
gh issue lock 42 --reason "off-topic"      # off-topic | too heated | resolved | spam
gh issue unlock 42
```

---

## Transferring Issues

```bash
# Transfer an issue to another repo in the same org
gh issue transfer 42 DEST_OWNER/DEST_REPO
```

---

## GitHub Projects Integration

### Add issue to a project
```bash
# List your projects to get PROJECT_NUMBER
gh project list --owner OWNER

# Add issue to project
gh project item-add PROJECT_NUMBER \
  --owner OWNER \
  --url "https://github.com/OWNER/REPO/issues/42"
```

### Update project item fields (e.g. status, sprint, priority)
```bash
# Get field IDs for a project
gh project field-list PROJECT_NUMBER --owner OWNER --format json

# Update a single-select field (e.g. Status)
gh project item-edit \
  --project-id PROJECT_ID \
  --id ITEM_ID \
  --field-id FIELD_ID \
  --single-select-option-id OPTION_ID
```

### List items in a project
```bash
gh project item-list PROJECT_NUMBER --owner OWNER --format json
```

### Archive a completed item
```bash
gh project item-archive PROJECT_NUMBER --owner OWNER --id ITEM_ID
```

---

## Bulk Operations

### Close all issues with a label
```bash
gh issue list --label "wontfix" --state open --limit 200 --json number \
  --jq '.[].number' | \
  xargs -I{} gh issue close {} --reason not_planned
```

### Apply a label to a batch of issues
```bash
for num in 10 11 12 13; do
  gh issue edit "$num" --add-label "sprint-backlog" --remove-label "backlog"
done
```

### Bulk comment on a set of issues
```bash
gh issue list --label "blocked" --json number --jq '.[].number' | \
  while read num; do
    gh issue comment "$num" --body "Sprint planning check-in: still blocked? Please update."
  done
```

### Export all open issues to JSON
```bash
gh issue list \
  --state open \
  --limit 500 \
  --json number,title,state,labels,assignees,milestone,createdAt,updatedAt,url \
  > issues-export.json
```

---

## REST API Escape Hatch (`gh api`)

For anything not covered by `gh issue` subcommands, use the REST API directly:

```bash
# Base pattern
gh api repos/OWNER/REPO/issues/NUMBER --method VERB --field key=value

# Get raw issue JSON
gh api repos/OWNER/REPO/issues/42

# Get issue timeline (all events: labeled, assigned, commented, closed, etc.)
gh api repos/OWNER/REPO/issues/42/timeline \
  --header "Accept: application/vnd.github.mockingbird-preview+json" \
  --jq '.[] | {event, created_at, actor: .actor.login}'

# Get issue events only (label, assign, milestone, close events)
gh api repos/OWNER/REPO/issues/42/events \
  --jq '.[] | {event, created_at, label: .label.name}'

# Get all reactions on an issue
gh api repos/OWNER/REPO/issues/42/reactions \
  --jq '.[] | {user: .user.login, reaction: .content}'

# Add a reaction to an issue
gh api repos/OWNER/REPO/issues/42/reactions \
  --method POST \
  --field content="+1"
# Valid reactions: +1, -1, laugh, confused, heart, hooray, rocket, eyes

# Get cross-references (what PRs/issues reference this one)
gh api repos/OWNER/REPO/issues/42/timeline \
  --jq '.[] | select(.event=="cross-referenced") | {source_type: .source.type, number: .source.issue.number, url: .source.issue.html_url}'

# Subscribe/unsubscribe from issue notifications
gh api repos/OWNER/REPO/issues/42/subscription \
  --method PUT \
  --field subscribed=true \
  --field ignored=false

# Lock with reason via API (more options than gh issue lock)
gh api repos/OWNER/REPO/issues/42/lock \
  --method PUT \
  --field lock_reason="resolved"
```

### GraphQL for complex queries
```bash
# Get issues with full comment bodies (REST paginates comments separately)
gh api graphql -f query='
  query($owner: String!, $repo: String!, $number: Int!) {
    repository(owner: $owner, name: $repo) {
      issue(number: $number) {
        number
        title
        state
        body
        labels(first: 20) { nodes { name } }
        assignees(first: 10) { nodes { login } }
        comments(first: 50) {
          nodes { author { login } body createdAt }
        }
      }
    }
  }
' -f owner=OWNER -f repo=REPO -F number=42

# List sprint issues with full metadata
gh api graphql -f query='
  query($owner: String!, $repo: String!, $milestone: String!) {
    repository(owner: $owner, name: $repo) {
      milestone: milestones(query: $milestone, first: 1) {
        nodes {
          title
          issues(first: 50, states: OPEN) {
            nodes { number title state labels(first:10){nodes{name}} assignees(first:5){nodes{login}} }
          }
        }
      }
    }
  }
' -f owner=OWNER -f repo=REPO -f milestone="Sprint 4"
```

---

## Output Formatting Reference

```bash
# --json fields available for gh issue view / gh issue list:
# number, title, body, state, stateReason, url, createdAt, updatedAt, closedAt,
# author, assignees, labels, milestone, comments, projectCards, projectItems,
# reactions, isPinned, locked, id

# jq patterns
gh issue list --json number,title,labels \
  --jq '.[] | select(.labels | map(.name) | contains(["blocked"]))'

# Template formatting (Go template syntax)
gh issue list --json number,title \
  --template '{{range .}}#{{.number}}: {{.title}}{{"\n"}}{{end}}'
```

---

## Workflow Patterns

### Agile sprint planning sequence
```bash
# 1. Review backlog
gh issue list --label "backlog" --state open --limit 50 --json number,title,labels

# 2. Move selected issues to sprint-backlog
for num in 10 15 22 31; do
  gh issue edit "$num" \
    --add-label "sprint-backlog" \
    --remove-label "backlog" \
    --milestone "Sprint 5"
done

# 3. Assign issues
gh issue edit 10 --add-assignee "@me"
gh issue edit 15 --add-assignee "teammate"

# 4. Comment confirming scope
gh issue comment 10 --body "**Sprint Planning**
Taking this on in Sprint 5. Scope: [assumption 1, assumption 2]. Will update at start of work."
```

### Start work on an issue
```bash
# Move to in-progress, create branch, check it out
ISSUE=42
gh issue edit "$ISSUE" --add-label "in-progress" --remove-label "sprint-backlog"
gh issue develop "$ISSUE" --name "analytics/${ISSUE}-$(gh issue view $ISSUE --json title --jq '.title | ascii_downcase | gsub("[^a-z0-9]"; "-") | .[0:40]')" --checkout
gh issue comment "$ISSUE" --body "**Status Update**
- Done since last session: Sprint planning
- Doing now: Starting analysis
- Blocked: None"
```

### Close an issue with full DoD comment
```bash
ISSUE=42
gh issue edit "$ISSUE" --add-label "done" --remove-label "in-review"
gh issue close "$ISSUE" --reason completed \
  --comment "**Closing — Definition of Done verified**

- [x] Acceptance criteria met (see checklist above)
- [x] Outputs: \`s3://bucket/20260221_T42_analysis.parquet\` — 42,000 rows
- [x] Key findings: [summary]
- [x] Caveats: [any]
- Related: Unblocks #43, see also #38"
```

### Blocked escalation
```bash
ISSUE=42
gh issue edit "$ISSUE" --add-label "blocked" --remove-label "in-progress"
gh issue comment "$ISSUE" \
  --body "**BLOCKED**
- What: Cannot access \`prod_analytics.events\` table — permission denied
- Need: Read access grant from data engineering
- Who: @data-eng-lead
- Workaround: None — will move to #45 in the meantime"
```

### Sprint review summary
```bash
# Collect closed issues from the sprint milestone
gh issue list \
  --milestone "Sprint 4" \
  --state closed \
  --json number,title,closedAt \
  --jq '.[] | "#\(.number) \(.title) — closed \(.closedAt)"'

# Collect carried-over issues
gh issue list \
  --milestone "Sprint 4" \
  --state open \
  --json number,title,labels
```

---

## Quick Reference

| Action | Command |
|--------|---------|
| Create issue | `gh issue create --title "..." --body "..."` |
| View issue | `gh issue view NUMBER` |
| View with comments | `gh issue view NUMBER --comments` |
| List open issues | `gh issue list` |
| List by label | `gh issue list --label "in-progress"` |
| Search issues | `gh issue list --search "query"` |
| Add label | `gh issue edit NUMBER --add-label "label"` |
| Remove label | `gh issue edit NUMBER --remove-label "label"` |
| Swap label | `gh issue edit NUMBER --add-label "in-progress" --remove-label "sprint-backlog"` |
| Assign to self | `gh issue edit NUMBER --add-assignee "@me"` |
| Unassign | `gh issue edit NUMBER --remove-assignee "user"` |
| Comment | `gh issue comment NUMBER --body "..."` |
| Close (completed) | `gh issue close NUMBER --reason completed` |
| Close with comment | `gh issue close NUMBER --comment "..."` |
| Reopen | `gh issue reopen NUMBER` |
| Create branch | `gh issue develop NUMBER --name "analytics/NUMBER-slug" --checkout` |
| Pin issue | `gh issue pin NUMBER` |
| Lock issue | `gh issue lock NUMBER --reason resolved` |
| Transfer issue | `gh issue transfer NUMBER OWNER/REPO` |
| Export to JSON | `gh issue list --state all --limit 500 --json number,title,state,labels,url` |
| Raw API call | `gh api repos/OWNER/REPO/issues/NUMBER` |