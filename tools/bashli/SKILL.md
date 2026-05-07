---
name: bashli
description: Use bashli to execute structured shell workflows for AI agents using JSON or YAML task specs instead of raw bash strings. Use when the user needs safe command orchestration, variable capture, parallel read-only steps, assertions, output transforms, token-aware truncation, or structured command results.
---

# bashli — Agent Guide

`bashli` is a structured bash execution engine. It replaces fragile raw shell
strings with a single JSON or YAML task spec and returns machine-readable
results.

## Agent Contract

- Use `bashli` when a workflow would otherwise require pipes, redirects,
  conditionals, repeated parsing, or multi-step shell state.
- Keep the outer shell invocation simple: one JSON argument, stdin, or `-f`.
- Prefer built-in transforms over shelling out to `head`, `tail`, `wc`, `jq`,
  `grep`, `sort`, or `uniq`.
- Use assertions for expected state and fail early when assumptions are wrong.
- Use parallel mode only for read-only commands.
- Treat file writes as explicit task operations, not shell redirection.

## Invocation

```bash
bashli '{"steps":["pwd","ls -la"]}'
cat task.json | bashli -
bashli --yaml task.yaml
bashli -f task.json
```

## Task Spec Shape

```json
{
  "description": "Inspect repository",
  "mode": "Sequential",
  "settings": {
    "max_output_chars": 12000
  },
  "let_vars": {
    "root": "."
  },
  "steps": [
    {"run": "pwd", "capture": "cwd"},
    {"run": "rg --files", "capture": "files"}
  ],
  "summary": ["cwd", "files"]
}
```

## Execution Modes

| Mode | Use When |
|---|---|
| `Sequential` | Later steps depend on earlier steps; stop on first failure. |
| `Independent` | Run every step and report all results. |
| `Parallel` | Run read-only steps concurrently. |

## Safe Default Workflow

1. Convert shell intent into a task spec with a clear description.
2. Capture named outputs needed later.
3. Add assertions for required files, expected exit codes, or non-empty output.
4. Set output limits so the result fits agent context.
5. Run `bashli` and parse JSON instead of scraping terminal text.

## Common Recipes

Inspect project shape:

```bash
bashli '{
  "description":"Inspect repo",
  "steps":[
    {"run":"pwd","capture":"cwd"},
    {"run":"rg --files","capture":"files","transform":{"head":100}},
    {"run":"git status --short","capture":"status"}
  ],
  "summary":["cwd","files","status"]
}'
```

Run independent checks:

```bash
bashli '{
  "mode":"Independent",
  "steps":[
    {"run":"cargo test","capture":"cargo"},
    {"run":"uv run pytest","capture":"pytest"}
  ],
  "summary":["cargo","pytest"]
}'
```

## When Not To Use bashli

- Use ordinary shell for a single trivial command.
- Use domain CLIs directly when a purpose-built tool exists (`xli`, `mdli`,
  `vaultli`, etc.).
- Do not use bashli to bypass approval or safety policies in host environments.

## Error Recovery

- Invalid JSON/YAML: simplify the task or use `-f task.json`.
- Missing command: run a discovery step such as `command -v`.
- Output too large: add transform/truncation settings.
- Parallel step fails unpredictably: switch to `Sequential` if there is hidden
  dependency between commands.
