# Jirali: Product Requirements & Engineering Design Document

| Field | Value |
|---|---|
| **Document status** | Draft v0.1 |
| **Last updated** | 2026-04-24 |
| **Primary author** | Brian Weisberg |
| **Product area** | Developer tooling / agent-native infrastructure |
| **Parent document** | *Architecting Jirali: A Dual-Purpose Command Line Interface* (foundation document; referenced as FD throughout) |
| **Audience** | Engineering, internal platform users, agent authors |
| **Review cadence** | Per phase milestone |

---

## 1. Executive Summary

### 1.1 Product in one sentence

**Jirali is an agent-first command-line interface for Atlassian Jira, designed to be consumed primarily by autonomous AI agents and secondarily by human operators, with strict stdout/stderr contracts, a granular exit code taxonomy, first-class Atlassian Document Format (ADF) handling, and optional daemon/cache/semantic-search extensions.**

### 1.2 Why build this

Existing Jira interfaces (REST v2/v3, GraphQL, Rovo MCP, ACLI, `ankitpokhrel/jira-cli`, `go-jira`) were designed before agent-driven workloads were a primary consumer. The Rovo MCP server, while architecturally legitimate, imposes a context-window tax of 40k–55k tokens before the first operational prompt in multi-server configurations and introduces persistent middleware with non-trivial failure modes. Existing CLIs are human-first: they interactively prompt, emit ANSI-decorated output to stdout, and do not segregate data from diagnostics. Neither path is suitable for tight agent loops over large Jira datasets.

Jirali fills this gap with an agent-native CLI that is fast, stateless by default, token-efficient, and composable with existing Unix tooling.

### 1.3 Scope summary

- **In scope (this document):** MVP through v1 feature set, system architecture, interface contracts, security model, observability, phased delivery plan, risks.
- **Out of scope (this document):** Detailed per-command specifications beyond the MVP subset (Appendix C), GUI or TUI beyond TTY-detected affordances, non-Atlassian issue trackers.

### 1.4 Delivery summary

Five phases, v0.1 (MVP) through v1.0, with explicit entry/exit criteria per phase. MVP target: 12 weeks. v1.0 target: 36 weeks from project start.

---

## 2. Background

### 2.1 Current Jira access landscape

The FD covers this in depth; a summary is retained here for reviewers without the FD at hand.

**REST v2 / v3 (Atlassian Cloud and Data Center):** Fully featured but verbose, requires multiple sequential calls for relational data, and (in v3) mandates Atlassian Document Format JSON for rich-text fields. Cloud v3's new `/rest/api/3/search/jql` endpoint uses cursor-based pagination and no longer returns total counts; `startAt` is deprecated in that path.

**Atlassian Platform GraphQL API:** Minimizes payloads and enables cross-product queries (Jira, Bitbucket, Compass, Opsgenie) and cross-product entities (Teams, Goals). Benchmarks show per-request latency roughly 2× optimized REST calls with higher variance.

**Atlassian Rovo MCP Server (GA, February 2026):** Cloud-based MCP gateway for Jira, Confluence, Compass, JSM. Uses OAuth 2.1 or API token authentication; permission groups gate tool access. Endpoint moved from `/v1/sse` to `/v1/mcp` (SSE deprecated, sunset 2026-06-30). Supports dynamic client registration. JSM Operations tools (alerts, on-call, schedules) are API-token-only. Surfaces Teamwork Graph data (linked pull requests, builds, deployments) from GitHub, GitLab, Azure DevOps, Jenkins, and Spinnaker. Atlassian Resource Identifiers (ARIs) are the opaque handles used in cross-product `fetch` calls.

**Community MCP (`sooperset/mcp-atlassian`):** Supports Jira Data Center via Personal Access Token, filling a gap Rovo MCP does not serve.

**`ankitpokhrel/jira-cli`:** Mature human-first Go CLI. Recent additions include JSON and CSV output, worklog CRUD, and a raw `api` passthrough. Remains designed around an interactive TUI and human affordances.

**`go-jira`:** Open-source Go CLI originally from Netflix. Stateless, customizable via YAML, deeply Unix-native, but entirely human-oriented.

**Atlassian ACLI (official):** Java-based, interactive, strong on administrative operations, weak on ADF input handling (e.g., `--body-file` returns mixed parse output when given ADF; no per-endpoint `--body-adf` handler).

### 2.2 Specific gaps

1. **No Jira CLI emits strict machine-first JSON on stdout with strict diagnostics on stderr by default.** Existing CLIs provide opt-in `--json` flags but mix warnings, progress indicators, or colorized output into stdout.
2. **No Jira CLI surfaces a granular exit code taxonomy differentiating not-found, permission denied, conflict/idempotent-noop, and usage error.** All collapse these to 0/1.
3. **No Jira CLI has first-class ADF handling with bidirectional Markdown conversion covering more than ~30% of ADF node types.**
4. **No Jira CLI offers plan/apply semantics, idempotent `ensure` subcommands, or a batch-operation DSL with referenceable creates.**
5. **No Jira CLI offers a webhook listener that blocks on a filtered event with structured output.**
6. **No Jira CLI integrates local semantic search over cached issues.**
7. **No Jira CLI exposes JQL linting, workflow transition validator feedback, or reports (velocity, burndown, CFD, cycle time) as structured-JSON emitters.**
8. **No Jira CLI offers per-tool correlation IDs and a local audit trail distinguishing agent-driven from human-driven activity.**

### 2.3 Why now

- Rovo MCP reaching GA codifies the baseline for enterprise-accepted Jira-agent access. An agent-native CLI alternative with aligned security posture now has a clear frame of reference for admins and security reviewers.
- Claude Code, Cursor, and similar agentic IDEs have converged on shell-executed tools as the preferred execution surface for long-horizon workflows. The CLI-as-tool paradigm is no longer speculative.
- Atlassian's Cloud v3 cursor pagination and Teamwork Graph enrichment change the data-fetching cost structure. A purpose-built CLI can abstract these cleanly; general wrappers increasingly cannot.

---

## 3. Goals and Non-Goals

### 3.1 Primary goals

| ID | Goal | Measurement |
|---|---|---|
| G1 | Enable agents to perform any core Jira operation without reading raw REST or GraphQL responses | ≥90% of top-50 agent use cases covered by named subcommands |
| G2 | Minimize per-invocation token cost vs. Rovo MCP | Median tokens per operation ≤20% of Rovo MCP baseline |
| G3 | Eliminate middleware as a source of failure | Command success rate ≥99.5% when Jira API is healthy |
| G4 | Provide deterministic, self-correcting error output for agents | 100% of error exits include structured stderr with error code, message, and suggested remediation |
| G5 | Preserve a usable human interactive experience | TTY-mode affordances (prompts, tables, spinners, colors) without leaking into pipes |

### 3.2 Secondary goals

| ID | Goal |
|---|---|
| S1 | Integrate cleanly with adjacent internal tools (tooli, vaultli, vizli, agentcli, embedd, sqlservd, sheetcraft, mdx, docli) |
| S2 | Support Jira Data Center deployments as a first-class target, not a Cloud afterthought |
| S3 | Ship Claude Code / Cursor skills that document high-leverage subcommand chains |
| S4 | Emit time-series JSON for agile reports suitable for downstream analytics pipelines |
| S5 | Provide an MCP bridge mode so jirali can be consumed by MCP-only clients |

### 3.3 Non-goals

| ID | Non-goal | Rationale |
|---|---|---|
| N1 | Feature parity with the Jira web UI | Administrative UIs (e.g., scheme design, project templates) are low-leverage for agent workloads |
| N2 | Full Confluence / Compass / Goals editing surfaces | Cross-product operations limited to linking and lightweight lookups |
| N3 | Hosted service | Jirali is a local binary; no long-running hosted component except the optional local daemon |
| N4 | Proprietary protocol | All interactions use documented Atlassian REST, GraphQL, or MCP endpoints |
| N5 | AI-inference features | Any natural-language capabilities (e.g., JQL generation) are opt-in integrations, not built into jirali itself |

### 3.4 Explicit deferrals

Deferred past v1.0 unless reprioritized:

- Jira Align integration
- Tempo time-tracking deep integration
- Xray test management
- Marketplace-app specific custom field types beyond the common set
- Jira Mobile push notifications

---

## 4. Personas and User Stories

### 4.1 Personas

**P1 — Autonomous coding agent (primary).** Non-interactive subprocess invocation. Parses stdout as JSON; branches on exit codes; reads stderr only on non-zero exit. Token-constrained. Expected to self-correct within 3–5 retries for recoverable errors. Examples: Claude Code in an agentic loop, a GitHub Actions job invoking jirali to update ticket status on PR merge.

**P2 — Developer human (secondary).** Interactive terminal user. Expects readable tables, colorized output, tab completion, readline editing, and interactive prompting when required arguments are missing. Primary friction today is the latency of the Jira web UI.

**P3 — Operator human (secondary).** Scripted shell workflows. Uses jirali in `bash`/`zsh` pipes with `jq`, `grep`, `awk`. Needs predictable piped output and non-zero exits on failure.

**P4 — Platform/admin team (tertiary).** Runs jirali in CI/CD and server environments. Needs headless auth, correlation-ID-based audit logs, configurable rate limits, and deterministic behavior across versions.

### 4.2 Representative user stories

**US-001 (P1).** As an autonomous coding agent, when given a PR to review, I want to fetch the linked Jira ticket's acceptance criteria and recent comments in a single command so that my context window is not consumed by raw API responses.

**US-002 (P1).** As an autonomous agent tasked with triaging new bugs, I want to list all high-priority issues created in the last 24 hours with a projection limited to key/summary/reporter/severity so that my downstream reasoning operates on ≤200 tokens per issue.

**US-003 (P1).** As an autonomous agent attempting a workflow transition that fails due to a missing validator field, I want structured stderr output telling me exactly which fields are required so that I can retry with correct parameters without human intervention.

**US-004 (P1).** As an autonomous agent, I want to wait for a human to approve a Jira ticket before proceeding with deployment so that I do not continuously poll the API.

**US-005 (P2).** As a developer, I want to start a branch named from a Jira ticket and have the ticket automatically transition to "In Progress" so that I do not have to open the web UI.

**US-006 (P2).** As a developer reviewing a sprint, I want a readable table of current-sprint issues with color-coded priority and ANSI-bold status so that I can triage at a glance.

**US-007 (P3).** As an operator, I want to pipe the output of `jirali issue list --jql "..."` into `jq` and then into `xargs jirali issue edit` to bulk-update labels without writing a Python script.

**US-008 (P4).** As a platform admin, I want every jirali invocation to emit an audit record locally so that I can distinguish agent vs. human activity during compliance review.

**US-009 (P1).** As an agent performing a sprint retrospective, I want a JSON time-series of daily remaining-effort for the sprint so that I can summarize velocity trends without fetching raw changelog entries.

**US-010 (P4).** As a platform admin supporting a Data Center deployment, I want jirali to work transparently against our on-prem instance using a Personal Access Token so that we are not excluded from tooling built for Cloud.

---

## 5. Success Metrics

