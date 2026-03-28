"""
Action registry — decorator and storage for menu bar plugin functions.

Plugin authors decorate functions with @menu_action to make them
discoverable by the menu bar app.

Usage (in a plugin file):
    from actions import menu_action

    @menu_action(name="greet")
    def greet(message):
        print(f"Hello, {message}!")
"""

import logging
import threading

log = logging.getLogger("menubar.actions")

# Thread lock — registry is written from the watchdog thread
# (via loader) and read from the main thread (via get_all_actions).
_lock = threading.Lock()

# Global registry: module_path -> {action_name: callable}
_registry: dict[str, dict[str, callable]] = {}


def menu_action(name: str):
    """
    Register a function as a menu bar action.

    Args:
        name: The internal action name used to reference this function
              in config.yaml. Must be unique across all plugins.
    """
    def wrapper(func):
        func._menu_action_name = name
        return func
    return wrapper


def get_registry() -> dict[str, dict]:
    with _lock:
        return {k: dict(v) for k, v in _registry.items()}


def clear_module(module_path: str):
    """Remove all registered actions from a specific module."""
    with _lock:
        _registry.pop(module_path, None)


def register_function(module_path: str, action_name: str, func):
    """Register a discovered function, warning on duplicates."""
    with _lock:
        # Check for duplicate action names in OTHER modules
        for other_path, funcs in _registry.items():
            if other_path != module_path and action_name in funcs:
                log.warning(
                    "Duplicate action name '%s' — %s overwrites %s",
                    action_name,
                    module_path,
                    other_path,
                )
                del funcs[action_name]

        if module_path not in _registry:
            _registry[module_path] = {}
        _registry[module_path][action_name] = func


def get_all_actions() -> dict[str, callable]:
    """Flatten registry into {action_name: callable}."""
    with _lock:
        actions = {}
        for funcs in _registry.values():
            actions.update(funcs)
        return actions
