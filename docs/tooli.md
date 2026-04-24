# Tooli Guide

`tooli` is an agent-native Python CLI framework. Use it when one Python function
should become a human-friendly CLI command, a structured JSON tool for agents, a
Python API call, and optionally an MCP server command.

Install with:

```bash
pip install tooli
pip install 'tooli[mcp]'
```

This repo currently pins `tooli==6.6.0`.

## Quick Start

```python
from tooli import Tooli, Annotated, Argument, Option
from tooli.annotations import ReadOnly

app = Tooli(name="my-tool", description="What my tool does", version="1.0.0")

@app.command(annotations=ReadOnly)
def greet(
    name: Annotated[str, Argument(help="Name to greet")],
    loud: Annotated[bool, Option(help="Shout the greeting")] = False,
) -> dict:
    """Greet someone by name."""
    msg = f"Hello, {name}!"
    if loud:
        msg = msg.upper()
    return {"greeting": msg}

if __name__ == "__main__":
    app()
```

## App Configuration

Create one `Tooli` app per CLI surface:

```python
app = Tooli(
    name="my-tool",
    description="My tool description",
    version="1.0.0",
    default_output="auto",
    mcp_transport="stdio",
    triggers=["when you need to ..."],
    anti_triggers=["not for ..."],
    workflows=[{"name": "lifecycle", "steps": ["create", "list", "delete"]}],
    rules=["Always validate before writing"],
    env_vars={"MY_TOKEN": {"description": "API token", "default": ""}},
    security_policy="standard",
    auth_scopes=["read", "write"],
    record=True,
    backend="typer",
)
```

Useful app methods:

| Method | Purpose |
|---|---|
| `app.command(...)` | Register a command decorator |
| `app.call(name, **kwargs)` | Invoke synchronously and return `TooliResult` |
| `app.acall(name, **kwargs)` | Invoke asynchronously |
| `app.stream(name, **kwargs)` | Stream list-returning command results |
| `app.get_tools()` | Return registered `ToolDef` objects |
| `app.resource(...)` | Register an MCP resource |
| `app.prompt(...)` | Register an MCP prompt |
| `app()` | Run the CLI |

## Commands

Register functions before calling `app()`:

```python
@app.command(
    annotations=ReadOnly | Idempotent,
    task_group="Query",
    when_to_use="Search for patterns",
    capabilities=["fs:read"],
    examples=[{"args": ["--pattern", "*.py"], "description": "Find Python files"}],
    error_codes={"E1001": "Pattern is invalid", "E3001": "No files found"},
    output_example=[{"line": 42, "text": "match"}],
    paginated=True,
    supports_dry_run=False,
    requires_approval=False,
    timeout=None,
    max_tokens=None,
    deprecated=False,
)
def my_command(
    path: Annotated[str, Argument(help="Input path")],
    count: Annotated[int, Option(help="Number of items")] = 10,
) -> dict:
    """Command docstring becomes the command description."""
    return {"path": path, "count": count}
```

Use `Annotated[Type, Argument(...)]` for positional inputs and
`Annotated[Type, Option(...)]` for named options. Target Python 3.10+.

## Behavioral Metadata

Commands should describe their safety profile:

```python
from tooli.annotations import ReadOnly, Idempotent, Destructive, OpenWorld

@app.command(annotations=ReadOnly | Idempotent)
def inspect() -> dict: ...

@app.command(annotations=Destructive)
def delete(path: str) -> dict: ...
```

Annotations appear in schemas, MCP tool definitions, JSON envelopes, and docs.

## Output Contract

Tooli chooses output mode in this order:

1. Explicit `--output`, `--json`, `--jsonl`, `--text`, or `--plain`.
2. `Tooli(default_output=...)`.
3. Auto-detection: TTY gets rich text, non-TTY gets JSON.
4. `TOOLI_OUTPUT`.

Successful agent-mode output:

```json
{
  "ok": true,
  "result": {"command_specific": "data"},
  "meta": {
    "tool": "app-name.command-name",
    "version": "1.0.0",
    "duration_ms": 42,
    "dry_run": false,
    "warnings": [],
    "annotations": {"readOnlyHint": true},
    "truncated": false,
    "next_cursor": null
  }
}
```

Error output:

```json
{
  "ok": false,
  "error": {
    "code": "E3001",
    "category": "state",
    "message": "File not found: ./data.csv",
    "suggestion": {
      "action": "retry_with_modified_input",
      "fix": "Check that the file exists.",
      "example": "my-tool process data.csv"
    },
    "is_retryable": true,
    "details": {"path": "./data.csv"}
  },
  "meta": {}
}
```

## Error Handling

Use structured `tooli` errors instead of raw Python exceptions for user-facing
failures.

| Error | Code family | Exit code | Use for |
|---|---:|---:|---|
| `InputError` | `E1xxx` | 2 | invalid user input |
| `AuthError` | `E2xxx` | 30 | authorization failures |
| `StateError` | `E3xxx` | 10 | missing files, empty results, bad state |
| `ToolRuntimeError` | `E4xxx` | 70 | external runtime failures |
| `InternalError` | `E5xxx` | 70 | framework/internal failures |

```python
from tooli.errors import InputError, StateError, Suggestion

raise InputError(message="Invalid email address", code="E1002", field="email")

raise StateError(
    message="No results matched pattern '*.rs'",
    code="E3001",
    suggestion=Suggestion(
        action="retry_with_modified_input",
        fix="Try a pattern that exists in this directory.",
        example="my-tool find-files '*.py'",
    ),
    details={"pattern": "*.rs"},
    is_retryable=True,
)
```

Use `ToolRuntimeError`, not Python's builtin `RuntimeError`, for tool failures.

## Inputs

Use `StdinOr[T]` when a command should accept stdin, URLs, files, or inline text:

```python
from tooli import StdinOr

@app.command()
def process(
    data: Annotated[StdinOr[str], Argument(help="File, URL, -, or inline text")],
) -> dict:
    return {"length": len(data)}
```

Resolution order: `-` reads stdin, URLs are fetched, file paths are read, and
plain strings are passed through.

Use `SecretInput[T]` for secrets:

```python
from tooli import SecretInput

@app.command()
def deploy(api_key: Annotated[SecretInput[str], Option(help="API key")]) -> dict:
    return {"deployed": True}
```

Tooli adds secret-file/stdin flags, checks `TOOLI_SECRET_<NAME>`, and redacts the
value in telemetry, recordings, and output.

## Dry Runs

Destructive commands should support dry runs:

```python
from tooli import dry_run_support, record_dry_action
from tooli.annotations import Destructive

@app.command(annotations=Destructive, supports_dry_run=True)
@dry_run_support
def cleanup(directory: str) -> dict:
    for path in find_temp_files(directory):
        record_dry_action("delete", path, details={})
        delete_file(path)
    return {"deleted": True}
```

When `--dry-run` is passed, recorded actions are returned instead of executing.

## Pagination

Set `paginated=True` for list-like commands. Tooli adds `--limit`, `--cursor`,
`--fields`, `--filter`, and `--max-items`; `meta.next_cursor` is set when more
results are available.

## Security And Auth

Set app security with `security_policy="off" | "standard" | "strict"`.

Use command-level auth scopes:

```python
@app.command(auth=["admin", "write"])
def sensitive_op() -> dict: ...
```

Configure allowed scopes with `TOOLI_AUTH_SCOPES`. In strict mode, declare
command capabilities such as `fs:read` or `fs:write` and configure
`TOOLI_ALLOWED_CAPABILITIES`.

## Python API