### 5.1 Adoption and usage

| Metric | Target (90 days post-MVP) | Target (6 months post-v1.0) |
|---|---|---|
| Weekly active installations | 50 internal | 250 internal / 2,500 external |
| Subcommand invocations per day (aggregate) | 5,000 | 100,000 |
| Fraction of invocations in agent mode (non-TTY) | ≥60% | ≥75% |
| Number of distinct Claude Code skills that wrap jirali | 5 | 20 |

### 5.2 Performance

| Metric | Target |
|---|---|
| Cold-start binary latency (p50) | ≤50 ms |
| Cold-start binary latency (p95) | ≤120 ms |
| Daemon-mode invocation latency (p50) | ≤10 ms |
| Daemon-mode invocation latency (p95) | ≤30 ms |
| Uncached issue view latency (p50) | ≤250 ms |
| Cached issue view latency (p50) | ≤15 ms |
| Bulk transition of 1,000 issues (p50) | ≤30 s |

### 5.3 Reliability

| Metric | Target |
|---|---|
| Command success rate when Jira API is healthy | ≥99.5% |
| Structured stderr on non-zero exit | 100% |
| Correlation ID present in audit log | 100% |
| Cached read availability during Jira outage | ≥95% |

### 5.4 Token efficiency

| Metric | Target |
|---|---|
| Median tokens per operation vs. Rovo MCP | ≤20% |
| Tokens to discover tool schema via `--help` vs. MCP preamble | ≤5% |

### 5.5 Quality

| Metric | Target |
|---|---|
| ADF node type coverage (round-trip from Markdown) | ≥90% by v1.0 |
| Test coverage (unit) | ≥80% |
| Agent-scenario integration test pass rate | ≥98% |
| P0/P1 bugs in 30 days post-v1.0 release | ≤5 |

---

## 6. Requirements

Requirement language follows RFC 2119: **SHALL** / **SHALL NOT** / **SHOULD** / **SHOULD NOT** / **MAY**.

### 6.1 Functional requirements

#### 6.1.1 Authentication and identity (FR-AUTH)

| ID | Requirement |
|---|---|
| FR-AUTH-1 | Jirali SHALL support API token authentication for Atlassian Cloud (email + token, Basic Auth encoding). |
| FR-AUTH-2 | Jirali SHALL support Personal Access Token (Bearer) authentication for Jira Data Center and Server. |
| FR-AUTH-3 | Jirali SHALL support OAuth 2.1 authorization code flow with PKCE for interactive users, including a localhost callback listener. |
| FR-AUTH-4 | Jirali SHALL support OAuth Dynamic Client Registration (DCR) where the upstream endpoint supports it. |
| FR-AUTH-5 | Jirali SHALL store secrets in the OS keychain by default (Keychain.app, libsecret, wincred). |
| FR-AUTH-6 | Jirali SHALL accept secrets from environment variables (`JIRALI_API_TOKEN`, `JIRALI_EMAIL`) and from a restrictive-mode config file as fallbacks. |
| FR-AUTH-7 | Jirali SHALL support mutual TLS client certificates for Data Center deployments. |
| FR-AUTH-8 | Jirali SHALL support named profiles (`--profile`, `JIRALI_PROFILE`) enabling multi-site operation. |
| FR-AUTH-9 | Jirali SHALL refresh OAuth access tokens in the background without user intervention when a refresh token is present. |
| FR-AUTH-10 | Jirali SHALL NOT log secrets or emit them to stdout, stderr, or audit records. |
| FR-AUTH-11 | Jirali SHALL emit a distinct `User-Agent` string that distinguishes agent-mode (`jirali-agent/<version>`) from human-mode (`jirali-human/<version>`). |
| FR-AUTH-12 | Jirali SHALL support a configurable correlation ID header (`X-Jirali-Correlation-Id`) on every outbound request. |

#### 6.1.2 Issue operations (FR-ISSUE)

| ID | Requirement |
|---|---|
| FR-ISSUE-1 | Jirali SHALL support creating, viewing, editing, and deleting issues. |
| FR-ISSUE-2 | Jirali SHALL support projection profiles (`--profile {skinny,triage,dev,full}`) and ad-hoc field projection (`--fields k1,k2,...`). |
| FR-ISSUE-3 | Jirali SHALL support issue cloning with text substitution in summary and description. |
| FR-ISSUE-4 | Jirali SHALL support idempotent ensure semantics (`jirali issue ensure`) that exit with code 5 (Conflict) when the desired state already holds. |
| FR-ISSUE-5 | Jirali SHALL support bulk edit and bulk transition using the Jira v3 bulk operations endpoints, with batches of up to 1,000 issues per request. |
| FR-ISSUE-6 | Jirali SHALL handle workflow transitions with structured stderr feedback identifying missing validator fields when a transition is blocked. |
| FR-ISSUE-7 | Jirali SHALL support diff-over-time (`jirali diff <KEY> --as-of <timestamp>`) using the issue changelog. |
| FR-ISSUE-8 | Jirali SHALL support a `jirali wait` primitive that blocks until a JQL result count, an issue status, or a custom predicate is satisfied, with a required `--timeout`. |

#### 6.1.3 Relationships and hierarchy (FR-LINK)

| ID | Requirement |
|---|---|
| FR-LINK-1 | Jirali SHALL support adding, removing, and listing issue links of any configured link type. |
| FR-LINK-2 | Jirali SHALL emit a link-type enumeration subcommand (`jirali link types`). |
| FR-LINK-3 | Jirali SHALL support emitting an issue relationship graph in DOT, Mermaid, and JSON-graph formats. |
| FR-LINK-4 | Jirali SHALL support hierarchy traversal (`ancestors`, `descendants`, `tree`) aware of classic and team-managed project hierarchy schemes. |
| FR-LINK-5 | Jirali SHALL support safe re-parenting (`jirali issue re-parent`) preserving comment and changelog history. |

#### 6.1.4 Sprint, board, and backlog (FR-SPRINT)

| ID | Requirement |
|---|---|
| FR-SPRINT-1 | Jirali SHALL support listing boards, board columns (with status mapping and WIP limits), and quick filters. |
| FR-SPRINT-2 | Jirali SHALL support listing sprints filtered by state (`future,active,closed`) and by relative reference (`--current`, `--next`, `--prev`). |
| FR-SPRINT-3 | Jirali SHALL support creating, starting, and closing sprints. |
| FR-SPRINT-4 | Jirali SHALL support adding issues to and moving issues between sprints. |
| FR-SPRINT-5 | Jirali SHALL support listing the backlog view distinct from issues-not-in-any-sprint. |
| FR-SPRINT-6 | Jirali SHALL support querying sprint time-series data (remaining effort, scope changes) in a normalized JSON format. |

#### 6.1.5 Release and version (FR-RELEASE)

| ID | Requirement |
|---|---|
| FR-RELEASE-1 | Jirali SHALL support listing, creating, updating, and archiving project versions. |
| FR-RELEASE-2 | Jirali SHALL support listing issues in a version and generating release notes in Markdown. |

#### 6.1.6 Attachments (FR-ATTACH)

| ID | Requirement |
|---|---|
| FR-ATTACH-1 | Jirali SHALL support uploading, downloading, listing, and removing attachments. |
| FR-ATTACH-2 | Jirali SHALL emit SHA-256 content digests for all listed attachments. |
| FR-ATTACH-3 | Jirali SHALL support reading upload content from stdin. |
| FR-ATTACH-4 | Jirali SHALL support attachments up to the Jira-configured maximum (default 10 MB Cloud, configurable DC) with a clear error above the limit. |

#### 6.1.7 Worklogs (FR-WORKLOG)

| ID | Requirement |
|---|---|
| FR-WORKLOG-1 | Jirali SHALL support creating, listing, editing, and deleting worklogs. |
| FR-WORKLOG-2 | Jirali SHALL support pre-aggregated output grouped by assignee, project, component, or temporal bucket (daily, weekly, monthly). |
| FR-WORKLOG-3 | Jirali SHALL accept flexible duration formats (`2h`, `90m`, `1h30m`, `1.5h`). |

#### 6.1.8 History and changelog (FR-HISTORY)

| ID | Requirement |
|---|---|
| FR-HISTORY-1 | Jirali SHALL support emitting the full issue changelog as structured JSON. |
| FR-HISTORY-2 | Jirali SHALL support filtering changelog by field and by time range. |
| FR-HISTORY-3 | Jirali SHALL support an "events since status transition" query. |

#### 6.1.9 Comments (FR-COMMENT)

| ID | Requirement |
|---|---|
| FR-COMMENT-1 | Jirali SHALL support listing, adding, editing, and removing comments. |
| FR-COMMENT-2 | Jirali SHALL distinguish internal (JSM) from public comments. |
| FR-COMMENT-3 | Jirali SHALL support visibility restrictions (role or group). |
| FR-COMMENT-4 | Jirali SHALL accept comment bodies as Markdown (converted internally to ADF) and as raw ADF via `--body-adf`. |
| FR-COMMENT-5 | Jirali SHALL support extracting @mentions from comments as a structured list. |

#### 6.1.10 JQL, filters, and search (FR-JQL)

| ID | Requirement |
|---|---|
| FR-JQL-1 | Jirali SHALL support arbitrary JQL search with cursor-based pagination on Cloud v3 and offset pagination on v2/DC. |
| FR-JQL-2 | Jirali SHALL provide a pre-flight JQL linter (`jirali jql lint`) warning on known anti-patterns (unnecessary negations, top-level `AND`, unindexed field usage). |
| FR-JQL-3 | Jirali SHALL provide a JQL explainer (`jirali jql explain`) emitting a natural-language translation. |
| FR-JQL-4 | Jirali SHALL detect Jira-side `SearchException` responses and re-emit them as exit code 1 with structured stderr including remediation suggestions. |
| FR-JQL-5 | Jirali SHALL enforce a configurable maximum result set size (default 10,000) and clearly indicate truncation in the `_meta` envelope. |
| FR-JQL-6 | Jirali SHALL support listing, creating, updating, and deleting saved filters. |
| FR-JQL-7 | Jirali SHALL support named JQL templates in the config file, referenceable by key. |

#### 6.1.11 Custom fields and aliases (FR-FIELD)

| ID | Requirement |
|---|---|
| FR-FIELD-1 | Jirali SHALL maintain a per-site field alias map mapping `customfield_NNNNN` IDs to user-friendly aliases and type metadata. |
| FR-FIELD-2 | Jirali SHALL support automatic alias generation (`jirali alias refresh`) from Jira custom field metadata. |
| FR-FIELD-3 | Jirali SHALL accept either aliased (`--story-points 5`) or native (`--customfield_10042 5`) forms on any command. |
| FR-FIELD-4 | Jirali SHALL emit both alias and native identifiers in JSON output to preserve round-trip fidelity. |

#### 6.1.12 Users, groups, and teams (FR-USER)

