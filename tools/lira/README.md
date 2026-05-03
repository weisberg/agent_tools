# lira

Local Jira for agents: a Rust CLI for local-first ticket management with
canonical YAML under `~/.lira/`, structured JSON output, atomic task lists,
comments, history, and agent claiming.

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

Still pending for full PRD v1.0:

- SQLite/FTS index and search/query/board.
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
