# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Barli is a macOS menu bar application framework using a decorator-based plugin system. Plugins self-register via `@menu_action`, are discovered dynamically, and the menu hot-reloads on file changes without restarting.

## Running

```bash
pip install -r requirements.txt
python app.py                          # default config.yaml
python app.py --config ~/my/config.yaml  # custom config
```

No build step. A ⚡ icon appears in the macOS menu bar.

## Architecture

Five focused modules with minimal cross-dependencies:

| Module | Role |
|--------|------|
| `app.py` | `PluginMenuBarApp` (extends `rumps.App`): menu building, callbacks, hot-reload orchestration |
| `actions.py` | `@menu_action` decorator + thread-safe registry (`_registry`, `_lock`) |
| `config.py` | YAML loader with `${variable}` resolution (dot-notation, type-preserving, up to 10 passes) |
| `loader.py` | Dynamic plugin discovery, `importlib`-based import/reload |
| `watcher.py` | Watchdog file monitoring with 0.5s debounce |

## Plugin System

### Writing a Plugin

```python
# plugins/my_plugin.py
from actions import menu_action

@menu_action(name="my_action")
def my_action(url):
    webbrowser.open(url)
```

### Configuring in config.yaml

```yaml
vars:
  site: "https://example.com"

menu:
  - label: "Open Site"
    action: my_action
    value: "${site}"       # scalar → func(value)
  - separator: true
  - label: "Submenu"
    submenu:
      - label: "Child Item"
        action: child_action
        value: {key: val}  # dict → func(**value)
```

### Value Dispatch Rules

| YAML type | Python call |
|-----------|-------------|
| scalar | `func(value)` |
| list | `func(*value)` |
| dict | `func(**value)` |
| omitted | `func()` |

### Auto-Discovery

Actions decorated with `@menu_action` but not listed in `config.yaml` appear automatically at the bottom of the menu under `[auto]`. Useful for prototyping.

## Hot-Reload

Watchdog monitors `plugins/` and the config directory. Changes debounce at 0.5s, then `_on_changes_debounced` schedules `_apply_changes` on the main AppKit thread via `rumps.Timer`. Deleted plugins are unloaded; new ones are auto-discovered.

## Key Behaviors

- Missing action in registry → menu item disabled, labeled `"(not loaded)"`
- Exception in action callback → `rumps.notification` shown, app keeps running
- Thread safety: watchdog thread and AppKit thread share registry via `_lock`
- Variables: `${colors.primary}` resolves with dot-notation; dict/list types are preserved (not stringified)

## No Tests

There is no test suite. Manual testing is done by running the app and verifying menu behavior; hot-reload makes this fast.
