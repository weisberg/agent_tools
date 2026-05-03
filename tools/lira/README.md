# lira

Initial scaffold for the `lira` CLI based on the PRD and development plan.

## Current status

Implemented foundation and early lifecycle commands:

- Cargo workspace with planned crate layout.
- `lira` binary with JSON envelope responses.
- `lira init` with `--dry-run` support.
- `lira project list`, `lira project create`, and `lira project show`.
- `LIRA_HOME` override support via `lira-store`.
- Structured error envelope used for `E_PROJECT_NOT_FOUND` in JSON mode.

## Run

```bash
cargo run -p lira-cli -- --json
cargo run -p lira-cli -- init --dry-run --json
cargo run -p lira-cli -- project create ORION "Orion Project" --json
cargo run -p lira-cli -- project list --json
cargo run -p lira-cli -- project show ORION --json
```
