# slackli plan

## North star

Expose Slack to agents through a narrow, typed, rate-limited, policy-controlled interface rather than a raw chat firehose.

## Architecture

- CLI parser layer (`clap`) with global `--format` and `--dry-run`.
- Policy gate for approvals and allow/deny constraints.
- Event router for `app_mention`, `message.im`, and selected event types.
- Core service layer for send, receive, search, history, thread.
- Slack adapters for Web API and Socket Mode.
- Local storage for config, cache metadata, audit events, and approvals.

## Crate/module growth plan

1. Keep `slackli` binary crate as orchestrator.
2. Add `slackli-core` for domain workflows (send, search, policy).
3. Add `slackli-slack` for Slack transport clients and rate-limit/retry logic.
4. Add `slackli-agent` for subprocess protocol and MCP serving.
5. Add `slackli-store` for SQLite cache/audit.
6. Add `slackli-config` for profile/env/keychain config.

## MVP phases

1. Implement `send`, `reply`, `history`, `thread`.
2. Add config and token storage.
3. Add rate-limits/retries (`Retry-After`, per-channel queue).
4. Implement `listen` via Socket Mode.
5. Add `agent once` and `agent run` with approval policies.
6. Add semantic search path with fallback warnings.
7. Add `mcp serve`.
8. Add audit logs and approval management commands.

## Defaults

- output: `json`
- stream output: `ndjson`
- receive mode: Socket Mode
- reply mode: thread
- write safety: dry-run for first agent run
- cache mode: metadata-first (no full message bodies by default)