| ID | Requirement |
|---|---|
| FR-USER-1 | Jirali SHALL support `whoami`, user lookup by email or display name, and reverse lookup from `accountId`. |
| FR-USER-2 | Jirali SHALL transparently resolve email/name queries to `accountId` and cache the mapping. |
| FR-USER-3 | Jirali SHALL NOT expose deprecated `username` or `userKey` fields. |
| FR-USER-4 | Jirali SHALL support group membership listing and mutation (where permissions allow). |
| FR-USER-5 | Jirali SHALL support listing Atlassian cross-product Teams and team membership. |

#### 6.1.13 Jira Service Management (FR-JSM)

| ID | Requirement |
|---|---|
| FR-JSM-1 | Jirali SHALL support listing service desks, request types, and queues. |
| FR-JSM-2 | Jirali SHALL support creating, listing, and updating JSM requests. |
| FR-JSM-3 | Jirali SHALL support listing and resolving SLA conditions on a request, including at-risk queries (`--at-risk --within 1h`). |
| FR-JSM-4 | Jirali SHALL support managing customers, organizations, and request participants. |
| FR-JSM-5 | Jirali SHALL support the JSM Operations surface: alerts (list, acknowledge, close, escalate), on-call schedules, team info, and escalation policies. |
| FR-JSM-6 | Jirali SHALL document that JSM operations calls require API token authentication when invoked through the Rovo MCP bridge. |

#### 6.1.14 Jira Assets (FR-ASSETS)

| ID | Requirement |
|---|---|
| FR-ASSETS-1 | Jirali SHALL support listing Assets schemas and object types. |
| FR-ASSETS-2 | Jirali SHALL support Assets object CRUD and link management. |
| FR-ASSETS-3 | Jirali SHALL support AQL query execution. |
| FR-ASSETS-4 | Jirali SHALL provide AQL linting analogous to JQL linting. |

#### 6.1.15 Automation rules (FR-AUTOMATION)

| ID | Requirement |
|---|---|
| FR-AUTOMATION-1 | Jirali SHALL support listing automation rules in a project and retrieving a rule definition in YAML or JSON. |
| FR-AUTOMATION-2 | Jirali SHALL support triggering a rule manually against a specified issue. |
| FR-AUTOMATION-3 | Jirali SHALL support querying automation execution history. |
| FR-AUTOMATION-4 | Jirali MAY support declarative rule import from YAML. |

#### 6.1.16 Webhooks (FR-WEBHOOK)

| ID | Requirement |
|---|---|
| FR-WEBHOOK-1 | Jirali SHALL support registering, listing, and deregistering webhooks via the Atlassian API. |
| FR-WEBHOOK-2 | Jirali SHALL provide a `listen` mode that temporarily registers a webhook, binds a local HTTP listener, receives a matching event, emits the event payload to stdout, deregisters the webhook, and exits. |
| FR-WEBHOOK-3 | Jirali SHALL support `listen --for-each <command>` invoking a shell template for each event until a timeout or explicit termination. |
| FR-WEBHOOK-4 | Jirali SHALL support webhook payload replay from a local history store. |

#### 6.1.17 Reports (FR-REPORT)

| ID | Requirement |
|---|---|
| FR-REPORT-1 | Jirali SHALL emit velocity, burndown, cumulative flow, cycle time, lead time, throughput, WIP snapshot, aging WIP, and flow efficiency reports as structured JSON time-series. |
| FR-REPORT-2 | Jirali SHALL support configurable bucket boundaries for each report. |
| FR-REPORT-3 | Jirali SHALL compute reports client-side where Jira APIs do not provide them natively, using only documented endpoints. |

#### 6.1.18 ADF handling (FR-ADF)

| ID | Requirement |
|---|---|
| FR-ADF-1 | Jirali SHALL convert Markdown (CommonMark + GitHub-flavored subset) to ADF for all rich-text input fields. |
| FR-ADF-2 | Jirali SHALL convert ADF to Markdown for output, with a lossy-conversion warning on stderr if any node fails to round-trip. |
| FR-ADF-3 | Jirali SHALL accept raw ADF JSON via `--body-adf <string_or_path>` on any rich-text input. |
| FR-ADF-4 | Jirali SHALL provide a standalone ADF utility subtree (`jirali adf from-markdown`, `to-markdown`, `validate`, `normalize`). |
| FR-ADF-5 | Jirali SHALL target ≥90% ADF node-type coverage by v1.0, including paragraph, heading, bulletList, orderedList, taskList, codeBlock, blockquote, rule, panel, expand, table, mediaGroup, inlineCard, mention, emoji, date, and status. |

#### 6.1.19 Declarative operations (FR-DECL)

| ID | Requirement |
|---|---|
| FR-DECL-1 | Jirali SHALL accept a YAML or JSON spec describing desired issue state and support `plan` (dry-run diff) and `apply` (idempotent reconciliation) operations. |
| FR-DECL-2 | Jirali SHALL support a batch operation file with `register_as` cross-references enabling intra-batch identity resolution. |
| FR-DECL-3 | Jirali SHALL provide `--transactional` best-effort rollback on partial failure, with clear documentation of limitations (Jira has no native transactions). |

#### 6.1.20 Cross-product (FR-XPROD)

| ID | Requirement |
|---|---|
| FR-XPROD-1 | Jirali SHALL support adding, listing, and removing Confluence page links from issues. |
| FR-XPROD-2 | Jirali SHALL support Confluence search and thin page creation from a JQL-driven template (e.g., release notes). |
| FR-XPROD-3 | Jirali SHALL support Compass component listing and linking to issues. |
| FR-XPROD-4 | Jirali SHALL support Atlas Goals listing, progress retrieval, and linking to epics and issues. |

#### 6.1.21 Git and CI integration (FR-GIT)

| ID | Requirement |
|---|---|
| FR-GIT-1 | Jirali SHALL support inferring the current issue key from branch name and recent commit history. |
| FR-GIT-2 | Jirali SHALL provide a `jirali branch start <KEY>` command that creates a git branch, optionally transitions the issue, and emits the branch name. |
| FR-GIT-3 | Jirali SHALL support emitting PR/MR description templates seeded from an issue. |

#### 6.1.22 Self-description (FR-META)

| ID | Requirement |
|---|---|
| FR-META-1 | Jirali SHALL emit a skill manifest (`jirali skill emit`) documenting high-leverage subcommands in a format consumable by Claude Code and Cursor skills. |
| FR-META-2 | Jirali SHALL support optional MCP bridge mode (`jirali mcp serve`) exposing a curated subset of subcommands as MCP tools over stdio or HTTP transport. |

#### 6.1.23 Escape hatches (FR-ESCAPE)

| ID | Requirement |
|---|---|
| FR-ESCAPE-1 | Jirali SHALL provide a raw REST passthrough (`jirali api <METHOD> <PATH>`) that inherits auth, retry, and rate-limit handling. |
| FR-ESCAPE-2 | Jirali SHALL provide GraphQL passthrough (`jirali graphql`) for cases requiring cross-product queries. |

### 6.2 Non-functional requirements

#### 6.2.1 Performance (NFR-PERF)

| ID | Requirement |
|---|---|
| NFR-PERF-1 | Cold binary invocation latency SHALL NOT exceed 50 ms p50, 120 ms p95, measured on a 2023 Apple Silicon or equivalent machine. |
| NFR-PERF-2 | Daemon-mode invocation latency SHALL NOT exceed 10 ms p50, 30 ms p95. |
| NFR-PERF-3 | Uncached issue view SHALL complete within 250 ms p50 against healthy Jira Cloud endpoints. |
| NFR-PERF-4 | Cached issue view SHALL complete within 15 ms p50. |
| NFR-PERF-5 | Memory footprint of the client binary SHALL NOT exceed 25 MB RSS. |
| NFR-PERF-6 | Memory footprint of the daemon SHALL NOT exceed 150 MB RSS at idle. |

#### 6.2.2 Reliability (NFR-REL)

| ID | Requirement |
|---|---|
| NFR-REL-1 | Jirali SHALL degrade to stateless operation if the daemon is unreachable. |
| NFR-REL-2 | Jirali SHALL retry transient HTTP failures (connect errors, 5xx, 429) with exponential backoff, respecting `Retry-After` when present. |
| NFR-REL-3 | Jirali SHALL emit correlation IDs for all failures. |
| NFR-REL-4 | Jirali SHALL provide cached-read fallback (`--allow-cached`) during Jira API outages for content fetched within the configured TTL. |

#### 6.2.3 Security (NFR-SEC)

See §9 for detailed design.

| ID | Requirement |
|---|---|
| NFR-SEC-1 | Secrets SHALL be stored in the OS keychain by default. |
| NFR-SEC-2 | Secrets SHALL NOT appear in process arguments (no `--password <pw>` patterns; only env vars or stdin). |
| NFR-SEC-3 | Local audit logs SHALL NOT contain secrets or full request bodies for writes; they SHALL contain correlation IDs, timestamps, subcommand, exit code, and user agent. |
| NFR-SEC-4 | TLS SHALL be required for all remote calls; TLS 1.2 or later. |
| NFR-SEC-5 | Jirali SHALL respect Atlassian IP allowlist enforcement and emit exit code 4 with remediation when an IP is not allowlisted. |
| NFR-SEC-6 | Jirali SHALL support optional PII masking (`--mask-pii`) for exported data, covering emails, display names, and configurable custom field patterns. |

#### 6.2.4 Compliance (NFR-COMP)

| ID | Requirement |
|---|---|
| NFR-COMP-1 | All user identification SHALL use opaque `accountId`, never `username` or `userKey`. |
| NFR-COMP-2 | Jirali SHALL emit GDPR-compliance headers (`x-atlassian-force-account-id: true`) on all REST calls. |
| NFR-COMP-3 | Audit records SHALL be sufficient to support SOC 2 CC7.2 (system monitoring) and CC7.3 (anomaly response) evidence collection. |

#### 6.2.5 Observability (NFR-OBS)

| ID | Requirement |
|---|---|
| NFR-OBS-1 | Jirali SHALL emit structured NDJSON logs to `~/.local/state/jirali/audit.ndjson` by default, with log rotation at configurable size. |
| NFR-OBS-2 | Jirali SHALL support OpenTelemetry trace export via OTLP when `--otel-export` is provided or `OTEL_EXPORTER_OTLP_ENDPOINT` is set. |
| NFR-OBS-3 | Daemon mode SHALL expose a Prometheus metrics endpoint when `--metrics` is provided. |

#### 6.2.6 Usability — human (NFR-HUMAN)

| ID | Requirement |
|---|---|
| NFR-HUMAN-1 | Jirali SHALL detect TTY on stdout and stderr and adapt output accordingly. |
| NFR-HUMAN-2 | Jirali SHALL provide shell completion scripts for bash, zsh, fish, and PowerShell. |
| NFR-HUMAN-3 | Jirali SHALL support dynamic completion (e.g., project keys, issue types) via subcommand. |
| NFR-HUMAN-4 | Jirali SHALL provide interactive TUI triage for sprint and backlog views when invoked in TTY mode. |

#### 6.2.7 Usability — agent (NFR-AGENT)

