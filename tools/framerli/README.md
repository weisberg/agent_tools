# framerli

Rust control-plane CLI for the Framer Server API, with a Node bridge for the official
`framer-api` SDK.

This milestone makes `framerli` a usable core CLI rather than only a command scaffold:

- Rust owns command parsing, JSON envelopes, dry-run plans, approval gates, profile config,
  audit logging, and stable exit behavior.
- `bridge/framerli-bridge.mjs` owns live Framer calls through the official Node SDK.
- `FRAMERLI_BRIDGE_MOCK=1` exercises the complete Rust-to-Node bridge path without
  Framer credentials.

## Build and Test

```bash
cargo test
cargo run -- tools
```

## Configure

For live Framer calls, install the bridge dependency and provide a project and API key:

```bash
cd bridge
npm install
cd ..

export FRAMER_API_KEY="..."
cargo run -- --project "https://framer.com/projects/Sites--example" project info
```

`framerli` reads configuration from these sources, highest precedence first:

1. CLI flags such as `--config`, `--profile`, and `--project`
2. Environment variables such as `FRAMERLI_CONFIG`, `FRAMERLI_PROFILE`, `FRAMERLI_PROJECT`, `FRAMERLI_KEY_SOURCE`, and `FRAMER_API_KEY`
3. A local `framerli.yaml`, `framerli.yml`, or `framerli.toml` in the current directory or a parent
4. The global config file at `~/.config/framerli.yaml`

Example `~/.config/framerli.yaml`:

```yaml
default_profile: marketing
profile:
  marketing:
    project: https://framer.com/projects/Sites--example
    key_source: env:MY_FRAMER_KEY
```

You can persist a project and an environment-variable key reference without writing the
secret itself to disk:

```bash
export MY_FRAMER_KEY="..."
cargo run -- auth login --profile marketing --project "https://framer.com/projects/Sites--example" --key-env MY_FRAMER_KEY
cargo run -- --profile marketing auth test
```

This writes YAML to `~/.config/framerli.yaml` by default. Use `--config ./framerli.yaml`
or `FRAMERLI_CONFIG=./framerli.yaml` to read and write a different config file. Use
`FRAMERLI_HOME` only for state such as audit logs.

Environment-only usage is also supported:

```bash
export FRAMERLI_PROJECT="https://framer.com/projects/Sites--example"
export FRAMER_API_KEY="..."
cargo run -- project info
```

## Mock Bridge

Mock mode is useful for agents, CI, and local contract checks:

```bash
FRAMERLI_BRIDGE_MOCK=1 FRAMER_API_KEY=mock \
  cargo run -- --project https://framer.com/projects/mock project info

FRAMERLI_BRIDGE_MOCK=1 FRAMER_API_KEY=mock \
  cargo run -- --project https://framer.com/projects/mock --yes publish --promote
```

## Implemented Core Operations

The Node bridge currently implements the v1 core slice:

- `project info`
- `auth test`
- `whoami`
- `can <method>`
- `status`
- `contributors`
- `publish [--promote]`
- `deploy promote <deployment-id>`
- `cms collections list`
- `cms collection show <slug>`
- `cms fields list <collection>`
- `cms items list|get|add|remove`
- `introspect`

The Rust CLI accepts the broader command taxonomy from the specs. Commands outside the
core bridge slice return structured `E_NOT_IMPLEMENTED` from the bridge unless run as
`--dry-run`.

## Safety Defaults

- Reads execute immediately when project/auth are configured.
- Mutations plan by default; pass `--yes` to execute.
- Destructive commands such as `deploy promote`, `cms items remove`, and `apply` require
  either `--dry-run` or `--yes`.
- Mutating commands append audit entries to `state/audit.ndjson` under the config home
  unless `--no-audit` is passed.

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
