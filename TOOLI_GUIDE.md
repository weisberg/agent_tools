# Tooli Guide for AI Development Agents

## Introduction

`tooli` is an "agent-native" CLI framework for Python. Its primary purpose is to allow you to write a single Python function that automatically serves as:
1. A human-friendly CLI tool (with rich text output).
2. A machine-consumable agent tool (with structured JSON output and schemas).
3. An MCP (Model Context Protocol) tool server.

As an AI development agent, you should use `tooli` when creating new CLI utilities or Python scripts to ensure they are immediately usable by other agents without requiring text-parsing regex or wrapper scripts.

## Installation

```bash
# Basic installation
pip install tooli

# With MCP server support (recommended for agent tools)
pip install tooli[mcp]
```

## Core Concepts & Usage

### 1. Basic Application Setup

Instantiate a `Tooli` app and use the `@app.command()` decorator to register functions.

```python
from tooli import Tooli, Annotated, Option, Argument

app = Tooli(
    name="my-agent-tools",
    description="A collection of tools for agents",
    version="1.0.0",
)

@app.command()
def hello(
    name: Annotated[str, Argument(help="The name to greet")],
    shout: Annotated[bool, Option(help="Whether to uppercase the output")] = False,
) -> dict:
    """A simple greeting command."""
    greeting = f"Hello, {name}!"
    if shout:
        greeting = greeting.upper()
    return {"message": greeting}

if __name__ == "__main__":
    app()
```

### 2. Behavioral Annotations

`tooli` uses annotations to inform agents about the nature of the tool. You MUST include these annotations so agents know if a command is safe to run.

```python
from tooli.annotations import ReadOnly, Destructive, Idempotent

# Safe to run multiple times, doesn't change state
@app.command(annotations=ReadOnly | Idempotent)
def read_config() -> dict:
    ...

# Changes state, use with caution
@app.command(annotations=Destructive)
def delete_file(path: str) -> dict:
    ...
```

### 3. Unified Inputs (`StdinOr`)

When a tool needs to read data, use `StdinOr[T]`. This allows the tool to seamlessly accept input from a file path, a URL, or standard input (piped data), making it highly composable in bash pipelines.

```python
from tooli import StdinOr
from pathlib import Path

@app.command()
def process_data(
    input_data: Annotated[StdinOr[Path], Argument(help="Input file, URL, or stdin")],
) -> dict:
    """Process data from any source."""
    # input_data is automatically resolved to a usable format
    pass
```

### 4. Dry-Run Support

For destructive commands, you should implement dry-run support so agents can preview changes before committing to them.

```python
from tooli import dry_run_support, record_dry_action
from tooli.annotations import Destructive

@app.command(annotations=Destructive)
@dry_run_support
def deploy_server(target: str) -> dict:
    # Record what WOULD happen
    record_dry_action("provision", target, details={"size": "large"})
    
    # Actual logic here...
    return {"status": "deployed", "target": target}
```
*Note: Agents can invoke this with `--dry-run --json` to get a structured preview.*

### 5. Standard Global Flags

Every `tooli` command automatically receives standard flags that are highly useful for agents. You do not need to implement these yourself:
- `--json`: Forces a structured JSON output envelope (`{"ok": true, "result": ...}`).
- `--jsonl`: Newline-delimited JSON for streaming.
- `--dry-run`: Previews actions without side-effects (if supported).
- `--schema`: Outputs the JSON schema of the command.
- `--agent-bootstrap`: Generates a `SKILL.md` file for the tool.

### 6. Error Handling

Do not raise raw Python exceptions for user errors. Return or raise `tooli` structured errors so that the agent receives an actionable recovery payload rather than a plain string stack trace. `tooli` provides typed errors like `InputError`, `AuthError`, `StateError`, etc.

## Agent Workflows

### Running as an MCP Server
Any `tooli` app can instantly serve as an MCP tool server. 
```bash
python my_app.py mcp serve --transport stdio
```

### Generating Documentation
You can automatically generate agent-optimized documentation (like `SKILL.md` or `CLAUDE.md`):
```bash
python my_app.py generate-skill > SKILL.md
```

## Summary Checklist for Agents Writing `tooli` Code:
- [ ] Use `Annotated[Type, Argument/Option(...)]` for all parameters.
- [ ] Provide clear docstrings for the function (used as the tool description).
- [ ] Add behavioral annotations (`ReadOnly`, `Destructive`, etc.).
- [ ] Return structured data (e.g., `dict`, `list`) instead of printing strings.
- [ ] Use `StdinOr` for file inputs.
- [ ] Support `--dry-run` for state-mutating commands.