| ID | Requirement |
|---|---|
| NFR-AGENT-1 | Jirali SHALL emit only valid JSON to stdout in non-TTY mode or when `--json` is set. |
| NFR-AGENT-2 | Jirali SHALL emit structured JSON error payloads to stderr on non-zero exit. |
| NFR-AGENT-3 | Jirali SHALL NOT prompt for input in non-TTY mode or when `--no-input` is set; it SHALL exit with code 2 when required input is missing. |
| NFR-AGENT-4 | Jirali SHALL suppress progress spinners, color codes, and ASCII borders when not in TTY mode. |
| NFR-AGENT-5 | Jirali SHALL emit a `_meta` envelope (opt-in via `--meta`) containing rate limit state, correlation ID, cache hit status, and endpoint latency. |

#### 6.2.8 Portability (NFR-PORT)

| ID | Requirement |
|---|---|
| NFR-PORT-1 | Jirali SHALL support macOS (x86_64 and arm64), Linux (x86_64 and arm64), and Windows (x86_64). |
| NFR-PORT-2 | Jirali SHALL be distributed as a single static binary per platform. |
| NFR-PORT-3 | Jirali SHALL be distributed via Homebrew, direct binary downloads, and a published Docker image. |

#### 6.2.9 Versioning (NFR-VER)

| ID | Requirement |
|---|---|
| NFR-VER-1 | Jirali SHALL follow semantic versioning. |
| NFR-VER-2 | Breaking changes to stdout JSON schema or exit code semantics SHALL require a major version increment. |
| NFR-VER-3 | Jirali SHALL support a `--schema-version` flag pinning output to a prior major schema for 12 months post-deprecation. |

---

## 7. Architecture

### 7.1 System context

```
+-------------------------------------------------------------------+
|                       Consumer processes                          |
|                                                                   |
|  [Claude Code agent]  [Cursor agent]  [CI job]  [human terminal]  |
|           \               |              |              /         |
|            \              |              |             /          |
|             v             v              v            v           |
+---------------------------+---------------------------------------+
                            |
                            | exec (stateless path)
                            | UDS (daemon path)
                            v
+-------------------------------------------------------------------+
|                      jirali client binary                         |
|                                                                   |
|   [arg parser] -> [router] -> [command handler]                   |
|                                  |                                |
|                                  +-- [auth]                       |
|                                  +-- [http client pool]           |
|                                  +-- [adf pipeline]               |
|                                  +-- [jql/aql pipeline]           |
|                                  +-- [output formatter]           |
|                                  +-- [audit writer]               |
+----+-----------------------+----+---------------------------------+
     |                       |
     | optional UDS          | optional FFI/IPC
     v                       v
+------------+          +----------+
| jiralid    |          | embedd   |
| (daemon)   |          | (embeds) |
|            |          +----------+
| - auth     |
| - cache DB |          +----------+
| - metadata |          | vaultli  |
| - pool     |          | (KB)     |
+------+-----+          +----------+
       |
       | HTTPS
       v
+-------------------------------------------------------------------+
|  Atlassian Cloud / Data Center                                    |
|   [REST v2/v3] [GraphQL] [Rovo MCP] [Assets AQL] [JSM Ops]        |
+-------------------------------------------------------------------+
```

### 7.2 Process models

Jirali supports two execution modes; both use the same binary.

**Stateless mode.** The `jirali` binary is invoked per command, executes, emits output, and exits. No background state. This is the default and the only guaranteed-available mode.

**Daemon mode.** A `jiralid` process (same binary, different entry point) runs in the background and listens on a Unix domain socket at `$XDG_RUNTIME_DIR/jirali.sock` (or `/tmp/jirali.sock` as fallback). The client binary detects the socket, forwards the command, and streams results back. If the socket is unreachable, the client falls back to stateless execution transparently. The daemon is opt-in (`jiralid start`).

### 7.3 Component breakdown

#### 7.3.1 Argument parser and router

Parses subcommands and flags. Emits usage errors as exit code 2 with structured stderr. Handles global flags (`--profile`, `--json`, `--no-input`, `--meta`, `--allow-cached`, `--profile`, `--schema-version`, `--otel-export`). Dispatches to a command handler.

#### 7.3.2 Auth subsystem

Resolves the effective credential from, in order: command-line flag, environment variable, OS keychain, config file. Supports API token, PAT, OAuth 2.1, and mTLS. Handles OAuth token refresh and local callback listener for initial authorization. Emits `X-Jirali-Correlation-Id` and a distinguished `User-Agent`.

#### 7.3.3 HTTP client pool

Single underlying HTTP/2-capable client per process, with connection reuse, keep-alive, and per-host connection caps. In daemon mode, the pool persists across invocations. Retry policy: exponential backoff with jitter on connect errors, 5xx, and 429 (honoring `Retry-After`). Circuit breaker at configurable failure rate to avoid hammering a downed Jira.

#### 7.3.4 ADF pipeline

Two components: `md2adf` (CommonMark AST → ADF JSON) and `adf2md` (ADF JSON → CommonMark). Both implemented over an intermediate IR so that node additions apply bidirectionally. Validation component (`adf-validator`) confirms ADF JSON against Atlassian's schema before submission.

#### 7.3.5 JQL / AQL pipeline

A minimal JQL parser capable of syntactic and semantic linting: identifies top-level `AND`, negation on indexed fields, unbounded `ORDER BY`, and deprecated field references. Does not fully evaluate JQL (that remains server-side). AQL pipeline is a sibling with Assets-specific schema awareness.

#### 7.3.6 Output formatter

Two modes: `TerminalFormatter` (ANSI, tables, color, progress) and `JsonFormatter` (schema-stable JSON envelope). Selected by TTY detection and flag overrides. Envelope structure is specified in §7.5.

#### 7.3.7 Audit writer

Appends NDJSON records to `~/.local/state/jirali/audit.ndjson` with rotation. Each record includes timestamp, correlation ID, subcommand, profile, auth method, exit code, duration, user agent, and cache hit indicators. No request bodies, no response bodies, no secrets.

#### 7.3.8 Daemon (optional)

Single-process multi-threaded server listening on UDS. Hosts the HTTP client pool, auth token cache, metadata cache (custom fields, users, projects, workflows), optional SQLite local cache, and optional metrics endpoint. Auto-starts HTTP connection health checks. Graceful shutdown on SIGTERM.

#### 7.3.9 Local cache (optional)

SQLite database at `~/.local/state/jirali/cache.db`. Schema in §7.6. FTS5 index on issue summaries and descriptions for local text search. Per-row TTL; background sweep on daemon startup removes expired entries. Disabled by default in stateless mode; enabled automatically in daemon mode.

#### 7.3.10 Semantic search integration (optional)

When `embedd` daemon is available and `--semantic` is set, issue content is passed to `embedd` for embedding and indexed into a sibling vector store (sqlite-vec). Query path: embed the query, retrieve top-k by cosine similarity, hydrate from the local cache. No vectors leave the host machine.

#### 7.3.11 MCP bridge (optional)

`jirali mcp serve` starts an MCP server exposing a curated subset of subcommands as MCP tools. Supports stdio and streamable HTTP transports. Translates MCP tool calls into internal subcommand invocations. Not the primary interface; enables jirali consumption from MCP-only clients.

### 7.4 Exit code taxonomy

Extends the FD's taxonomy. All codes are documented in `jirali help exit-codes`.

| Code | Classification | Semantic meaning | Agent response |
|---|---|---|---|
| 0 | Success | Command completed, stdout contains data | Parse and proceed |
| 1 | General failure | Transient error, network, 5xx, unexpected response shape | Retry with backoff |
| 2 | Usage error | Invalid arguments, unknown flags, required input missing | Read stderr, reconstruct args |
| 3 | Not found | Target resource does not exist | Update assumptions, search |
| 4 | Permission denied | Auth insufficient, IP not allowlisted, scope missing | Halt, escalate to human |
| 5 | Conflict / idempotent | Resource exists, desired state already holds | Treat as success |
| 6 | Rate limited | 429 received after retry budget exhausted | Back off longer, retry later |
| 7 | Validation failed | Jira-side validation rejected input (e.g., field constraints) | Parse stderr, adjust input |
| 8 | Timeout | Operation exceeded client or server timeout | Retry with longer timeout or paginate |

Codes 6–8 extend the FD's original 0–5 to cover cases the FD discussed but did not enumerate.

### 7.5 Output envelope specification

#### 7.5.1 stdout envelope (success)

Single-object form (`jirali issue view`):

```json
{
  "data": { ... the entity ... },
  "_schema": "jirali.issue.v1",
  "_meta": {
    "correlation_id": "01HQG8...",
    "rate_limit_remaining": 147,
    "rate_limit_reset_in_s": 42,
    "cache_hit": false,
    "endpoint_latency_ms": 312,
    "truncated": false
  }
}
```

Collection form (`jirali issue list`):

```json
{
  "data": [ ... entities ... ],
  "_schema": "jirali.issue.v1",
  "_meta": {
    "correlation_id": "...",
    "next_page_token": "opaque-cursor",
    "page_size": 50,
    "truncated": false,
    ...
  }
}
```

The `_meta` envelope is emitted only when `--meta` is specified or in daemon mode. By default, output is unwrapped (just `data`) to minimize agent token cost. Agents that need meta information opt in.

#### 7.5.2 stderr envelope (failure)

```json
{
  "error": true,
  "code": "PERMISSION_DENIED",
  "exit_code": 4,
  "message": "The authenticated token does not have permission to edit issue ENG-123.",
  "suggestion": "Verify the user has the 'Edit Issues' permission for project ENG, or authenticate as a user with that permission.",
  "context": {
    "issue_key": "ENG-123",
    "required_permission": "EDIT_ISSUES"
  },
  "correlation_id": "01HQG8...",
  "documentation_url": "https://jirali.dev/errors/PERMISSION_DENIED"
}
```

Error codes are enumerated strings (e.g., `PERMISSION_DENIED`, `NOT_FOUND`, `USAGE_ERROR`, `CONFLICT`, `RATE_LIMITED`, `VALIDATION_FAILED`, `TIMEOUT`, `JQL_SEARCH_EXCEPTION`).

### 7.6 Local cache schema

