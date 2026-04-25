# Jirali Development Notes

Jirali lives in `tools/jirali` and is implemented as a Rust CLI.

## Architecture Decisions

- Rust was chosen to match the repository's newer CLI tools and support static
  binary distribution.
- The binary entrypoint is intentionally tiny; implementation lives in the
  library crate so command families can be split further without changing the
  process wrapper.
- Deterministic local state lives under `JIRALI_HOME` or the platform data
  directory for fixture/local mode.
- Live Jira access is available through `jirali api`, `jirali graphql`, and the
  configured-profile path for core issue/comment commands.
- Atlassian Cloud profiles store the site root. A pasted browser URL ending in
  `/jira/` is normalized to the root because `/jira/` serves the web UI, not
  REST JSON.
- All non-TTY output is JSON. Non-zero exits emit JSON to stderr.
- Audit records are append-only NDJSON and omit request/response bodies and
  secrets.

## Atlassian Cloud Authentication Findings

Live testing on 2026-04-24 against an Atlassian Cloud site showed:

- The root REST base is `https://<site>.atlassian.net/rest/api/3/...`.
- `https://<site>.atlassian.net/jira/` is a web UI route. Appending
  `/rest/api/...` under `/jira/` returns browser HTML and should be treated as a
  configuration error.
- Atlassian Cloud API tokens authenticate with Basic auth using
  `email:api_token`. Personal API tokens should not be sent as Bearer tokens.
- Atlassian has both classic/direct and scoped API token URL families. Scoped
  tokens may require
  `https://api.atlassian.com/ex/jira/{cloudId}/rest/api/3/...` rather than the
  site-hosted REST URL.
- The Cloud ID can be discovered from
  `https://<site>.atlassian.net/_edge/tenant_info`.
- A symptom set of `/myself` returning `Client must be authenticated to access
  this resource` while project search is empty often means the wrong URL family
  or auth scheme is being used, not necessarily that the token is revoked.

The live smoke test created and read back issue `SCRUM-5` with label `jirali`,
confirming that the direct site URL plus Basic auth path works for that tested
personal API token.

## Test Strategy

Run:

```bash
cd tools/jirali
cargo test
```

The contract and productionization tests verify:

- issue lifecycle JSON output
- idempotent no-op exit code `5`
- transition validator feedback exit code `7`
- ADF conversion for headings, marks, links, tables, and code blocks
- parser-backed JQL diagnostics
- token redaction and external secret storage
- audit record creation
- roadmap command surfaces
- mock Jira HTTP response mapping
- live-profile issue view cache population
- `/jira/` web UI URL normalization for Atlassian Cloud profiles

## Live vs Local Status

Run:

```bash
jirali tools
```

The `implementation_status` field classifies command groups as live-backed,
local/fixture-backed, or schema-stable placeholder surfaces. This keeps agents
from assuming that every command has endpoint-specific live parity.
