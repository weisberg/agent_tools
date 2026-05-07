---
name: barli
description: Use barli to build and operate a macOS menu bar automation app that discovers Python functions via decorators and configures menus from YAML. Use when the user wants local menubar actions, hot-reloaded Python plugins, YAML-driven command menus, or lightweight GUI launchers for agent tools.
---

# barli — Agent Guide

`barli` is a macOS menu bar app. It discovers Python functions decorated with
`@menu_action`, builds a menu from `config.yaml`, and hot-reloads when plugin
files change.

## Agent Contract

- Treat `config.yaml` as the menu source of truth.
- Treat `plugins/*.py` as the action implementation surface.
- Keep plugin actions small and deterministic.
- Prefer explicit YAML values over hard-coded constants in Python.
- After editing config or plugins, restart or rely on the watcher and verify the
  menu appears.
- Do not put secrets directly in `config.yaml`; read them from environment
  variables or a secure store inside the plugin.

## Setup

From `tools/barli`:

```bash
pip install -r requirements.txt
python app.py
```

A menu bar icon appears on macOS. Click it to run configured actions.

## Mental Model

```text
config.yaml      -> labels, ordering, values, submenus
plugins/*.py     -> @menu_action functions
app.py           -> loads config, builds menu, watches for changes
```

## Write A Plugin

Create or edit a Python file under `plugins/`:

```python
from actions import menu_action

@menu_action(name="greet")
def greet(message):
    print(message)
```

Then wire it in `config.yaml`:

```yaml
menu:
  - label: "Say Hello"
    action: greet
    value: "hello from barli"
```

## YAML Value Dispatch

| YAML value | Python call |
|---|---|
| scalar | `func(value)` |
| list | `func(*values)` |
| dict | `func(**values)` |
| omitted | `func()` |

Examples:

```yaml
menu:
  - label: "Add Numbers"
    action: calculate
    value: [2, 3]

  - label: "Greet Brian"
    action: greet_person
    value:
      name: Brian
      greeting: Hey

  - separator: true

  - label: "Nested"
    submenu:
      - label: "Child"
        action: greet
        value: "inside submenu"
```

## Safe Development Workflow

1. Read `config.yaml` and list existing action names.
2. Add or update one plugin function.
3. Add a matching menu entry.
4. Run `python app.py` in a GUI macOS session.
5. Watch terminal logs for import errors or action exceptions.
6. Keep the menu small enough for a human to scan.

## Error Recovery

- Menu item does nothing: verify `action:` matches `@menu_action(name=...)`.
- Plugin does not load: run the file or `python app.py` and inspect import
  errors.
- Wrong argument count: compare YAML value shape with the function signature.
- GUI unavailable: do not try to launch the app; document the config/plugin
  change and leave runtime verification for a macOS GUI session.
