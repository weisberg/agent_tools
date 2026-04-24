---
name: jirali
description: Use Jirali to inspect, create, transition, reconcile, report on, and audit Jira work from an agent-safe CLI.
---

# Jirali Skill

Use `jirali` when Jira state needs to be read or changed from a shell workflow.

## Agent Contract

- Treat stdout as the only data channel.
- Treat stderr as structured error data.
- Branch on exit codes:
  - `0`: success
  - `2`: fix command syntax
  - `3`: search or update assumptions
  - `4`: stop and ask a human for auth/permission help
  - `5`: idempotent desired state already holds
  - `6`: back off and retry later
  - `7`: retry with validator-required fields
  - `8`: narrow the query or increase timeout

## Common Commands

```bash
jirali issue view ENG-123 --view-profile dev
jirali issue list --jql 'project = ENG ORDER BY updated DESC' --limit 20
jirali issue create --project ENG --type Task --summary 'Follow up'
jirali issue transition ENG-123 'In Progress'
jirali jql lint 'project = ENG AND status != Done'
jirali plan jira-state.yaml
jirali apply jira-state.yaml
jirali report velocity --jql 'project = ENG'
```

## Rich Text

Use Markdown for ordinary comments and descriptions:

```bash
jirali comment add ENG-123 --markdown 'Ready for @reviewer'
```

Use raw ADF only when you already have valid ADF JSON:

```bash
jirali comment add ENG-123 --body-adf @comment.adf.json
```

## Live Jira Escape Hatch

When a named command does not yet expose a Jira endpoint directly, use:

```bash
jirali api GET /rest/api/3/issue/ENG-123
jirali graphql --query '{ ... }'
```

Credentials come from the active profile configured with `jirali auth login`.
