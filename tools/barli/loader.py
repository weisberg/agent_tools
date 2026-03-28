"""
Plugin loader — discovers decorated functions in .py files,
imports them, and supports hot-reload on file changes.
"""

import importlib
import importlib.util
import inspect
import logging
import sys
from pathlib import Path

from actions import clear_module, register_function

log = logging.getLogger("menubar.loader")

# Track whether we've added the project root to sys.path
_path_initialized = False


def _ensure_plugin_path(project_root: Path):
    """
    Add the project root to sys.path exactly once so plugins can do:
        from actions import menu_action
    without any sys.path hacks of their own.
    """
    global _path_initialized
    if _path_initialized:
        return

    root_str = str(project_root.resolve())
    if root_str not in sys.path:
        sys.path.insert(0, root_str)
        log.info("Added to sys.path: %s", root_str)
    _path_initialized = True


def load_module(filepath: Path):
    """
    (Re-)load a single plugin file and register its @menu_action functions.

    Steps:
      1. Clear any previously registered actions from this file.
      2. Import (or reload) the module.
      3. Walk the module's attributes looking for the _menu_action_name
         sentinel that @menu_action attaches.
      4. Register each discovered function.
    """
    module_key = str(filepath.resolve())
    clear_module(module_key)

    module_name = f"_plugin_{filepath.stem}"

    try:
        # If already imported, reload; otherwise import fresh
        if module_name in sys.modules:
            module = importlib.reload(sys.modules[module_name])
        else:
            spec = importlib.util.spec_from_file_location(module_name, filepath)
            if spec is None or spec.loader is None:
                log.warning("Could not create spec for %s", filepath)
                return
            module = importlib.util.module_from_spec(spec)
            sys.modules[module_name] = module
            spec.loader.exec_module(module)

        # Discover decorated functions
        count = 0
        for attr_name, obj in inspect.getmembers(module, inspect.isfunction):
            action_name = getattr(obj, "_menu_action_name", None)
            if action_name:
                register_function(module_key, action_name, obj)
                count += 1
                log.info("  Registered action '%s' from %s", action_name, filepath.name)

        log.info("Loaded %s — %d action(s)", filepath.name, count)

    except Exception:
        log.exception("Failed to load plugin %s", filepath)


def unload_module(filepath: Path):
    """Remove a plugin's actions when its file is deleted."""
    module_key = str(filepath.resolve())
    clear_module(module_key)

    module_name = f"_plugin_{filepath.stem}"
    sys.modules.pop(module_name, None)
    log.info("Unloaded %s", filepath.name)


def load_all(plugins_dir: Path, project_root: Path | None = None):
    """
    Initial scan — load every .py file in the plugins directory.

    Args:
        plugins_dir: Directory to scan.
        project_root: Project root to add to sys.path so plugins can
                      import `actions`. Defaults to plugins_dir parent.
    """
    if project_root is None:
        project_root = plugins_dir.parent
    _ensure_plugin_path(project_root)

    if not plugins_dir.is_dir():
        log.warning("Plugins directory does not exist: %s", plugins_dir)
        return
    for py_file in sorted(plugins_dir.glob("*.py")):
        if py_file.name.startswith("_"):
            continue
        load_module(py_file)
