# Tooli Developer Guide

A comprehensive reference for agents building tools with the tooli framework.

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

Install tooli: `pip install tooli` (add `[mcp]` for MCP server support).

---

## Table of Contents

1. [App Configuration](#app-configuration)
2. [Registering Commands](#registering-commands)
3. [Parameter Types](#parameter-types)
4. [Behavioral Annotations](#behavioral-annotations)
5. [Output System](#output-system)
6. [JSON Envelope](#json-envelope)
7. [Error Handling](#error-handling)
8. [Special Input Types](#special-input-types)
9. [Dry-Run Support](#dry-run-support)
10. [Pagination](#pagination)
11. [Security and Auth](#security-and-auth)
12. [Python API](#python-api)
13. [MCP Server Integration](#mcp-server-integration)
14. [Caller Detection](#caller-detection)
15. [Testing](#testing)
16. [Global Flags](#global-flags)
17. [Environment Variables](#environment-variables)
18. [Exit Codes](#exit-codes)
19. [Coding Conventions](#coding-conventions)
20. [Common Patterns](#common-patterns)

---

## App Configuration

```python
app = Tooli(
    name="my-tool",                      # CLI command name
    description="My tool description",   # Shown in --help
    version="1.0.0",                     # Appears in envelope meta

    # Output
    default_output="auto",               # "auto"|"json"|"jsonl"|"text"|"plain"
    mcp_transport="stdio",               # Default MCP transport

    # Agent-facing metadata
    triggers=["when you need to ..."],   # When agents should use this tool
    anti_triggers=["not for ..."],       # When NOT to use
    workflows=[{                         # Multi-step workflow docs
        "name": "lifecycle",
        "steps": ["create", "list", "delete"],
    }],
    rules=["Always validate before writing"],

    # Environment variable docs
    env_vars={
        "MY_TOKEN": {"description": "API token", "default": ""},
    },

    # Security
    security_policy="standard",          # "off"|"standard"|"strict"
    auth_scopes=["read", "write"],

    # Invocation recording
    record=True,                         # True=default path, str=custom path

    # Backend
    backend="typer",                     # "typer"|"native"
)
```

### Key Methods

| Method | Description |
|---|---|
| `app.command(...)` | Register a command (decorator) |
| `app.call(name, **kwargs)` | Invoke command synchronously → `TooliResult` |
| `app.acall(name, **kwargs)` | Invoke command asynchronously → `TooliResult` |
| `app.stream(name, **kwargs)` | Yield `TooliResult` items from list-returning commands |
| `app.astream(name, **kwargs)` | Async stream of `TooliResult` items |
| `app.get_command(name)` | Look up callback by name |
| `app.get_tools()` | Return all `ToolDef` objects |
| `app.resource(uri, ...)` | Register an MCP resource |
| `app.prompt(name, ...)` | Register an MCP prompt |
| `app()` | Run as CLI |

---

## Registering Commands

```python
@app.command(
    name=None,                          # Defaults to function name (hyphenated)

    # Behavioral hints
    annotations=ReadOnly | Idempotent,  # Composable with |

    # Agent metadata
    task_group="Query",                 # Groups commands in docs
    when_to_use="Search for patterns",  # One-line guidance for agents
    capabilities=["fs:read"],           # Permission declarations
    handoffs=[                          # Delegation hints
        {"command": "extract", "when": "need full content"},
    ],
    delegation_hint="Use before filter",
    examples=[
        {"args": ["--pattern", "*.py"], "description": "Find Python files"},
    ],
    error_codes={
        "E1001": "Pattern is invalid",
        "E3001": "No files found",
    },
    output_example=[{"line": 42, "text": "match"}],

    # Pagination
    paginated=False,                    # Adds --limit, --cursor, --fields, --filter
    list_processing=False,              # Enables --null and NUL-delimited stdin

    # Safety
    supports_dry_run=False,
    requires_approval=False,
    danger_level=None,                  # "high", "low"
    human_in_the_loop=False,            # Requires confirmation in STRICT mode

    # Auth
    auth=[],                            # Required auth scopes

    # Misc
    timeout=None,                       # Seconds
    max_tokens=None,                    # Token budget; triggers truncation
    version=None,                       # Creates versioned alias
    deprecated=False,
    deprecated_message=None,
)
def my_command(
    arg: Annotated[str, Argument(help="Positional argument")],
    option: Annotated[int, Option(help="Optional value")] = 10,
) -> dict:
    """Command docstring becomes the description."""
    return {"result": arg, "count": option}
```

Commands must be registered before `app()` is called.

---

## Parameter Types

```python
from tooli import Annotated, Argument, Option

# Positional argument
path: Annotated[str, Argument(help="File path")]

# Named option with default
count: Annotated[int, Option(help="Number of items")] = 10

# Optional (None by default)
tag: Annotated[str | None, Option(help="Optional tag")] = None

# Boolean flag
verbose: Annotated[bool, Option(help="Verbose output")] = False
```

`Argument` and `Option` are re-exported from typer and are syntactically compatible.

---

## Behavioral Annotations

```python
from tooli.annotations import ReadOnly, Idempotent, Destructive, OpenWorld
```

| Annotation | Meaning |
|---|---|
| `ReadOnly` | Does not modify state |
| `Idempotent` | Safe to retry, same result |
| `Destructive` | Modifies or deletes data |
| `OpenWorld` | Makes network/external calls |

Compose with `|`:

```python
@app.command(annotations=ReadOnly | Idempotent)
@app.command(annotations=Destructive | Idempotent)
```

Annotations appear in JSON Schema, MCP tool definitions, envelope meta, and docs.

---

## Output System

Every command auto-detects the output mode:

- **TTY (interactive)**: Rich-formatted via `rich.print()`
- **Non-TTY / piped**: JSON envelope to stdout
- **Explicit flags**: `--json`, `--jsonl`, `--plain`, `--text`

### Output Mode Resolution Order

1. Explicit `--output`/`--json`/`--jsonl` flag
2. `Tooli(default_output=...)` app-level default
3. Auto-detection: TTY → Rich, non-TTY → JSON
4. `TOOLI_OUTPUT` environment variable

---

## JSON Envelope

Every command in agent mode returns a standard envelope:

```json
{
  "ok": true,
  "result": "<command-specific-data>",
  "meta": {
    "tool": "app-name.command-name",
    "version": "1.0.0",
    "duration_ms": 42,
    "dry_run": false,
    "warnings": [],
    "annotations": {"readOnlyHint": true},
    "truncated": false,
    "next_cursor": null,
    "caller_id": "claude-code",
    "session_id": "abc-123"
  }
}
```

On error:

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
  "meta": { "..." }
}
```

---

## Error Handling

### Error Hierarchy

```
ToolError (base)
├── InputError        (E1xxx) — exit code 2  — input validation
├── AuthError         (E2xxx) — exit code 30 — authorization
├── StateError        (E3xxx) — exit code 10 — precondition/state
├── ToolRuntimeError  (E4xxx) — exit code 70 — external/runtime
└── InternalError     (E5xxx) — exit code 70 — framework
```

### Raising Errors

```python
from tooli.errors import InputError, StateError, ToolRuntimeError, Suggestion

# Simple
raise InputError(message="File not found: ./foo.txt", code="E1001")

# With suggestion
raise StateError(
    message="No results matched pattern '*.rs'",
    code="E3001",
    suggestion=Suggestion(
        action="retry_with_modified_input",
        fix="The directory contains .py files. Try '*.py' instead.",
        example="my-tool find-files '*.py'",
    ),
    details={"pattern": "*.rs", "directory": "."},
    is_retryable=True,
)

# With field reference
raise InputError(
    message="Invalid email address",
    code="E1002",
    field="email",
)
```

**Important**: Use `ToolRuntimeError`, never Python's `RuntimeError` (shadows the builtin).

---

## Special Input Types

### `StdinOr[T]` — Flexible Input Sources

Accepts file paths, URLs, or stdin (`-`):

```python
from tooli import StdinOr

@app.command()
def process(
    data: Annotated[StdinOr[str], Argument(help="File, URL, or - for stdin")],
) -> dict:
    return {"length": len(data)}
```

Resolution order: `"-"` → stdin, URL → fetch, file path → read, string → pass through.

### `SecretInput[T]` — Secret Parameters

```python
from tooli import SecretInput

@app.command()
def deploy(
    api_key: Annotated[SecretInput[str], Option(help="API key")],
) -> dict:
    return {"deployed": True}
```

The framework automatically:
- Adds `--api-key-secret-file` and `--api-key-secret-stdin` flags
- Redacts secret values in telemetry, recording, and output
- Checks `TOOLI_SECRET_API_KEY` env var as fallback

---

## Dry-Run Support

```python
from tooli import dry_run_support, record_dry_action
from tooli.annotations import Destructive

@app.command(annotations=Destructive, supports_dry_run=True)
@dry_run_support
def cleanup(directory: str) -> dict:
    """Delete temp files."""
    files = glob.glob(f"{directory}/**/*.tmp", recursive=True)
    for f in files:
        record_dry_action("delete", f, details={"size": os.path.getsize(f)})
        os.unlink(f)
    return {"deleted": len(files)}
```

When `--dry-run` is passed, the decorator returns the recorded actions instead of executing:

```json
{
  "ok": true,
  "result": [
    {"action": "delete", "target": "/tmp/foo.tmp", "details": {"size": 1024}}
  ],
  "meta": {"dry_run": true}
}
```

---

## Pagination

Enable with `paginated=True`:

```python
@app.command(paginated=True, annotations=ReadOnly)
def list_items() -> list[dict]:
    return [{"id": i} for i in range(100)]
```

Adds these flags automatically:

| Flag | Purpose |
|---|---|
| `--limit N` | Max items to return |
| `--cursor N` | Offset-based cursor |
| `--fields a,b,c` | Project only listed keys |
| `--filter key=value` | Filter items by dict equality |
| `--max-items N` | Absolute cap |

Cursor is integer offset. `meta.next_cursor` is set when results are truncated.

---

## Security and Auth

### Security Policies

```python
app = Tooli(name="my-tool", security_policy="standard")
```

| Policy | Behavior |
|---|---|
| `off` | No enforcement |
| `standard` | Prompts for destructive commands (unless `--yes`) |
| `strict` | Full enforcement + capability checks |

### Auth Scopes

```python
@app.command(auth=["admin", "write"])
def sensitive_op() -> dict: ...
```

Set allowed scopes via `TOOLI_AUTH_SCOPES="read,write,admin"`. Missing scopes raise `AuthError(code="E2001")`.

### Capability Enforcement (STRICT mode)

```python
@app.command(capabilities=["fs:read", "fs:write"])
def write_file() -> dict: ...
```

Set `TOOLI_ALLOWED_CAPABILITIES="fs:read,net:read"` — unmatched capabilities raise `AuthError`.

---

## Python API

Invoke commands programmatically without going through CLI parsing:

```python
# Synchronous
result = app.call("find-files", pattern="*.py")
if result.ok:
    files = result.result
else:
    print(result.error.message)

# Raise on error
files = app.call("find-files", pattern="*.py").unwrap()

# Async
result = await app.acall("find-files", pattern="*.py")

# Stream list results
for item in app.stream("list-files", pattern="*.py"):
    print(item.result)

# Dry-run via API
result = app.call("cleanup", directory="/tmp", dry_run=True)
```

### `TooliResult[T]`

```python
@dataclass(frozen=True)
class TooliResult(Generic[T]):
    ok: bool
    result: T | None
    error: TooliError | None
    meta: dict[str, Any] | None

    def unwrap(self) -> T:  # Returns result or raises ToolError
```

---

## MCP Server Integration

Requires `pip install tooli[mcp]`.

### Serving

```bash
# As CLI subcommand
my-tool mcp serve --transport stdio
my-tool mcp serve --transport http --port 8080

# Via tooli CLI
tooli serve path/to/app.py --transport stdio
tooli serve mypackage.app:app --transport http
```

### Resources and Prompts

```python
@app.resource(
    uri="skill://config",
    description="Current configuration",
    mime_type="application/json",
)
def get_config() -> str:
    return json.dumps({"version": "1.0.0"})

@app.prompt(name="system-prompt", description="System prompt for agents")
def my_prompt() -> str:
    return "You are a helpful assistant."
```

Every MCP server auto-registers `skill://manifest` and `skill://documentation` resources.

### Deferred Loading

```bash
my-tool mcp serve --transport stdio --defer-loading
```

Exposes only two meta-tools (`search_tools`, `run_tool`) instead of all commands — useful for apps with many commands.

---

## Caller Detection

Tooli auto-detects who is running the CLI (human, AI agent, CI/CD, container).

### Setting Caller Identity

Agents should set these env vars before invoking any Tooli CLI:

```bash
export TOOLI_CALLER="claude-code"
export TOOLI_CALLER_VERSION="1.4.0"
export TOOLI_SESSION_ID="run-abc123"
```

Well-known slugs: `claude-code`, `cursor`, `copilot-workspace`, `aider`, `devin`, `windsurf`, `langchain`, `crewai`, and others.

### Detection API

```python
from tooli.detect import detect_execution_context, is_agent

ctx = detect_execution_context()
ctx.category      # CallerCategory.AI_AGENT
ctx.agent_name    # "Claude Code"
ctx.confidence    # 1.0
ctx.is_agent      # True
```

---

## Testing

### Using TooliTestClient

```python
from tooli.testing import TooliTestClient

client = TooliTestClient(app)

def test_my_command():
    result = client.invoke(["my-command", "input.txt", "--json"])
    payload = client.assert_json_envelope(result)
    assert payload["ok"] is True
```

### Direct Function Testing

```python
result = my_command(input_file="test.txt", count=5)
assert isinstance(result, list)
```

### Python API Testing

```python
result = app.call("my-command", input_file="test.txt")
assert result.ok
assert len(result.result) == 5
```

### Test Cleanup

```python
from tooli.detect import reset_cache
from tooli.idempotency import clear_records

def setup_function():
    reset_cache()
    clear_records()
```

Run tests with: `pytest -x -q`

---

## Global Flags

Every command automatically gets these flags (no need to declare them):

| Flag | Purpose |
|---|---|
| `--output` / `-o` | Output mode: `auto\|json\|jsonl\|text\|plain` |
| `--json` | Alias for `--output json` |
| `--jsonl` | Alias for `--output jsonl` |
| `--text` / `--plain` | Text output aliases |
| `--dry-run` | Preview without executing |
| `--quiet` / `-q` | Suppress non-essential output |
| `--verbose` / `-v` | Increase verbosity (`-vvv`) |
| `--yes` / `-y` | Skip confirmation prompts |
| `--force` | Force destructive commands |
| `--no-color` | Disable colored output |
| `--timeout` | Max execution time (seconds) |
| `--idempotency-key` | Safe-retry key |
| `--schema` | Print JSON Schema and exit |
| `--help-agent` | Compact YAML help for agents |
| `--agent-manifest` | Emit agent manifest JSON and exit |
| `--response-format` | `concise\|detailed` |

---

## Environment Variables

| Variable | Purpose |
|---|---|
| `TOOLI_CALLER` | Agent self-identification slug |
| `TOOLI_CALLER_VERSION` | Semver of calling agent |
| `TOOLI_SESSION_ID` | Tracing session ID |
| `TOOLI_OUTPUT` | Default output mode |
| `TOOLI_SECURITY_POLICY` | `off\|standard\|strict` |
| `TOOLI_AUTH_SCOPES` | Comma-separated allowed scopes |
| `TOOLI_ALLOWED_CAPABILITIES` | Comma-separated capabilities (STRICT) |
| `TOOLI_RECORD` | Invocation recording JSONL path |
| `TOOLI_OTEL_ENABLED` | Enable OpenTelemetry spans |
| `TOOLI_INCLUDE_SCHEMA` | Include output schema in envelope |
| `NO_COLOR` | Disable colored output |

---

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 2 | Invalid input (E1xxx) |
| 10 | State error (E3xxx) |
| 20 | Missing required input |
| 30 | Auth denied (E2xxx) |
| 40 | Runtime unavailable |
| 50 | Timeout expired |
| 65 | Generic failure |
| 70 | Internal error (E4xxx, E5xxx) |
| 75 | Partial failure |
| 101 | Human handoff required |

---

## Coding Conventions

- Use `ToolRuntimeError`, never `RuntimeError` (shadows the builtin)
- Use `collections.abc.Iterable`/`Callable` for imports (not `typing`)
- Guard `signal.SIGALRM` with `hasattr(signal, "SIGALRM")` for cross-platform
- Use `click.echo(..., err=True)` for stderr output, not `print()`
- Access command metadata via `get_command_meta(callback)` — never individual `__tooli_xxx__` attributes
- Target Python 3.10+

---

## Common Patterns

### Full Agent Metadata

```python
@app.command(
    annotations=ReadOnly | Idempotent,
    task_group="Query",
    when_to_use="Search for patterns in log files",
    capabilities=["fs:read"],
    handoffs=[{"command": "filter", "when": "need to narrow results"}],
    examples=[{"args": ["--pattern", "ERROR"], "description": "Find errors"}],
    error_codes={"E1001": "Invalid regex", "E3001": "File not found"},
    output_example=[{"line": 42, "text": "match"}],
    paginated=True,
    max_tokens=8000,
)
def search(
    file: Annotated[str, Argument(help="Log file path")],
    pattern: Annotated[str, Option(help="Search regex")],
) -> list[dict]:
    """Search log file for matching lines."""
    ...
```

### Destructive Command with Dry-Run

```python
@app.command(annotations=Destructive, supports_dry_run=True)
@dry_run_support
def deploy(target: str) -> dict:
    record_dry_action("upload", target, details={"size": "12MB"})
    record_dry_action("restart", f"{target}-service")
    return {"deployed": target}
```

### Orchestration (Multi-Tool Plans)

```bash
echo '[
  {"command": "stats", "arguments": {"path": "README.md"}},
  {"command": "search", "arguments": {"path": "README.md", "pattern": "def "}}
]' | my-tool orchestrate run --json
```

### Agent Framework Integration

```python
# From another Python module
result = app.call("search", file="app.log", pattern="ERROR")
if result.ok:
    for match in result.result:
        print(match["text"])
else:
    print(result.error.suggestion["fix"])
```