```sql
CREATE TABLE issues (
    key           TEXT PRIMARY KEY,
    site_id       TEXT NOT NULL,
    project_key   TEXT NOT NULL,
    issuetype     TEXT NOT NULL,
    status        TEXT,
    summary       TEXT,
    updated       TEXT NOT NULL,  -- ISO 8601
    payload_json  TEXT NOT NULL,  -- full issue payload
    expires_at    TEXT NOT NULL,
    fetched_at    TEXT NOT NULL
);

CREATE VIRTUAL TABLE issues_fts USING fts5(
    key UNINDEXED,
    summary,
    description,
    comments,
    content='',
    tokenize='porter'
);

CREATE TABLE custom_fields (
    site_id       TEXT NOT NULL,
    field_id      TEXT NOT NULL,   -- e.g., customfield_10042
    alias         TEXT,
    display_name  TEXT,
    field_type    TEXT,
    schema_json   TEXT,
    project_scope TEXT,            -- JSON array of project keys or 'global'
    updated       TEXT NOT NULL,
    PRIMARY KEY (site_id, field_id)
);

CREATE TABLE users (
    account_id    TEXT PRIMARY KEY,
    site_id       TEXT NOT NULL,
    display_name  TEXT,
    email_hash    TEXT,           -- SHA-256 of email, not the email itself
    active        INTEGER,
    updated       TEXT NOT NULL
);

CREATE TABLE projects (
    key           TEXT NOT NULL,
    site_id       TEXT NOT NULL,
    name          TEXT,
    project_type  TEXT,            -- 'classic' | 'team-managed'
    schemes_json  TEXT,
    updated       TEXT NOT NULL,
    PRIMARY KEY (site_id, key)
);

CREATE TABLE workflows (
    id            TEXT NOT NULL,
    site_id       TEXT NOT NULL,
    project_key   TEXT NOT NULL,
    issuetype     TEXT NOT NULL,
    transitions_json TEXT NOT NULL,
    updated       TEXT NOT NULL,
    PRIMARY KEY (site_id, id, project_key, issuetype)
);

CREATE TABLE audit (
    correlation_id TEXT PRIMARY KEY,
    timestamp     TEXT NOT NULL,
    subcommand    TEXT NOT NULL,
    profile       TEXT,
    exit_code     INTEGER NOT NULL,
    duration_ms   INTEGER,
    user_agent    TEXT,
    cache_hit     INTEGER,
    error_code    TEXT
);
```

Vector index (when `embedd` is integrated) lives in a sibling table managed by `sqlite-vec`:

```sql
CREATE VIRTUAL TABLE issue_embeddings USING vec0(
    key TEXT PRIMARY KEY,
    embedding float[768]  -- Nomic ModernBERT dimension
);
```

### 7.7 Configuration

Config file location (XDG Base Directory Spec): `$XDG_CONFIG_HOME/jirali/config.toml` or `~/.config/jirali/config.toml`.

```toml
[default]
profile = "prod"

[profile.prod]
site_url = "https://vanguard.atlassian.net"
email = "brian.weisberg@vanguard.com"
auth_method = "keychain"       # keychain | env | oauth | mtls
default_project = "ENG"

[profile.prod.cache]
enabled = true
ttl_seconds = 900

[profile.prod.daemon]
enabled = true
socket = "auto"                # auto | /custom/path

[profile.sandbox]
site_url = "https://sandbox.atlassian.net"
email = "..."
auth_method = "env"

[aliases.customfield_10042]
alias = "story_points"
type = "number"

[jql.my_open_highs]
query = "assignee = currentUser() AND priority = High AND status != Done"

[output]
default_profile = "skinny"
color = "auto"                 # auto | always | never
```

Profile resolution order: `--profile <name>` > `JIRALI_PROFILE` > `[default].profile` > first profile defined.

### 7.8 Data flow examples

#### 7.8.1 Simple issue view, stateless, cold

```
[agent] -> exec(jirali issue view ENG-123 --profile skinny)
  [parser] -> valid, route to 'issue.view'
  [auth]   -> resolve credential from keychain, mint Basic Auth header
  [http]   -> GET /rest/api/3/issue/ENG-123?fields=summary,status,assignee
            -> 200 OK, 187ms
  [formatter] -> skinny projection applied, JSON envelope
  [audit]  -> append NDJSON record with correlation_id, exit_code=0
  [stdout] <- {"data":{"key":"ENG-123","summary":"...","status":"In Progress","assignee":{"accountId":"..."}}}
  [exit] 0
```

#### 7.8.2 Issue view, daemon, warm cache

```
[agent] -> exec(jirali issue view ENG-123)
  [client]  -> UDS connect to jiralid
  [daemon]  -> dispatch to 'issue.view'
              -> cache lookup 'ENG-123': HIT, fetched_at=3s ago, within TTL
              -> return cached payload
  [client]  <- stream JSON envelope
  [stdout]  <- data
  [exit] 0   (total elapsed: 8ms)
```

#### 7.8.3 Blocked transition with validator feedback

```
[agent] -> exec(jirali issue transition ENG-123 "Code Review")
  [auth]    -> resolve
  [http]    -> GET /rest/api/3/issue/ENG-123/transitions -> list
  [http]    -> POST /rest/api/3/issue/ENG-123/transitions -> 400
              {"errorMessages":[],"errors":{"customfield_10050":"Root Cause is required"}}
  [formatter] -> translate to structured stderr:
              {
                "error":true,
                "code":"VALIDATION_FAILED",
                "exit_code":7,
                "message":"Transition to 'Code Review' blocked by validator.",
                "suggestion":"Provide 'root_cause' (customfield_10050).",
                "context":{"issue_key":"ENG-123","target_status":"Code Review",
                           "missing_fields":[{"id":"customfield_10050","alias":"root_cause","type":"string"}]}
              }
  [audit]   -> append record with error_code=VALIDATION_FAILED
  [exit] 7
```

---

## 8. Detailed Design

### 8.1 Command taxonomy

Top-level subcommand groups and their charters:

| Group | Charter |
|---|---|
| `auth` | Login, logout, whoami, profile list, token rotate |
| `issue` | Core CRUD + projections + ensure + diff + wait |
| `link` | Link types, add, remove, list, graph |
| `hierarchy` | Ancestor/descendant/tree navigation, re-parent |
| `sprint` | Sprint list, create, start, close, add/move issue |
| `board` | Board list, columns, quick filters, backlog |
| `release` | Version list, create, update, issues, notes |
| `attach` | Attachment upload, download, list, remove |
| `worklog` | Worklog CRUD + aggregate |
| `comment` | Comment CRUD + mentions |
| `history` | Changelog queries |
| `user` | Whoami, find, workload, groups, teams |
| `project` | Project list, get, schemes, roles |
| `customfield` | Field list, context, aliases |
| `workflow` | Workflow list, transitions, validate |
| `jql` | Search, lint, explain, filter CRUD |
| `filter` | Saved filter CRUD |
| `jsm` | Desk, request-type, request, sla, queue, customer, org, kb, ops |
| `assets` | Schema, object, AQL |
| `automation` | Rule list/get/trigger/audit/import |
| `webhook` | Register, list, listen, replay |
| `report` | Velocity, burndown, CFD, cycle, lead, throughput, wip, aging, flow |
| `wiki` | Confluence link, search, thin create |
| `compass` | Component list, metric, link |
| `goal` | Atlas goals list/progress/link |
| `adf` | Markdown ↔ ADF utilities |
| `plan` | Dry-run diff of declarative spec |
| `apply` | Reconcile declarative spec |
| `batch` | Multi-op transactional batch |
| `branch` | Git branch start / link |
| `api` | Raw REST passthrough |
| `graphql` | Raw GraphQL passthrough |
| `mcp` | MCP bridge serve |
| `skill` | Emit skill manifest |
| `alias` | Custom field alias management |
| `local` | Local cache search, embed, nearest |
| `audit` | Query local audit log |
| `context` | Infer current ticket from cwd/branch |
| `diff` | Issue diff over time |
| `snapshot` | Create/diff point-in-time snapshots |
| `config` | Print effective config, validate |
| `mask` | PII masking utility for piped payloads |

Naming convention: `jirali <group> <verb>` (e.g., `jirali issue create`). Single-word top-level commands (`jirali apply`, `jirali plan`, `jirali batch`) are reserved for operations that conceptually span groups.

### 8.2 Agent vs human mode

Mode detection order:

1. `--no-input` forces agent mode regardless of TTY.
2. `--json` forces JSON output but does not suppress interactive prompts if TTY detected.
3. `isatty(stdout)` and `isatty(stdin)` both true → human mode.
4. Otherwise → agent mode.

Human mode affordances:
- ANSI colors (respecting `NO_COLOR` env var per convention)
- Readline prompts for missing required input
- Progress spinners for long operations (≥500ms)
- Tabular output for collections
- Pager (`less -R`) for long output when `JIRALI_PAGER` or `PAGER` is set and output exceeds terminal height

Agent mode constraints:
- All output to stdout is valid JSON (empty result set is `{"data":[]}`, not empty string)
- No ANSI escape codes on stdout
- No prompts; missing required input → exit 2 with structured stderr
- No spinners or progress indicators
- Operations exceeding 60s emit periodic keep-alive comments to stderr (configurable)

### 8.3 Authentication flows

#### 8.3.1 API token / PAT

Direct. Email (Cloud) or username (DC) + token combined into Basic Auth header. For DC PATs, `Authorization: Bearer <token>`. Stored in keychain keyed by `jirali://{profile_name}`.

#### 8.3.2 OAuth 2.1 with PKCE

1. `jirali auth login --profile prod --method oauth`
2. Jirali binds a local HTTP server on a random high port.
3. Jirali computes a PKCE code challenge and opens a browser to Atlassian's authorize endpoint with `response_type=code`, `redirect_uri=http://localhost:<port>/callback`, `scope=<requested>`, `state=<random>`.
4. Browser completes the flow; the local listener receives the authorization code.
5. Jirali exchanges the code + verifier for access + refresh tokens.
6. Tokens stored in keychain; refresh token used on subsequent invocations.

#### 8.3.3 OAuth with DCR

Where supported, the initial call registers jirali as an OAuth client dynamically and caches the resulting `client_id` / `client_secret`. Eliminates manual OAuth app pre-registration.

#### 8.3.4 mTLS

Client cert and key paths specified in profile config. Key optionally password-protected; password from env or keychain. Useful for Data Center deployments behind reverse proxies that require client cert.

### 8.4 ADF pipeline

Markdown → ADF conversion proceeds via a shared CommonMark AST (pulldown-cmark or equivalent for Rust), with a visitor that emits ADF nodes. Tables, task lists, and mentions are handled via extension points. Round-trip testing is governed by a test corpus of Markdown fixtures → ADF → Markdown; losses are tracked and reported.

ADF → Markdown handles nodes not representable in CommonMark (panel, expand, mediaGroup, inlineCard) through a documented extended syntax:

- `panel` → `> :information_source: **Info** …` (GitHub-style admonition)
- `expand` → `<details><summary>Title</summary>…</details>`
- `mediaGroup` → inline image references with title attribute
- `inlineCard` → bare URL with a rendering hint comment

Node types targeted by v1.0 (≥90%): `doc`, `paragraph`, `heading`, `bulletList`, `orderedList`, `listItem`, `taskList`, `taskItem`, `blockquote`, `codeBlock`, `rule`, `panel`, `expand`, `nestedExpand`, `table`, `tableRow`, `tableCell`, `tableHeader`, `mediaSingle`, `mediaGroup`, `text`, `hardBreak`, `emoji`, `mention`, `date`, `status`, `inlineCard`, `blockCard`.

### 8.5 JQL pipeline

Parser: hand-written recursive-descent over a grammar close to the ANTLR grammar Atlassian publishes. Emits an AST. Linter walks the AST, applying rules:

- `Rule.NegationOnField(field=indexed)` → warn
- `Rule.TopLevelAnd(ratio > 0.5)` → warn
- `Rule.DeprecatedFunctionUsage` → warn (e.g., `membersOf` vs. `issuesForGroup`)
- `Rule.UnboundedOrderBy` → warn if no `LIMIT`-equivalent via pagination cap
- `Rule.UnknownField` → error
- `Rule.UnknownOperatorForField` → error
- `Rule.TypeMismatch(expected=number, got=string)` → error

