# ⚡ Plugin Menu Bar

A macOS menu bar app that discovers Python functions via decorators, hot-reloads on file changes, and builds its menu from `config.yaml`.

## Quick Start

```bash
pip install -r requirements.txt
python app.py
```

A ⚡ icon appears in your menu bar. Click it to see your configured actions.

## How It Works

```
config.yaml          →  defines menu labels, order, values
plugins/*.py         →  contains @menu_action decorated functions
app.py               →  builds menu, watches for changes, hot-reloads
```

### 1. Write a Plugin

Drop a `.py` file in `plugins/`. Decorate functions with `@menu_action`:

```python
from actions import menu_action

@menu_action(name="my_action")
def my_action(value):
    """value comes from config.yaml"""
    print(f"Got: {value}")
```

The `value` field in config.yaml supports three dispatch modes based on its YAML type:

| YAML type | Python call | Example |
|-----------|-------------|---------|
| scalar | `func(value)` | `value: "hello"` |
| list | `func(*values)` | `value: [1, 2, 3]` |
| dict | `func(**values)` | `value: {name: "Brian", count: 5}` |
| omitted | `func()` | *(no value key)* |

```python
# Scalar — one positional arg
@menu_action(name="greet")
def greet(message):
    print(message)

# List — multiple positional args
@menu_action(name="calculate")
def calculate(a, b):
    print(a + b)

# Dict — keyword args (with defaults)
@menu_action(name="greet_person")
def greet_person(name, greeting="Hello", title=None):
    print(f"{greeting}, {name}!")

# No value — no args
@menu_action(name="system_info")
def system_info():
    print("info")
```

### 2. Configure the Menu

Edit `config.yaml` to control what appears and in what order:

```yaml
menu:
  - label: "Do the Thing"     # what the user sees
    action: my_action          # matches @menu_action(name="...")
    value: "hello world"       # scalar → func("hello world")

  - label: "Add Numbers"
    action: calculate
    value: [2, 3]              # list → func(2, 3)

  - label: "Greet Brian"
    action: greet_person
    value:                      # dict → func(name="Brian", greeting="Hey")
      name: "Brian"
      greeting: "Hey"

  - separator: true            # visual divider

  - label: "Nested Menu"       # submenus
    submenu:
      - label: "Child Item"
        action: my_action
        value: "from submenu"
```

### 3. Hot-Reload

The app watches `plugins/` and `config.yaml` continuously:

- **Edit a plugin** → functions are re-imported, menu rebuilds
- **Add a new .py file** → new actions appear in the menu automatically
- **Delete a plugin** → its actions are removed
- **Edit config.yaml** → menu restructures itself

No restart needed.

## Project Structure

```
menubar-app/
├── app.py              # Main entry point (rumps app)
├── actions.py          # @menu_action decorator + action registry
├── config.py           # YAML config loader + variable resolution
├── config.yaml         # Menu definition + variables
├── loader.py           # Module discovery and import/reload
├── watcher.py          # Watchdog file observer + debounce
├── requirements.txt
└── plugins/
    ├── basics.py       # Greet, open URL, clipboard, shell, styled
    └── system_info.py  # System info, disk usage, IP address
```

## Variables

Define reusable values once under `vars:` and reference them anywhere with `${path.to.var}`:

```yaml
vars:
  colors:
    primary: "#1A73E8"
    danger: "#D93025"
    bg:
      dark: "#1E1E2E"
  brand:
    name: "Acme Corp"
    url: "https://acme.com"
  # Vars can reference other vars
  theme:
    alert_color: "${colors.danger}"

menu:
  - label: "Deploy"
    action: deploy
    value:
      color: "${colors.primary}"       # → "#1A73E8"
      bg: "${colors.bg.dark}"          # → "#1E1E2E"

  - label: "${brand.name} Site"        # → "Acme Corp Site"
    action: open_url
    value: "${brand.url}"              # → "https://acme.com"

  - label: "Alert"
    action: notify
    value:
      color: "${theme.alert_color}"    # → "#D93025" (resolved via chain)
```

Resolution rules:

- **Dot notation** for nested access: `${colors.bg.dark}`
- **Type preservation**: if the entire string is `"${some.var}"` and the var is a dict/list/int, you get that type (not a string)
- **String interpolation**: `"Hello ${brand.name}!"` → `"Hello Acme Corp!"` (always a string)
- **Cross-references**: vars can reference other vars (resolved iteratively)
- **Unresolved refs**: left as-is with a warning logged

## Config Reference

```yaml
vars:                            # Optional: reusable variables
  my_color: "#1A73E8"

app:
  title: "⚡"              # Menu bar text
  icon: "icon.png"          # Optional .png icon (16x16 or 18x18)
  tooltip: "Plugin Menu Bar"

plugins_dir: "plugins"      # Where to look for .py files

menu:                        # Ordered list of menu items
  - label: "Display Text"
    action: action_name      # @menu_action(name="action_name")
    value: "anything"        # Scalar → func("anything")
    icon: "item.png"         # Optional per-item icon

  - label: "Multi-Arg"
    action: some_action
    value: ["arg1", "arg2"]  # List → func("arg1", "arg2")

  - label: "Styled"
    action: other_action
    value:                    # Dict → func(color="#1A73E8", n=42)
      color: "${my_color}"
      n: 42

  - separator: true

  - label: "Submenu"
    submenu:
      - label: "Child"
        action: child_action
        value: 42
```

**Auto-discovery:** Any `@menu_action` functions NOT referenced in the config still appear at the bottom of the menu under `[auto]` labels, so new plugins are usable immediately even before you update the config.

## Writing Plugins — Tips

- Value dispatch: `dict` → `**kwargs`, `list` → `*args`, scalar → single arg, omitted → no args
- Use default parameter values for optional config fields (e.g. `color="#FFF"`)
- Use `rumps.notification(title, subtitle, message)` for user feedback
- Use `subprocess.run(["pbcopy"], input=text.encode())` for clipboard
- Use `webbrowser.open(url)` to open URLs
- Keep imports inside functions if they're heavy (faster startup)
- Any uncaught exception shows a notification — the app keeps running

## Custom Config Path

```bash
python app.py --config ~/my-menus/config.yaml
```

The `plugins_dir` in that config is resolved relative to the config file's location.

## Running in the Background

Running `python app.py` in a terminal works but ties up a window. Two better options:

### Option 1: Launch Agent (recommended for development)

Create a `launchd` plist so macOS runs the app automatically at login with no terminal window. It auto-restarts on crash.

Create `~/Library/LaunchAgents/com.menubar.plugins.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.menubar.plugins</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/bin/python3</string>
        <string>/full/path/to/menubar-app/app.py</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/menubar-app.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/menubar-app.err</string>
</dict>
</plist>
```

Update the python and script paths to match your environment (use `which python3` to find yours). If you use a virtualenv, point to its python binary instead.

Then manage it with:

```bash
# Start
launchctl load ~/Library/LaunchAgents/com.menubar.plugins.plist

# Stop
launchctl unload ~/Library/LaunchAgents/com.menubar.plugins.plist

# Check status
launchctl list | grep menubar

# View logs
tail -f /tmp/menubar-app.log
```

This is the best option while actively developing — no rebuild step, and edits to plugins and config hot-reload as usual.

### Option 2: py2app (native .app bundle)

Package the project as a standalone macOS application you can double-click, put in `/Applications`, or add to Login Items.

Add a `setup.py`:

```python
from setuptools import setup

APP = ['app.py']
DATA_FILES = [
    ('plugins', ['plugins/basics.py', 'plugins/system_info.py']),
    ('.', ['config.yaml']),
]
OPTIONS = {
    'argv_emulation': False,
    'plist': {
        'LSUIElement': True,  # hides from Dock (menu bar only)
    },
    'packages': ['rumps', 'watchdog', 'yaml'],
}

setup(
    app=APP,
    data_files=DATA_FILES,
    options={'py2app': OPTIONS},
    setup_requires=['py2app'],
)
```

Build and run:

```bash
pip install py2app
python setup.py py2app
open dist/app.app
```

To launch at login, drag the `.app` into **System Settings → General → Login Items**.

This is the better option if you want to distribute it or want a fully self-contained app. The tradeoff is a rebuild step (`python setup.py py2app`) whenever you change core files — though plugins and config still hot-reload from the bundled paths.
