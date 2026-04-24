# notionli

`notionli` is an agent-safe Notion CLI implemented in Rust from the PRD in this
directory. The current implementation covers the MVP 0 command shape:

- JSON envelopes and structured errors with PRD exit codes.
- Integration-token auth via `NOTION_API_KEY`, `--token-cmd`, or macOS Keychain.
- Local profile state under `~/.local/share/notionli`, backed by `cache.sqlite`
  through the system `sqlite3` command.
- Aliases, selected target (`.`), local object resolution, operation receipts,
  audit log, and dry-run-by-default writes.
- Core Notion calls through the system `curl` command.
- MVP commands for `search`, `page`, `block`, `db`, `ds`, `row`, `comment`,
  `user`, `op`, `schema`, and `tools`.

Build and run:

```bash
cargo build
NOTION_API_KEY=secret_... cargo run -- auth whoami
cargo run -- alias set tasks data_source:248104cd477e80afbc30000bd28de8f9
cargo run -- row upsert tasks --key ExternalID=gh:123 --set "Status=In Progress"
```

Writes are dry-run plans unless `--apply` is supplied.
