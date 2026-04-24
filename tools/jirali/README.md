# Jirali

Jirali is an agent-native Jira CLI. It is designed for autonomous agents first
and human terminal users second:

- stdout is JSON in non-TTY mode.
- stderr is structured JSON on non-zero exits.
- exit codes are stable and meaningful.
- local deterministic state is available for tests, dry runs, and agent
  rehearsal.
- `jirali api` and `jirali graphql` provide authenticated Jira escape hatches
  for live Atlassian Cloud/Data Center calls.

## Quickstart

```bash
cargo run -- issue create --project ENG --type Task --summary "Build Jirali"
cargo run -- issue view ENG-1
cargo run -- jql lint 'project = ENG AND status != Done ORDER BY created'
```

Configure a live Jira profile:

```bash
cargo run -- auth login \
  --method api-token \
  --site-url https://example.atlassian.net \
  --email you@example.com \
  --token "$JIRALI_API_TOKEN"
```

Then use the raw REST escape hatch:

```bash
cargo run -- api GET /rest/api/3/myself
```

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General failure |
| 2 | Usage error |
| 3 | Not found |
| 4 | Permission denied |
| 5 | Conflict / idempotent no-op |
| 6 | Rate limited |
| 7 | Validation failed |
| 8 | Timeout |

## Implementation Status

Jirali now has two execution paths:

- **Live Jira-backed:** `api`, `graphql`, and configured-profile paths for
  `issue view/list/create/edit/delete/transition` and `comment` commands. A
  profile is considered live when `site_url` is configured and the profile mode
  is not `local`.
- **Local/fixture-backed:** commands without a live profile continue to use
  deterministic local state under `JIRALI_HOME`. This is intentional for agent
  rehearsal and CI tests.

`jirali tools` reports implementation status per command group so agents can
tell which surfaces are safe for live workflows.

## Roadmap Coverage

The first implementation covers the GitHub roadmap issues for:

- project foundation and stable contracts
- v0.1 core issue, sprint, link, comment, JQL, ADF, auth, audit, and API
  surfaces
- v0.2 aliasing, projections, workflow validation feedback, attachments,
  worklogs, hierarchy, releases, git branch helpers, and skill emission
- v0.3 declarative `plan`/`apply`, `batch`, webhook listen, local cache/audit
  traces, and daemon/MCP placeholders
- v0.4 reports, cross-product helpers, JSM, automation, and pagination-shaped
  outputs
- v0.5 semantic local search, snapshots, diffs, automation import/export, and
  Assets/AQL
- v1.0 hardening surfaces for MCP bridge, PII masking, schema versioning,
  observability metadata, docs, and tests

Live Jira behavior is implemented for the core issue and comment path and
covered by a local mock-Jira integration test. Broader report, JSM,
cross-product, automation, and Assets commands still expose schema-stable
surfaces and deterministic local behavior while their endpoint-specific clients
are expanded.