Explainer: second AST walker that emits a natural-language rendering, e.g., `project = ENG AND assignee = currentUser() AND status IN ("In Progress", "Code Review")` → "Issues in project ENG assigned to the current user with status 'In Progress' or 'Code Review'".

Search execution: constructs the correct endpoint depending on site type (Cloud v3 `/rest/api/3/search/jql`, Cloud v2 or DC `/rest/api/2/search`). Cursor-based or offset pagination selected automatically.

### 8.6 Bulk operations

Uses the Jira v3 Bulk Operations endpoints:
- `/rest/api/3/bulk/issues` for create
- `/rest/api/3/bulk/issues/fields` for edit
- `/rest/api/3/bulk/issues/transition` for transition
- `/rest/api/3/bulk/issues/delete` for delete

Batch size: up to 1,000 issues, 200 fields per request. Larger inputs are automatically chunked. Results aggregated into a unified response with per-issue status. On partial failure, exit code 1 with structured stderr enumerating failed keys; successful keys appear in stdout `data` array with status markers.

### 8.7 Workflow transitions with validator feedback

Flow:

1. Fetch available transitions for the target issue.
2. Match by name or ID.
3. Fetch transition screen field requirements if unknown.
4. Submit the transition; if rejected with validation errors, parse the response.
5. Surface missing/invalid fields as structured stderr with alias resolution.
6. In TTY mode, prompt for missing fields and retry; in agent mode, exit 7 with remediation context.

### 8.8 Plan/apply semantics

Spec format (YAML):

```yaml
version: 1
profile: prod
issues:
  - key: ENG-123
    fields:
      summary: "Deploy Q2 release"
      status: "In Progress"
      assignee: jsmith
      labels: [release, q2]
      parent: ENG-100
    ensure_links:
      - type: blocks
        target: ENG-125
  - project: ENG
    create:
      issuetype: Task
      summary: "Final QA sweep"
      register_as: qa_task
  - key: ENG-125
    ensure_links:
      - type: is_blocked_by
        target: "${qa_task}"
```

`plan` algorithm:
1. Parse spec; resolve `register_as` references.
2. For each item with `key`, fetch current state.
3. Compute diff per field: equal → no-op; different → update; absent → create.
4. Emit JSON diff summary. No side effects.

`apply` algorithm:
1. Execute `plan` internally.
2. Process operations in topological order (creates before links that reference them).
3. For each operation, attempt idempotent execution:
   - Update only if value differs (exit 5 for this op if identical).
   - Create only if `register_as` not yet bound.
   - Link only if not already present.
4. On per-op failure in non-transactional mode: continue, track failures.
5. In `--transactional` mode: on first failure, attempt best-effort rollback of prior operations in this apply. Rollback is documented as best-effort only.
6. Emit summary to stdout, errors to stderr.

### 8.9 Batch format

```yaml
version: 1
operations:
  - op: issue.create
    input:
      project: ENG
      summary: "Bug from customer report"
      issuetype: Bug
    register_as: new_bug

  - op: link.add
    input:
      source: "${new_bug}"
      type: blocks
      target: ENG-200

  - op: comment.add
    input:
      issue: ENG-200
      markdown: "Blocked by ${new_bug}."

  - op: issue.transition
    input:
      key: ENG-200
      status: "Blocked"
      fields:
        root_cause: "Upstream dependency"
```

Execution is sequential by default; `--parallel <N>` enables parallel execution where dependency graph permits.

### 8.10 Webhook listener

Architecture:

1. Client generates a random shared secret.
2. Client starts a local HTTP listener on a configured port (or auto-selects).
3. If `--tunnel` is set, client opens an outbound tunnel (via configured tunnel provider: ngrok, cloudflared) and uses the resulting public URL.
4. Client registers a webhook with Jira targeting the public URL, filtered by event type and (optionally) JQL.
5. Listener receives events; validates shared secret in header.
6. On matching event: emit payload to stdout, (optional) `--for-each <cmd>` invocation, deregister webhook, exit 0.
7. On timeout: deregister, exit 1 (timeout code 8).

In enterprise environments without tunnel access, a companion sidecar (`jirali webhook relay`) can receive events on a publicly accessible host and forward them to the local listener over an authenticated channel. Out of scope for MVP.

### 8.11 Daemon architecture

- Single binary, started via `jiralid start` or `systemd --user` unit.
- Listens on UDS in `$XDG_RUNTIME_DIR`.
- Per-connection request/response protocol: length-prefixed JSON messages.
- Thread pool for request handling; connection pool for HTTP to Atlassian.
- Background tasks:
  - Token refresh (OAuth)
  - Cache TTL sweep
  - Metrics emission (if enabled)
  - Audit log rotation
- Graceful shutdown: drain in-flight requests, flush audit log, close DB, exit.
- Health probe: client sends `{"op":"ping"}`, expects `{"ok":true}` within 100ms.

### 8.12 Local cache behavior

- All GET operations against Jira populate cache.
- Cache lookup precedes API call when `--allow-cached` is set.
- Cache TTL default 15 minutes for issues, 24 hours for custom field metadata, 7 days for workflows, 1 hour for user directory entries.
- Invalidation: explicit via `jirali local invalidate <key>`; implicit on write operations against the same key.
- Maximum cache size: configurable (default 500MB); LRU eviction.

### 8.13 Semantic search integration

Requires embedd daemon. Workflow:

1. `jirali local embed --jql "..."` iterates results and calls embedd's embedding endpoint for each.
2. Embeddings stored in `issue_embeddings` sqlite-vec virtual table.
3. `jirali local search "natural query" --semantic` embeds the query, runs cosine similarity search, hydrates top-k from `issues` table.
4. `jirali local nearest ENG-123 --k 10` retrieves the pre-computed embedding for ENG-123 and finds neighbors.
5. All vectors and text remain local.

### 8.14 Ecosystem interop

| Tool | Interop |
|---|---|
| tooli | Command-surface naming conventions align |
| vaultli | Cache DB uses slug addressing; issue payloads exportable to vault |
| vizli | Report subcommands emit vizli-compatible time-series JSON |
| sheetcraft | Issue lists and worklog aggregates compatible with sheetcraft schemas |
| agentcli | Daemon IPC reuses agentcli's UDS protocol |
| embedd | Semantic search uses embedd as the embedding service |
| sqlservd | PII masking semantics aligned with sqlservd |
| mdx | Long issue bodies pipe through mdx for token-budget-aware chunking |
| docli | Release notes can pipe to docli for docx generation |
| clipli | ADF utilities match clipli's Excel HTML pipeline idioms for cross-tool composability |

---

## 9. Security

### 9.1 Threat model

| Threat | Surface | Mitigation |
|---|---|---|
| Credential theft via process listing | API tokens passed on command line | No CLI flags accept secrets; env vars, stdin, keychain only |
| Credential theft via disk | Config file or audit log | Config file mode 0600; audit log contains no secrets |
| Token leakage in logs | Debug / verbose output | Redaction layer; secrets never pass through log formatter |
| Token leakage in telemetry | OTel / audit emissions | Redaction applies before emission |
| Accidental write to production | Profile confusion | Explicit profile in all destructive operations; `--confirm` required for bulk deletes above threshold |
| Agent runaway bulk operation | Broad JQL matching many issues | Bulk operations default to `--dry-run`; `--max <N>` required for non-dry; exit 2 if not set above threshold |
| Malicious webhook payload | `jirali webhook listen` | Shared-secret HMAC validation on incoming events; payload size limits |
| Malicious Jira server (MITM) | All outbound HTTPS | TLS 1.2+ required; cert pinning option for high-trust environments |
| Insider misuse | Human with valid auth | Audit log; user-agent differentiation; integration with Atlassian audit logs |
| Stale token reuse | Compromised PAT still active | Token rotation subcommand; keychain access controls |
| IP allowlist bypass | Agent running in cloud environment | Respect Atlassian enforcement; exit 4 with clear message; no bypass attempted |
| SSRF via URL flag | `jirali api`, `jirali graphql` | Restrict target host to configured profile's site URL |

### 9.2 Credential storage design

Preferred storage tier:

1. **OS keychain** (primary): `security` (macOS), `libsecret` (Linux), `wincred` (Windows). Entry key `jirali://{profile_name}/{credential_kind}`.
2. **.netrc** (secondary): traditional machine/login/password, mode 0600.
3. **Environment variables** (tertiary): `JIRALI_API_TOKEN`, `JIRALI_EMAIL`, `JIRALI_OAUTH_REFRESH_TOKEN`.
4. **Config file** (fallback, discouraged): `~/.config/jirali/auth.toml`, mode 0600, warns on use.

CLI flag-based credentials are rejected at parse time with a usage error directing the user to one of the above.

### 9.3 PII handling

GDPR posture:

- All user references emit and consume `accountId`.
- `username`, `userKey`, `key` fields on user objects are stripped before stdout emission.
- `x-atlassian-force-account-id: true` on every REST call.
- Email hashing in cache: emails stored as SHA-256; display used only if already present in Jira response.

Export masking (`--mask-pii`):

- `emails` → `<redacted-email>`
- `display_names` → opaque hash (`user_<8-char-prefix>`)
- `custom_fields.{pattern}` → configurable patterns for sensitive custom fields
- `comment_bodies` → full redaction to `<redacted-comment>`

### 9.4 Audit

NDJSON records per invocation:

```json
{
  "timestamp": "2026-04-24T13:42:19.882Z",
  "correlation_id": "01HQG8KXV8A...",
  "subcommand": "issue.edit",
  "profile": "prod",
  "user_agent": "jirali-agent/0.3.1",
  "auth_method": "api_token",
  "exit_code": 0,
  "duration_ms": 312,
  "cache_hit": false,
  "args_fingerprint": "sha256:a3f2...",
  "error_code": null
}
```

`args_fingerprint` is a SHA-256 of the argument vector with secrets redacted. Original args not stored. Allows deduplication without retention.

### 9.5 Supply chain

- Release binaries signed with cosign.
- SBOM published alongside each release.
- Dependency update policy: weekly automated PRs; high-severity CVE response within 48 hours.
- Release integrity verified before install by Homebrew formula and Docker image digests.

---

## 10. Observability

### 10.1 Logs

Structured NDJSON at `~/.local/state/jirali/audit.ndjson`. Log rotation at 10 MB, 10 rotated files retained. Schema version field in every record.

### 10.2 Metrics (daemon only)

Prometheus exposition at `/metrics` when enabled:

- `jirali_requests_total{subcommand, exit_code, profile}`
- `jirali_request_duration_seconds{subcommand}` (histogram)
- `jirali_http_calls_total{method, host, status_class}`
- `jirali_http_duration_seconds{host, status_class}` (histogram)
- `jirali_cache_operations_total{op, result}`
- `jirali_rate_limit_remaining{site}`
- `jirali_daemon_uptime_seconds`

### 10.3 Traces

