---
name: framerli
description: Use framerli to inspect, publish, promote, and manage Framer projects through an agent-safe Rust CLI with a Node bridge to the official framer-api SDK. Use when the user asks for Framer project status, CMS collections/items, deployment promotion, publishing, or Framer API automation.
---

# framerli — Agent Guide

`framerli` is a Rust control-plane CLI for Framer. Rust owns the safe CLI
contract; the Node bridge owns live calls through the official Framer SDK.

## Agent Contract

- Start with read-only commands: `project info`, `status`, `contributors`,
  `introspect`, or `cms ... list`.
- Use mock bridge mode for tests, examples, and CI-like checks.
- Mutations plan by default. Pass `--yes` only when the user clearly asked to
  execute the change.
- For destructive or promotion-style operations, prefer `--dry-run` first.
- Treat stdout JSON as the data channel. Use structured errors for recovery.
- Do not print or store Framer API keys in docs, logs, or config.

## Setup

From `tools/framerli`:

```bash
cargo test
cargo run -- tools
```

Install the Node bridge dependency before live calls:

```bash
cd bridge
npm install
cd ..
```

Use mock mode when credentials are not available:

```bash
FRAMERLI_BRIDGE_MOCK=1 FRAMER_API_KEY=mock \
  cargo run -- --project https://framer.com/projects/mock project info
```

## Configuration Order

Highest precedence first:

1. CLI flags: `--config`, `--profile`, `--project`
2. Environment: `FRAMERLI_CONFIG`, `FRAMERLI_PROFILE`, `FRAMERLI_PROJECT`,
   `FRAMERLI_KEY_SOURCE`, `FRAMER_API_KEY`
3. Local `framerli.yaml`, `framerli.yml`, or `framerli.toml`
4. Global `~/.config/framerli.yaml`

Persist a profile without writing the secret itself:

```bash
export MY_FRAMER_KEY="..."
cargo run -- auth login \
  --profile marketing \
  --project "https://framer.com/projects/Sites--example" \
  --key-env MY_FRAMER_KEY
```

## Safe Default Workflow

1. Confirm configuration:

   ```bash
   framerli auth test
   framerli project info
   framerli status
   ```

2. Inspect the available surface:

   ```bash
   framerli tools
   framerli introspect
   framerli cms collections list
   ```

3. Preview mutation:

   ```bash
   framerli publish --dry-run
   framerli cms items add posts --dry-run --data @item.json
   ```

4. Execute only after explicit user approval:

   ```bash
   framerli --yes publish --promote
   ```

## Implemented Core Operations

| Need | Command |
|---|---|
| Project metadata | `project info` |
| Auth check | `auth test`, `whoami`, `can <method>` |
| Project status | `status`, `contributors` |
| Publish | `publish [--promote]` |
| Promote deployment | `deploy promote <deployment-id>` |
| CMS collections | `cms collections list`, `cms collection show <slug>` |
| CMS fields/items | `cms fields list`, `cms items list|get|add|remove` |
| Capability discovery | `tools`, `introspect` |

Commands outside the bridge's core slice may return structured
`E_NOT_IMPLEMENTED` unless run as `--dry-run`.

## Output Contract

Success:

```json
{
  "ok": true,
  "data": {},
  "meta": {
    "ms": 12,
    "profile": "default",
    "dry_run": false,
    "generated_at": "2026-04-24T10:00:00Z"
  }
}
```

Error:

```json
{
  "ok": false,
  "error": {
    "code": "E_AUTH_MISSING",
    "message": "No Framer API key configured.",
    "hint": "Set FRAMER_API_KEY.",
    "retryable": false
  },
  "meta": {}
}
```

## Error Recovery

- `E_AUTH_MISSING`: set `FRAMER_API_KEY` or configure a profile key source.
- `E_NOT_IMPLEMENTED`: use `--dry-run` for planning or fall back to a supported
  core operation.
- Bridge failures: run in mock mode to separate Rust CLI issues from live SDK or
  credential issues.
- Project not found: verify the `--project` URL and active profile.
