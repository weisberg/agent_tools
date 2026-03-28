"""
basics.py — Starter plugin with common utility actions.

Drop this in your plugins/ directory. Decorate functions with @menu_action.

The value from config.yaml is dispatched based on its YAML type:
  - scalar  →  func(value)        — single positional arg
  - list    →  func(*values)      — positional args
  - dict    →  func(**values)     — keyword args
  - omitted →  func()             — no args
"""

import subprocess
import webbrowser
from datetime import datetime

from actions import menu_action


# ── Scalar value examples ────────────────────────────────────────

@menu_action(name="greet")
def greet(message):
    """Show a macOS notification with a greeting."""
    import rumps
    rumps.notification(
        title="Hello!",
        subtitle="",
        message=str(message),
    )


@menu_action(name="open_url")
def open_url(url):
    """Open a URL in the default browser."""
    webbrowser.open(str(url))


@menu_action(name="clipboard_timestamp")
def clipboard_timestamp(fmt="%Y-%m-%d %H:%M:%S"):
    """Copy a formatted timestamp to the clipboard."""
    ts = datetime.now().strftime(fmt)
    subprocess.run(["pbcopy"], input=ts.encode(), check=True)

    import rumps
    rumps.notification(title="Copied!", subtitle="", message=ts)


@menu_action(name="shell_command")
def shell_command(cmd):
    """Run a shell command and show the output in a notification."""
    try:
        result = subprocess.run(
            str(cmd), shell=True, capture_output=True, text=True, timeout=10,
        )
        output = result.stdout.strip() or result.stderr.strip() or "(no output)"
    except subprocess.TimeoutExpired:
        output = "Command timed out"
    except Exception as e:
        output = f"Error: {e}"

    import rumps
    rumps.notification(
        title="Shell Output",
        subtitle=str(cmd)[:50],
        message=output[:200],
    )


# ── Dict value (keyword args) example ────────────────────────────
#
# config.yaml:
#   - label: "Greet Brian"
#     action: greet_person
#     value:
#       name: "Brian"
#       greeting: "Hey"
#       title: "Analytics Lead"

@menu_action(name="greet_person")
def greet_person(name, greeting="Hello", title=None):
    """Greet someone by name with optional title."""
    import rumps
    msg = f"{greeting}, {name}!"
    if title:
        msg += f"\n({title})"
    rumps.notification(title="Greeting", subtitle="", message=msg)


# ── List value (positional args) example ─────────────────────────
#
# config.yaml:
#   - label: "Add 2 + 3"
#     action: calculate
#     value: [2, 3]

@menu_action(name="calculate")
def calculate(a, b):
    """Add two numbers and show the result."""
    import rumps
    result = float(a) + float(b)
    rumps.notification(
        title="Calculator",
        subtitle=f"{a} + {b}",
        message=f"= {result}",
    )


# ── Dict value with ${vars} examples ────────────────────────────
#
# config.yaml:
#   vars:
#     colors:
#       primary: "#1A73E8"
#       danger: "#D93025"
#       bg:
#         dark: "#1E1E2E"
#
#   - label: "Deploy Staging"
#     action: styled_notify
#     value:
#       title: "Deploying…"
#       message: "Staging deploy started"
#       color: "${colors.primary}"      ← resolved before dispatch
#       bg: "${colors.bg.dark}"

@menu_action(name="styled_notify")
def styled_notify(title, message, color="#FFFFFF", bg="#000000"):
    """
    Show a notification with color metadata.
    The color/bg values are resolved from ${vars} in config.yaml.
    On macOS, native notifications don't support color, so we log it
    and include the hex in the message for demonstration.
    """
    import rumps
    rumps.notification(
        title=title,
        subtitle=f"color={color}  bg={bg}",
        message=message,
    )


@menu_action(name="status_light")
def status_light(status, color="#FFFFFF", label="Unknown"):
    """
    Show a status notification with a color indicator.
    Receives resolved ${colors.*} values as keyword args.
    """
    import rumps
    emoji = {"healthy": "🟢", "degraded": "🟡", "down": "🔴"}.get(status, "⚪")
    rumps.notification(
        title=f"{emoji} Status: {status.upper()}",
        subtitle=f"[{color}]",
        message=label,
    )
