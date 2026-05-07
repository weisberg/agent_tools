# slackli

`slackli` is a Rust-first CLI + local service foundation for safe, auditable agent interactions with Slack.

## Product direction

`slackli` is designed as a **Slack toolbelt for agents**:

- send/reply/update/react with policy controls and audit logs,
- receive live events via Socket Mode or Events API,
- read thread/channel context just-in-time,
- search workspace context with semantic-first behavior when available.

## Platform choices

- **Slack Web API** for send/read/write actions.
- **Events API** for incoming events.
- **Socket Mode** as the default local/behind-firewall receive path.
- **Real-time Search API (`assistant.search.context`)** as preferred semantic search backend when configured.

## CLI map (scaffolded)

- `auth` (`login`, `logout`, `status`, `rotate`)
- `send`, `reply`, `update`, `delete`, `react`, `upload`
- `listen`, `history`, `thread`, `search`
- `users`, `channels`
- `agent` (`run`, `once`, `test-event`)
- `mcp serve`
- `config`, `approvals`

Current implementation status:

- `status` is implemented.
- All other top-level commands are scaffolded and currently return structured `NOT_IMPLEMENTED` JSON.

## Foundation principles

- JSON-first command output, NDJSON for streams.
- Dry-run support for mutating commands.
- Policy gate before every Slack write.
- Structured tracing and local audit log.
- Narrow interfaces over bulk ingestion.

## Run

```bash
cargo run -- --help
cargo run -- status
cargo run -- send
```

## Roadmap

See `PLAN.md` for phased implementation, architecture, policy model, storage, and MVP sequencing.