```python
result = app.call("find-files", pattern="*.py")
if result.ok:
    files = result.result
else:
    print(result.error.message)

files = app.call("find-files", pattern="*.py").unwrap()
result = await app.acall("find-files", pattern="*.py")

for item in app.stream("list-files", pattern="*.py"):
    print(item.result)
```

`TooliResult[T]` contains `ok`, `result`, `error`, `meta`, and `unwrap()`.

## MCP

Install MCP support with `pip install 'tooli[mcp]'`.

```bash
my-tool mcp serve --transport stdio
my-tool mcp serve --transport http --port 8080
tooli serve path/to/app.py --transport stdio
tooli serve mypackage.app:app --transport http
```

Register resources and prompts with `app.resource(...)` and `app.prompt(...)`.
Every MCP server auto-registers `skill://manifest` and `skill://documentation`.
Use `--defer-loading` for large apps; it exposes `search_tools` and `run_tool`
instead of every command up front.

## Caller Detection

Agents should identify themselves:

```bash
export TOOLI_CALLER="claude-code"
export TOOLI_CALLER_VERSION="1.4.0"
export TOOLI_SESSION_ID="run-abc123"
```

Use `tooli.detect.detect_execution_context()` and `is_agent()` inside tools when
behavior should adapt to human, agent, CI, or container contexts.

## Testing

```python
from tooli.testing import TooliTestClient

client = TooliTestClient(app)
result = client.invoke(["my-command", "input.txt", "--json"])
payload = client.assert_json_envelope(result)
assert payload["ok"] is True
```

Also test command functions directly and through `app.call(...)`. Clear process
global state in setup when tests depend on caller detection or idempotency:

```python
from tooli.detect import reset_cache
from tooli.idempotency import clear_records

def setup_function():
    reset_cache()
    clear_records()
```

## Global Flags

Tooli injects these flags:

| Flag | Purpose |
|---|---|
| `--output` / `-o` | `auto`, `json`, `jsonl`, `text`, or `plain` |
| `--json` / `--jsonl` | Structured output shortcuts |
| `--text` / `--plain` | Human-readable output shortcuts |
| `--dry-run` | Preview without executing, when supported |
| `--quiet` / `-q` | Suppress non-essential output |
| `--verbose` / `-v` | Increase verbosity |
| `--force` | Force destructive commands where supported |
| `--no-color` | Disable color |
| `--timeout` | Max execution time |
| `--idempotency-key` | Safe retry key |
| `--schema` | Print JSON Schema |
| `--help-agent` | Compact agent-oriented help |
| `--agent-manifest` | Emit agent manifest JSON |
| `--response-format` | `concise` or `detailed` |

Avoid defining command parameters that collide with global flags.

## Environment

Important environment variables:

| Variable | Purpose |
|---|---|
| `TOOLI_CALLER` | Agent or caller slug |
| `TOOLI_CALLER_VERSION` | Caller semver |
| `TOOLI_SESSION_ID` | Tracing session ID |
| `TOOLI_OUTPUT` | Default output mode |
| `TOOLI_SECURITY_POLICY` | `off`, `standard`, or `strict` |
| `TOOLI_AUTH_SCOPES` | Comma-separated allowed scopes |
| `TOOLI_ALLOWED_CAPABILITIES` | Comma-separated strict-mode capabilities |
| `TOOLI_RECORD` | Invocation recording JSONL path |
| `TOOLI_OTEL_ENABLED` | Enable OpenTelemetry spans |
| `TOOLI_INCLUDE_SCHEMA` | Include output schema in envelope |
| `NO_COLOR` | Disable colored output |

## Coding Conventions

- Return dictionaries, lists, or typed data; do not print command results.
- Use `collections.abc.Iterable` and `Callable` instead of `typing` imports.
- Guard `signal.SIGALRM` with `hasattr(signal, "SIGALRM")`.
- Use `click.echo(..., err=True)` for stderr.
- Access command metadata with `get_command_meta(callback)`.
- Keep commands focused, composable, and explicit about safety.
