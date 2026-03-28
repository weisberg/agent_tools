"""
Config loader — reads config.yaml, resolves ${var} references,
and returns structured menu config.

Variable system:
  - Define variables under a top-level `vars:` key (supports nesting)
  - Reference them anywhere with ${path.to.var}
  - Variables can reference other variables
  - Works in strings, lists, dicts, and inside larger strings

Examples:
    vars:
      colors:
        primary: "#1A73E8"
        danger: "#D93025"
      app_name: "My App"

    menu:
      - label: "${app_name} — Deploy"
        action: deploy
        value:
          color: "${colors.primary}"
"""

import logging
import re
from pathlib import Path

import yaml

log = logging.getLogger("menubar.config")

DEFAULT_CONFIG = {
    "app": {
        "title": "⚡",
        "icon": None,
        "tooltip": "Plugin Menu Bar",
    },
    "plugins_dir": "plugins",
    "menu": [],
}

# Matches ${some.dotted.path}
_VAR_PATTERN = re.compile(r"\$\{([^}]+)\}")

# Max resolution passes (to handle vars referencing other vars)
_MAX_RESOLVE_DEPTH = 10


def _lookup(var_path: str, variables: dict):
    """
    Walk a dot-separated path into a nested dict.
    Returns the value if found, or None if any segment is missing.

    Example: _lookup("colors.primary", {"colors": {"primary": "#FFF"}})
             → "#FFF"
    """
    keys = var_path.strip().split(".")
    current = variables
    for key in keys:
        if isinstance(current, dict) and key in current:
            current = current[key]
        else:
            return None
    return current


def _resolve_string(s: str, variables: dict) -> str | dict | list | int | float:
    """
    Resolve ${...} references within a string.

    Two modes:
      1. The ENTIRE string is a single reference like "${colors.bg}"
         → return the raw value (preserving type: dict, list, int, etc.)
      2. The string contains references mixed with text like "Color: ${colors.bg}"
         → substitute in-place, always returns a string
    """
    # Case 1: entire string is one variable reference
    match = _VAR_PATTERN.fullmatch(s)
    if match:
        result = _lookup(match.group(1), variables)
        if result is not None:
            return result
        log.warning("Unresolved variable: ${%s}", match.group(1))
        return s

    # Case 2: mixed string — substitute each reference
    def _replacer(m):
        val = _lookup(m.group(1), variables)
        if val is None:
            log.warning("Unresolved variable: ${%s}", m.group(1))
            return m.group(0)  # leave as-is
        return str(val)

    return _VAR_PATTERN.sub(_replacer, s)


def _resolve(node, variables: dict):
    """
    Recursively resolve ${...} references in any data structure.
    """
    if isinstance(node, str):
        return _resolve_string(node, variables)
    elif isinstance(node, dict):
        return {k: _resolve(v, variables) for k, v in node.items()}
    elif isinstance(node, list):
        return [_resolve(item, variables) for item in node]
    else:
        return node


def _resolve_vars_block(variables: dict) -> dict:
    """
    Resolve references *within* the vars block itself so that
    vars can reference other vars. Iterates until stable.
    """
    for _ in range(_MAX_RESOLVE_DEPTH):
        resolved = _resolve(variables, variables)
        if resolved == variables:
            break
        variables = resolved
    return variables


def load_config(config_path: Path) -> dict:
    """Load config.yaml, resolve variables, and return structured config."""
    if not config_path.exists():
        log.warning("No config.yaml found at %s — using defaults", config_path)
        return DEFAULT_CONFIG.copy()

    try:
        with open(config_path) as f:
            raw = yaml.safe_load(f) or {}
    except Exception:
        log.exception("Failed to parse config.yaml — using defaults")
        return DEFAULT_CONFIG.copy()

    # Extract and self-resolve the vars block
    variables = raw.get("vars", {})
    if not isinstance(variables, dict):
        log.warning("`vars` must be a mapping — ignoring")
        variables = {}
    variables = _resolve_vars_block(variables)

    # Resolve variables across the entire raw config (except vars itself)
    raw.pop("vars", None)
    raw = _resolve(raw, variables)

    # Merge with defaults
    config = DEFAULT_CONFIG.copy()
    config["app"] = {**DEFAULT_CONFIG["app"], **raw.get("app", {})}
    config["plugins_dir"] = raw.get("plugins_dir", DEFAULT_CONFIG["plugins_dir"])
    config["menu"] = raw.get("menu", [])
    config["vars"] = variables  # keep for reference / debugging

    return config

