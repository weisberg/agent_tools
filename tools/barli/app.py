#!/usr/bin/env python3
"""
Plugin Menu Bar — a macOS menu bar app that discovers Python functions
via decorators, hot-reloads on file changes, and builds the menu
from config.yaml.

Usage:
    python app.py                      # uses ./config.yaml + ./plugins/
    python app.py --config /path/to/config.yaml
"""

import argparse
import logging
import threading
import traceback
from pathlib import Path

import rumps

from actions import get_all_actions
from config import load_config
from loader import load_all, load_module, unload_module
from watcher import start_watcher

VERSION = "1.0"

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
    datefmt="%H:%M:%S",
)
log = logging.getLogger("menubar.app")


class PluginMenuBarApp(rumps.App):
    """
    The menu bar application.

    On startup it:
      1. Loads config.yaml
      2. Scans the plugins directory for decorated functions
      3. Builds the menu based on config ordering
      4. Starts a file watcher for hot-reload
    """

    def __init__(self, config_path: Path):
        self.config_path = config_path.resolve()
        self.base_dir = self.config_path.parent
        self._cfg = load_config(self.config_path)
        self._cfg.setdefault("vars", {})["barli_version"] = VERSION

        plugins_dir_str = self._cfg["plugins_dir"]
        self.plugins_dir = (
            Path(plugins_dir_str)
            if Path(plugins_dir_str).is_absolute()
            else self.base_dir / plugins_dir_str
        )

        app_cfg = self._cfg["app"]
        super().__init__(
            name=app_cfg.get("tooltip", "Plugin Menu Bar"),
            title=app_cfg.get("title", "⚡"),
            quit_button=None,  # we add our own at the bottom
        )

        if app_cfg.get("icon"):
            icon_path = self.base_dir / app_cfg["icon"]
            if icon_path.exists():
                self.icon = str(icon_path)

        # Initial plugin load (also sets up sys.path for plugins)
        self.plugins_dir.mkdir(parents=True, exist_ok=True)
        load_all(self.plugins_dir, project_root=self.base_dir)
        self._build_menu()

        # Start file watcher
        self._observer = start_watcher(
            plugins_dir=self.plugins_dir,
            config_path=self.config_path,
            on_change_callback=self._on_changes_debounced,
        )

    # -----------------------------------------------------------------
    # Menu building
    # -----------------------------------------------------------------

    def _build_menu(self):
        """
        Rebuild the entire menu from config.yaml + registered actions.

        Config menu items look like:
            - label: "Deploy Staging"
              action: deploy
              value: staging
              icon: deploy.png       # optional

            - separator: true

            - label: "Utilities"
              submenu:
                - label: "Ping"
                  action: ping
                  value: google.com
        """
        actions = get_all_actions()
        items = []

        for entry in self._cfg.get("menu", []):
            item = self._make_menu_item(entry, actions)
            if item is not None:
                items.append(item)

        # Show unregistered actions that aren't in the config
        configured_names = self._collect_configured_actions(self._cfg.get("menu", []))
        unconfigured = {k: v for k, v in actions.items() if k not in configured_names}
        if unconfigured:
            items.append(None)  # separator
            for name, func in sorted(unconfigured.items()):
                mi = rumps.MenuItem(f"[auto] {name}", callback=self._make_callback(func, None))
                items.append(mi)

        # Always end with separator + Reload + Quit
        items.append(None)
        items.append(rumps.MenuItem("↻ Reload", callback=self._manual_reload))
        items.append(rumps.MenuItem("Quit", callback=self._on_quit))

        self.menu.clear()
        for item in items:
            self.menu.add(item if item is not None else rumps.separator)

        log.info("Menu rebuilt — %d items", len(items))

    def _make_menu_item(self, entry: dict, actions: dict):
        """Recursively build a MenuItem (or separator) from a config entry."""
        if entry.get("separator"):
            return None  # rumps separator

        label = entry.get("label", "???")

        # Submenu
        if "submenu" in entry:
            parent = rumps.MenuItem(label)
            for child_entry in entry["submenu"]:
                child = self._make_menu_item(child_entry, actions)
                if child is None:
                    parent.add(rumps.separator)
                else:
                    parent.add(child)
            return parent

        # Leaf item
        action_name = entry.get("action")
        value = entry.get("value")
        func = actions.get(action_name) if action_name else None

        if func is None and action_name:
            label = f"{label} (not loaded)"

        callback = self._make_callback(func, value) if action_name else None
        mi = rumps.MenuItem(label, callback=callback)
        mi.state = 0

        if entry.get("icon"):
            icon_path = self.base_dir / entry["icon"]
            if icon_path.exists():
                mi.icon = str(icon_path)

        return mi

    def _collect_configured_actions(self, entries: list) -> set:
        """Recurse config to find all referenced action names."""
        names = set()
        for entry in entries:
            if entry.get("action"):
                names.add(entry["action"])
            if "submenu" in entry:
                names |= self._collect_configured_actions(entry["submenu"])
        return names

    # -----------------------------------------------------------------
    # Callbacks
    # -----------------------------------------------------------------

    @staticmethod
    def _make_callback(func, value):
        """
        Wrap a plugin function call so rumps can use it as a callback.

        Dispatches based on the type of `value` from config.yaml:
          - dict  → func(**value)     — named keyword arguments
          - list  → func(*value)      — positional arguments
          - None  → func()            — no arguments
          - other → func(value)       — single positional argument

        Runs in a background thread so long-running actions
        don't freeze the menu bar.
        """
        if func is None:
            def _noop(_):
                rumps.notification(
                    title="Plugin Menu Bar",
                    subtitle="Action not available",
                    message="The plugin for this action is not loaded.",
                )
            return _noop

        def _invoke():
            try:
                if isinstance(value, dict):
                    func(**value)
                elif isinstance(value, list):
                    func(*value)
                elif value is None:
                    func()
                else:
                    func(value)
            except Exception:
                tb = traceback.format_exc()
                log.error("Action failed:\n%s", tb)
                rumps.notification(
                    title="Plugin Error",
                    subtitle="Action raised an exception",
                    message=tb[:200],
                )

        def _cb(_):
            t = threading.Thread(target=_invoke, daemon=True)
            t.start()

        return _cb

    # -----------------------------------------------------------------
    # Hot-reload
    # -----------------------------------------------------------------

    def _on_changes_debounced(self, changes: list[tuple[str, Path]]):
        """
        Called by the watcher AFTER debounce, from a background timer thread.

        We schedule the actual work on the main thread via rumps.Timer.
        """
        # Use a one-shot timer to bounce to the main (AppKit) thread.
        def _process(_timer):
            _timer.stop()
            self._apply_changes(changes)

        rumps.Timer(_process, 0.05).start()

    def _apply_changes(self, changes: list[tuple[str, Path]]):
        """
        Process queued file changes on the main thread.
        Loads/unloads plugin modules, reloads config, then rebuilds the menu.
        """
        config_changed = False

        for event_type, filepath in changes:
            if event_type == "config_changed":
                config_changed = True
            elif event_type == "deleted":
                unload_module(filepath)
            elif event_type in ("created", "modified"):
                load_module(filepath)

        if config_changed:
            self._cfg = load_config(self.config_path)
            self._cfg.setdefault("vars", {})["barli_version"] = VERSION

        self._build_menu()

    def _manual_reload(self, _):
        """Menu item: force full reload."""
        log.info("Manual reload triggered")
        self._cfg = load_config(self.config_path)
        self._cfg.setdefault("vars", {})["barli_version"] = VERSION
        load_all(self.plugins_dir, project_root=self.base_dir)
        self._build_menu()
        rumps.notification("Plugin Menu Bar", "Reloaded", "All plugins reloaded.")

    def _on_quit(self, _):
        """Clean up the file watcher before quitting."""
        log.info("Shutting down")
        if self._observer:
            self._observer.stop()
            self._observer.join(timeout=2)
        rumps.quit_application()


# -----------------------------------------------------------------
# Entry point
# -----------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Plugin Menu Bar")
    parser.add_argument(
        "--config", "-c",
        type=Path,
        default=Path(__file__).parent / "config.yaml",
        help="Path to config.yaml (default: ./config.yaml)",
    )
    args = parser.parse_args()

    app = PluginMenuBarApp(config_path=args.config)
    app.run()


if __name__ == "__main__":
    main()
