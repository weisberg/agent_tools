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
- All non-TTY output is JSON. Non-zero exits emit JSON to stderr.
- Audit records are append-only NDJSON and omit request/response bodies and
  secrets.

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

## Live vs Local Status

Run:

```bash
jirali tools
```

The `implementation_status` field classifies command groups as live-backed,
local/fixture-backed, or schema-stable placeholder surfaces. This keeps agents
from assuming that every command has endpoint-specific live parity.
