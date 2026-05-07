# lira

Local-first issue tracking for agents: a Rust CLI for durable YAML tickets under
`~/.lira/`, structured JSON output, required acceptance criteria, atomic task
lists, comments, history, links, and agent claiming.

lira is designed to be the local tracker/control-plane layer for agent work. It
can reference read-only Jira parents, sync peer GitHub Issues, and expose
Symphony-compatible normalized issue projections for an external runner. It does
not require a daemon, database server, network access, or Codex app-server for
local ticket operations.

## Current status

Implemented local MVP foundation:

- Cargo workspace with planned crate layout.
- `lira` binary with PRD-shaped JSON envelopes: `schema_version`, `ok`, and
  `result` or `error`.
- `lira init` with `--dry-run` support.
- `lira project list`, `lira project create`, and `lira project show`.
- Ticket lifecycle: `lira new`, `lira show`, `lira ls`, and `lira mv`.
- Required acceptance criteria and required embedded tasks.
- Completion guard for `lira mv <ID> done`.
- Task operations: list, add, status, done, cancel.
- Ticket comments, history entries, labels, links, claim/release/active/next.
- `lira doctor` / `lira validate` for workspace and ticket validation.
- `LIRA_HOME` override support via `lira-store`.
- JSONL mutation logging under `~/.lira/logs/`.

Planned by the updated v0.5 PRD/dev plan:

- Symphony-compatible local tracker helpers:
  `lira candidates`, `lira issue show`, `lira issue current`, and
  `lira workflow symphony export|validate`.
- Normalized issue projections with blockers, active/terminal status policy,
  and deterministic candidate sorting.
- SQLite/FTS index and search/query/board if filesystem search needs help.
- Read-only Jira bridge.
- GitHub binding, push/pull, three-way sync, and conflict resolution.
- Generated JSON schemas, release packaging, and docs generation.

## Run

```bash
cargo run -p lira-cli -- --json
cargo run -p lira-cli -- init --dry-run --json
cargo run -p lira-cli -- project create ORION "Orion Project" --json
cargo run -p lira-cli -- project list --json
cargo run -p lira-cli -- project show ORION --json
cargo run -p lira-cli -- new "Add local tickets" \
  --project ORION \
  --acceptance-criterion "Tickets are stored as YAML." \
  --task "Implement ticket creation." \
  --json
cargo run -p lira-cli -- task done ORION-1 T1 --json
cargo run -p lira-cli -- mv ORION-1 done --json
cargo run -p lira-cli -- doctor --json
```

## Symphony Boundary

lira can support a Symphony-style runner by serving as the issue tracker:

```bash
lira candidates --project ORION --json
lira claim ORION-42 --agent symphony-runner --json
lira issue current --ids ORION-42 --json
lira comment ORION-42 "Runner posted proof of work." --json
lira mv ORION-42 in-review --json
```

The runner remains responsible for polling cadence, Codex app-server sessions,
workspaces, retries, stall detection, token accounting, and CI/PR shepherding.
