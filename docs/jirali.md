# Jirali Development Notes

Jirali lives in `tools/jirali` and is implemented as a Rust CLI.

## Architecture Decisions

- Rust was chosen to match the repository's newer CLI tools and to support
  static binary distribution.
- The implementation uses one binary, `jirali`, with a broad command tree and
  deterministic local state under `JIRALI_HOME` or the platform data directory.
- Live Jira access is available through `jirali api` and `jirali graphql`.
  Higher-level commands currently use the local state engine so agent contract
  tests do not require Atlassian credentials.
- All non-TTY output is JSON. Non-zero exits emit JSON to stderr.
- Audit records are append-only NDJSON and omit request/response bodies and
  secrets.

## Test Strategy

Run:

```bash
cd tools/jirali
cargo test
```

The contract tests verify:

- issue lifecycle JSON output
- idempotent no-op exit code `5`
- transition validator feedback exit code `7`
- ADF conversion
- JQL linting
- token redaction
- audit record creation
- roadmap command surfaces

## Distribution Notes

The crate is ready for normal Rust binary packaging. Release automation should
add platform matrix builds, checksums, signing, Homebrew formula generation, and
Docker image publishing.