OpenTelemetry spans per subcommand invocation with child spans for HTTP calls, cache operations, and daemon RPCs. Exported via OTLP when configured. No payload data in span attributes; only identifiers and timings.

### 10.4 Correlation

Every invocation generates a ULID correlation ID. Propagated as `X-Jirali-Correlation-Id` on HTTP requests. Included in all log records and spans. Queryable via `jirali audit trace <id>`.

---

## 11. Testing Strategy

### 11.1 Unit tests

≥80% line coverage. Per-package. Core logic (JQL parser, ADF pipeline, output formatter, auth resolver) covered to ≥90%.

### 11.2 Integration tests

Against a containerized Jira Cloud sandbox (using Atlassian's test instance programs where available) or a mocked HTTPS server that replays recorded fixtures. Covers happy-path and error-path scenarios per requirement. Runs in CI on every PR.

### 11.3 Agent scenario tests

A suite of end-to-end scenarios simulating agent workflows:

- Triage new bugs (15 steps)
- Sprint planning (8 steps)
- Workflow transition with validator feedback (5 steps)
- Blocked-on-human approval via webhook listen (4 steps)
- Bulk cleanup with partial failure recovery (12 steps)
- Plan/apply reconciliation (6 steps)

Each scenario is a deterministic script using record/replay fixtures. Runs nightly.

### 11.4 Performance tests

Benchmark suite for key latency targets (cold invocation, daemon invocation, bulk operation throughput). CI gates on 20% regression.

### 11.5 Security tests

- Static analysis: `cargo audit`, `cargo-deny`, `clippy` with security lints.
- Secret scanning: `gitleaks` on every commit.
- Dependency vulnerability scanning: weekly.
- Fuzz testing: JQL parser, ADF parser, CLI argument handling.

### 11.6 Compatibility tests

Matrix: Jira Cloud (latest), Jira DC 9.x, 10.x, 11.x. Per-platform: macOS latest, Ubuntu latest, Windows latest. Nightly builds execute the test suite on all matrix points.

---

## 12. Phased Delivery Plan

### 12.1 Phase v0.1 (MVP) — 12 weeks

**Entry criteria.** PRD approved, core team staffed, Jira sandbox access provisioned.

**Scope.**
- Auth subsystem (API token Cloud, PAT DC, OS keychain, profiles)
- Argument parser, router, TTY detection, dual-mode output
- Exit code taxonomy (all 9 codes)
- `issue`: view, list, create, edit, delete, transition, ensure
- `link`: add, remove, list, types
- `sprint`: list (with `--current`, `--next`, `--prev`), add, move
- `comment`: add, list, edit, remove (with Markdown ADF basic)
- `jql`: search with Cloud v3 cursor pagination and v2/DC offset pagination
- `api`: REST passthrough
- `adf`: from-markdown, to-markdown (paragraph, heading, lists, code, table, mention)
- Bulk: transition, edit, delete (v3 bulk endpoints)
- Structured stderr for all error paths
- Audit log (NDJSON)
- Shell completion: bash, zsh
- Distribution: Homebrew, direct binary (macOS, Linux)

**Exit criteria.**
- All FR-MVP requirements pass integration tests
- Performance targets for cold invocation met
- Security review passed
- Documentation complete for MVP surface

### 12.2 Phase v0.2 — 8 weeks

**Scope.**
- Field aliasing (`alias refresh`, alias-aware flags)
- Projection profiles
- Workflow-aware transitions with validator feedback
- JQL linter (`jql lint`)
- `attach`, `worklog`, advanced `comment` (internal/public, visibility, mentions)
- `hierarchy`: ancestors, descendants, tree
- `release`: list, create, update, issues, notes
- Git context detection; `branch start`
- Skill manifest emission
- Windows binary distribution
- Shell completion: fish, PowerShell

**Exit criteria.**
- Agent-scenario tests pass ≥95%
- Field alias usage reduces per-command token count by ≥15% in benchmark

### 12.3 Phase v0.3 — 10 weeks

**Scope.**
- `plan` / `apply` declarative reconciliation
- `batch` multi-operation format with `register_as`
- `ensure` semantics for issues, sprints, links
- Daemon mode (`jiralid`) with UDS
- Local SQLite cache with FTS5
- `webhook listen` with `--for-each`
- `_meta` envelope with rate limit state
- Correlation ID audit and `audit trace`

**Exit criteria.**
- Daemon invocation p50 ≤ 10 ms
- Cache hit rate ≥ 60% in representative agent workload
- Webhook listen reliability ≥ 99% on test fixtures

### 12.4 Phase v0.4 — 8 weeks

**Scope.**
- `report`: velocity, burndown, cfd, cycle-time, lead-time, throughput, wip, aging, flow-efficiency
- vizli, sheetcraft, docli, mdx integration helpers
- `wiki` (Confluence thin surface), `compass`, `goal`
- `jsm`: desks, request types, requests, SLA, queues, customers, orgs
- `jsm ops`: alerts, on-call, schedules
- `automation`: list, get, trigger, audit
- Cursor-based pagination complete across all relevant endpoints

**Exit criteria.**
- Reports reproduce Jira's native values within 1% for identical JQL
- JSM flows exercised by end-to-end incident response scenario
- Cross-product linking tested

### 12.5 Phase v0.5 — 6 weeks

**Scope.**
- embedd-backed semantic search (`local embed`, `local search --semantic`, `local nearest`)
- Duplicate / nearest-neighbor detection in triage workflow
- `snapshot create/diff`, `issue diff --as-of`
- Automation rule YAML export/import
- `assets` (schema, object, AQL, lint)

**Exit criteria.**
- Semantic search recall@10 ≥ 85% on a labeled duplicate dataset
- AQL linter matches JQL linter parity

### 12.6 Phase v1.0 — 4 weeks hardening

**Scope.**
- MCP bridge mode (curated subset)
- ADF coverage ≥ 90% of targeted node types
- Full observability (OTel, Prometheus)
- PII masking (`--mask-pii`)
- `schema-version` pinning
- Stable config schema
- All platforms, all distribution channels
- Comprehensive SKILL.md set for Claude Code and Cursor

**Exit criteria.**
- All NFR targets met
- P0/P1 bug count over prior 30 days ≤ 5
- External beta feedback incorporated
- Release process rehearsed

---

## 13. Risks and Open Questions

### 13.1 Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Atlassian API breaking changes mid-project | Medium | High | Pin to versioned endpoints; subscribe to deprecation announcements; CI matrix includes latest |
| Cloud v3 cursor pagination changes again | Low | Medium | Abstract pagination behind trait; one change site |
| Rovo MCP introduces features that obviate a CLI | Low | Medium | Token economics and pipeline composability arguments remain valid; MCP bridge mode hedges |
| OAuth DCR not supported on DC | High | Low | PAT remains the DC path; not dependent on DCR |
| Webhook listen requires inbound connectivity | Medium | Medium | Tunnel support; relay sidecar deferred to post-v1.0; documented limitation |
| Daemon security posture concerns | Medium | High | UDS with filesystem permissions; no TCP listener by default; optional and off |
| Embeddings model drift affecting recall | Low | Low | Version the embeddings schema; re-embed on model change |
| Scope creep during phases | High | Medium | Phase exit criteria are contractual; additions deferred to subsequent phase |
| Enterprise IP allowlist frustration | High | Medium | Clear exit 4 messaging; documentation on allowlist configuration; corporate-proxy-aware HTTP client |
| ADF coverage incomplete at v1.0 | Medium | Medium | 90% target, clear documentation of unsupported nodes, lossy-conversion warnings |

### 13.2 Open questions

| ID | Question | Owner | Due |
|---|---|---|---|
| OQ-1 | Go vs. Rust for the implementation language? | Engineering lead | Pre-MVP kickoff |
| OQ-2 | Which tunnel provider (if any) for webhook listen in the MVP? | Eng + security | Phase v0.3 |
| OQ-3 | Do we ship a public-facing MCP registry entry, or stay internal? | PM + security | Phase v0.4 |
| OQ-4 | Does v1.0 include Jira Align integration or defer? | PM | Phase v0.5 |
| OQ-5 | Per-profile cache isolation vs. shared cache with site_id scoping? | Eng | Phase v0.3 |
| OQ-6 | Do we need a Windows keychain beyond wincred (e.g., Credential Manager APIs)? | Eng | Phase v0.1 |
| OQ-7 | Should `plan/apply` support cross-site specs, or one site per spec file? | PM + Eng | Phase v0.3 |
| OQ-8 | What is the deprecation policy for the `_meta` envelope schema? | Eng | Phase v0.3 |
| OQ-9 | How do we handle Jira custom fields with dynamic schemas across team-managed projects in the alias system? | Eng | Phase v0.2 |
| OQ-10 | Distribution of the Docker image — official Atlassian-partnered image, or independent? | PM | Phase v0.1 |

### 13.3 Recommended decisions (with rationale)

- **OQ-1:** Rust. Ecosystem consistency with existing internal Rust CLIs (clipli, docli, agentcli), single-binary distribution, low runtime overhead, strong type system for the large interface surface.
- **OQ-2:** Defer formal tunnel support to v0.3; MVP supports only listener-accessible deployments. Phase v0.3 evaluates cloudflared tunnel integration.
- **OQ-5:** Shared cache with `site_id` scoping. Saves disk, simplifies cross-profile lookups, no security concern because entries are scoped by composite key.
- **OQ-7:** One site per spec file. Cross-site coordination adds complexity and is not in the top agent use cases.

---

## 14. Out of Scope

Not addressed by this document and not planned for v1.0:

- Jira web UI features that require interactive JavaScript (e.g., drag-and-drop board configuration)
- Jira Align
- Proprietary Atlassian Marketplace app integrations beyond the common custom-field surface
- Tempo time-tracking deep integration
- Xray test management
- Jira mobile push notifications
- Multi-tenant hosted jirali service
- AI-inference capabilities embedded in the binary (natural language JQL generation is an integration, not a built-in)
- Competing MCP server for non-Atlassian issue trackers
- Jira Data Center administration (user management, license ops, backup restore)
- Jira Cloud migration tooling

---

## Appendix A — Use Case → Subcommand Matrix

| Agent use case | Primary subcommands | Phase introduced |
|---|---|---|
| Triage new bugs | `jql lint`, `issue list --profile triage`, `issue edit --priority --labels`, `local nearest` (for duplicate detection) | v0.2 / v0.5 |
| Sprint planning | `sprint list`, `report velocity`, `board list`, `backlog list`, `sprint create` | v0.1 / v0.4 |
| Stand-up summary | `sprint list --current`, `history --since 1d`, `user workload` | v0.2 |
| Release notes | `release issues`, `release notes --format markdown`, `wiki create --from-jql` | v0.2 / v0.4 |
| Incident response (JSM) | `jsm ops alert list`, `jsm ops oncall`, `jsm ops alert ack`, `jsm sla at-risk` | v0.4 |
| Duplicate detection | `local nearest`, `local search --semantic` | v0.5 |
| Dependency analysis | `link list`, `graph --depth N`, `hierarchy descendants` | v0.1 / v0.2 |
| Bulk cleanup | `issue bulk-transition --jql`, `issue bulk-edit --jql` | v0.1 |
| Audit / compliance | `history`, `audit trace`, `automation audit` | v0.2 / v0.3 |
| Cross-team coordination | `goal list`, `team members`, `compass component list` | v0.4 |
| Blocked-on-approval | `webhook listen` | v0.3 |
| Declarative environment setup | `plan`, `apply`, `batch` | v0.3 |
| Retrospective analysis | `report cycle-time`, `report lead-time`, `report flow-efficiency` | v0.4 |
| PR-to-ticket linking | `branch start`, `context`, `api` | v0.2 |

---

## Appendix B — Missing Primitive Inventory (introduced)

Primitives not found in any existing Jira tool surveyed; jirali introduces these:

1. `jirali diff <KEY> --as-of <timestamp>` — compare issue state against a past time, computed from changelog
2. `jirali undo <correlation_id>` — reverse recent changes best-effort, using changelog history
3. `jirali wait --jql "..." --condition "count = 0"` — block until JQL is empty (CI gate)
4. `jirali wait --issue <KEY> --status Done --timeout 1h` — block until specific state reached
5. `jirali hierarchy tree --project <KEY> --root-type <TYPE>` — project-wide tree with configurable root
6. `jirali jql explain "..."` — natural-language translation of JQL
7. `jirali validate-transition <KEY> "<STATUS>"` — dry-run a transition, report would-pass/would-fail
8. `jirali attachment diff <ATTACH_ID_1> <ATTACH_ID_2>` — binary or text diff of two attachments
9. `jirali jsm sla at-risk --within 1h` — proactive SLA surfacing
10. `jirali dependents --closed --days 30` — recently-closed blocking issues (potential unblocks)
11. `jirali assignee-of <KEY> --history` — ever-assignee list
12. `jirali stuck --jql "..." --in-status "Code Review" --longer-than 7d` — stalled work detection
13. `jirali issue re-parent <KEY> --new-parent <KEY>` — safe epic/parent reassignment preserving history
14. `jirali snapshot create --jql "..." --name <NAME>`, `snapshot diff <A> <B>` — point-in-time comparisons

---

## Appendix C — MVP Command Specifications

Formal specifications for the phase v0.1 command surface. Each includes usage, flags, output schema, and exit codes.

### C.1 `jirali auth login`

**Synopsis.** Establish credentials for a profile.

**Usage.**
```
jirali auth login [--profile <name>] --method {api-token|pat|oauth|mtls}
                  [--site-url <url>] [--email <email>]
```

**Behavior.**
- TTY mode: interactive prompts for missing fields.
- Non-TTY mode: required fields must be supplied via env or flags; exits 2 otherwise.
- On success: credential stored in keychain, `auth.toml` profile entry created/updated.
- On failure: exits 4 (permission denied) or 1 (general failure).

**Exit codes.** 0, 1, 2, 4.

### C.2 `jirali issue view`

**Synopsis.** Retrieve a single issue.

**Usage.**
```
jirali issue view <KEY> [--profile <name>] [--fields <list>]
                        [--view-profile {skinny|triage|dev|full}]
                        [--json] [--meta] [--allow-cached]
```

**Output schema (agent mode).**
```json
{
  "key": "ENG-123",
  "id": "10042",
  "fields": { ... projected fields ... },
  "_schema": "jirali.issue.v1"
}
```

**Exit codes.** 0, 1, 3, 4.

### C.3 `jirali issue list`

**Synopsis.** List issues by JQL.

**Usage.**
```
jirali issue list [--jql "<jql>"] [--project <key>] [--assignee <user>]
                  [--status <name>] [--created-after <date>]
                  [--limit <N>] [--page-token <token>]
                  [--view-profile {skinny|triage|dev|full}]
                  [--json] [--meta]
```

**Output schema.**
```json
{
  "data": [ ... array of issues ... ],
  "_schema": "jirali.issue.v1",
  "_meta": {
    "next_page_token": "...",
    "truncated": false
  }
}
```

**Exit codes.** 0, 1, 2 (bad JQL syntax pre-flight), 4.

### C.4 `jirali issue create`

**Usage.**
```
jirali issue create --project <KEY> --type <TYPE> --summary "..."
                    [--description-md "..." | --description-adf <path>]
                    [--assignee <user>] [--priority <name>]
                    [--labels a,b,c] [--components a,b]
                    [--parent <KEY>] [--sprint <ID>]
                    [--field <alias>=<value> ...]
                    [--json]
```

**Output schema.**
```json
{
  "key": "ENG-124",
  "id": "10100",
  "self": "https://.../rest/api/3/issue/10100"
}
```

**Exit codes.** 0, 1, 2, 4, 7.

### C.5 `jirali issue edit`

**Usage.**
```
jirali issue edit <KEY> [--summary "..."] [--description-md "..."]
                        [--assignee <user>] [--priority <name>]
                        [--add-label <l>] [--remove-label <l>]
                        [--field <alias>=<value> ...]
                        [--json]
```

**Exit codes.** 0, 1, 2, 3, 4, 5, 7.

### C.6 `jirali issue transition`

**Usage.**
```
jirali issue transition <KEY> "<STATUS_OR_TRANSITION_NAME>"
                        [--field <alias>=<value> ...]
                        [--comment-md "..."]
                        [--resolution <name>]
                        [--json]
```

**Exit codes.** 0, 1, 2, 3, 4, 5 (already in target status), 7 (validator blocked).

### C.7 `jirali issue ensure`

**Usage.** Same as `issue edit`, but idempotent.

**Semantics.** If all specified fields already equal target values, exits 5 with empty `{}` stdout. Otherwise performs the minimal set of updates and exits 0 with the changed fields.

### C.8 `jirali link add`

**Usage.**
```
jirali link add <SOURCE_KEY> --type <TYPE_NAME_OR_ID> <TARGET_KEY>
                             [--comment-md "..."]
                             [--json]
```

**Exit codes.** 0, 1, 2, 3, 4, 5 (link already exists).

### C.9 `jirali sprint list`

**Usage.**
```
jirali sprint list [--board <BOARD_ID>] [--project <KEY>]
                   [--state future,active,closed]
                   [--current | --next | --prev]
                   [--json]
```

**Exit codes.** 0, 1, 3, 4.

### C.10 `jirali jql search`

**Usage.**
```
jirali jql search "<JQL>" [--fields <list>] [--limit <N>]
                          [--page-token <token>] [--json]
```

Alias of `issue list --jql`. Canonical.

### C.11 `jirali comment add`

**Usage.**
```
jirali comment add <KEY> [--markdown "..." | --body-adf <path-or-json>]
                         [--visibility <role>:<name>] [--internal]
                         [--json]
```

**Exit codes.** 0, 1, 2, 3, 4.

### C.12 `jirali api`

**Usage.**
```
jirali api <METHOD> <PATH> [--body <json-or-@path>]
                           [--query <key>=<value> ...]
                           [--header <key>: <value> ...]
```

**Behavior.** Resolves path against the profile's site URL. Applies auth, retry, rate-limit handling. Emits response body to stdout, status code on non-2xx to stderr with exit code mapped from HTTP status.

**Exit codes.** 0, 1, 3, 4, 6.

### C.13 Global flags

Applicable to all subcommands:

- `--profile <name>` — select configured profile
- `--json` — force JSON output regardless of TTY
- `--no-input` — fail fast on missing required input
- `--meta` — include `_meta` envelope
- `--allow-cached` — serve from cache if present and within TTL
- `--schema-version <v>` — pin output schema to a prior major version
- `--otel-export <endpoint>` — emit traces to OTLP endpoint
- `--verbose` / `-v` — increase stderr diagnostic verbosity (does not affect stdout)

---

## Appendix D — Comparison with Existing Tools

| Capability | Rovo MCP | mcp-atlassian (community) | ACLI | ankitpokhrel/jira-cli | go-jira | **jirali** |
|---|---|---|---|---|---|---|
| Primary consumer | LLM agent | LLM agent | Human admin | Human developer | Human developer | **LLM agent + human** |
| Cloud support | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Data Center support | ❌ | ✅ | ✅ (limited) | ✅ | ✅ | ✅ |
| Stateless binary | N/A | ❌ (server) | ✅ | ✅ | ✅ | ✅ |
| Token-efficient (no preamble) | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| Strict stdout/stderr segregation | N/A | N/A | ❌ | Partial (`--plain`) | Partial | ✅ |
| Granular exit codes | N/A | N/A | ❌ | ❌ | ❌ | ✅ (9 codes) |
| ADF Markdown round-trip | ❌ | Partial | ❌ | Partial | ❌ | ✅ (≥90% target) |
| JQL linter | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Bulk operations | ✅ | Partial | ✅ | Partial | ❌ | ✅ |
| Webhook listen | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Jira Expressions | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Plan/apply semantics | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Batch DSL | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Field aliasing | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Local cache | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Semantic search | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ (v0.5) |
| JSM ops tools | ✅ (API-token only) | Partial | ❌ | ❌ | ❌ | ✅ |
| Assets / AQL | Partial | Partial | Partial | ❌ | ❌ | ✅ |
| Reporting primitives | ❌ | ❌ | ❌ | Partial | ❌ | ✅ |
| MCP bridge | N/A | N/A | ❌ | ❌ | ❌ | ✅ |
| Correlation IDs + audit | Partial | ❌ | ❌ | ❌ | ❌ | ✅ |

---

## Appendix E — References

1. Foundation Document: *Architecting Jirali: A Dual-Purpose Command Line Interface for AI Agents and Human Operators in the Jira Ecosystem*
2. Atlassian Jira Cloud Platform REST API v3: https://developer.atlassian.com/cloud/jira/platform/rest/v3/intro/
3. Atlassian Document Format: https://developer.atlassian.com/cloud/jira/platform/apis/document/structure/
4. Atlassian Platform GraphQL API: https://developer.atlassian.com/platform/atlassian-graphql-api/graphql/
5. Atlassian Rovo MCP Server: https://support.atlassian.com/atlassian-rovo-mcp-server/
6. Atlassian Rovo MCP Server GA announcement: https://www.atlassian.com/blog/announcements/atlassian-rovo-mcp-ga
7. Atlassian Rovo MCP supported tools reference: https://support.atlassian.com/atlassian-rovo-mcp-server/docs/supported-tools/
8. atlassian/atlassian-mcp-server: https://github.com/atlassian/atlassian-mcp-server
9. sooperset/mcp-atlassian: https://github.com/sooperset/mcp-atlassian
10. ankitpokhrel/jira-cli: https://github.com/ankitpokhrel/jira-cli
11. go-jira/jira: https://github.com/go-jira/jira
12. Jira Bulk Operations API: https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-bulk-operations/
13. Jira Expressions: https://developer.atlassian.com/cloud/jira/software/jira-expressions/
14. Anthropic effective context engineering: https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
15. Model Context Protocol specification: https://modelcontextprotocol.io/
16. RFC 2119: Key words for use in RFCs to Indicate Requirement Levels: https://www.rfc-editor.org/rfc/rfc2119