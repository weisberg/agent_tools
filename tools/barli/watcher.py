"""
File watcher — monitors the plugins directory and config.yaml
for changes and triggers hot-reload via a callback.

Key design decisions:
  - Only .py files inside the plugins_dir are treated as plugins.
    Other .py files (app.py, config.py, etc.) are ignored.
  - File changes are debounced: rapid events within DEBOUNCE_SECONDS
    are collapsed into a single reload.
  - File change details are queued so the main thread can process them.
"""

import logging
import threading
import time
from pathlib import Path

from watchdog.events import FileSystemEventHandler, FileSystemEvent
from watchdog.observers import Observer

log = logging.getLogger("menubar.watcher")

DEBOUNCE_SECONDS = 0.5


class PluginEventHandler(FileSystemEventHandler):
    """React to plugin .py and config.yaml changes."""

    def __init__(self, plugins_dir: Path, config_filename: str, on_change_callback):
        """
        Args:
            plugins_dir: Resolved path to the plugins directory.
                Only .py files here are treated as plugins.
            config_filename: The config file's name (e.g. "config.yaml").
            on_change_callback: Called with a list of (event_type, Path) tuples
                after the debounce window closes.
        """
        super().__init__()
        self._plugins_dir = plugins_dir.resolve()
        self._config_filename = config_filename
        self._on_change = on_change_callback

        # Debounce state
        self._lock = threading.Lock()
        self._pending_changes: list[tuple[str, Path]] = []
        self._debounce_timer: threading.Timer | None = None

    # -- helpers ----------------------------------------------------------

    def _is_plugin(self, path_str: str) -> bool:
        """True only for .py files directly inside the plugins directory."""
        p = Path(path_str).resolve()
        return (
            p.suffix == ".py"
            and not p.name.startswith("_")
            and p.parent == self._plugins_dir
        )

    def _is_config(self, path_str: str) -> bool:
        return Path(path_str).name == self._config_filename

    def _queue_change(self, event_type: str, filepath: Path):
        """Queue a change and (re)start the debounce timer."""
        with self._lock:
            self._pending_changes.append((event_type, filepath))

            # Cancel any pending timer and start a fresh one
            if self._debounce_timer is not None:
                self._debounce_timer.cancel()

            self._debounce_timer = threading.Timer(
                DEBOUNCE_SECONDS, self._flush_changes
            )
            self._debounce_timer.daemon = True
            self._debounce_timer.start()

    def _flush_changes(self):
        """Drain the queue and notify the app."""
        with self._lock:
            changes = self._pending_changes.copy()
            self._pending_changes.clear()
            self._debounce_timer = None

        if changes:
            log.info("Flushing %d queued change(s)", len(changes))
            self._on_change(changes)

    # -- watchdog callbacks -----------------------------------------------

    def on_created(self, event: FileSystemEvent):
        if event.is_directory:
            return
        if self._is_plugin(event.src_path):
            log.info("New plugin detected: %s", event.src_path)
            self._queue_change("created", Path(event.src_path))
        elif self._is_config(event.src_path):
            self._queue_change("config_changed", Path(event.src_path))

    def on_modified(self, event: FileSystemEvent):
        if event.is_directory:
            return
        if self._is_plugin(event.src_path):
            log.info("Plugin modified: %s", event.src_path)
            self._queue_change("modified", Path(event.src_path))
        elif self._is_config(event.src_path):
            log.info("Config changed")
            self._queue_change("config_changed", Path(event.src_path))

    def on_deleted(self, event: FileSystemEvent):
        if event.is_directory:
            return
        if self._is_plugin(event.src_path):
            log.info("Plugin removed: %s", event.src_path)
            self._queue_change("deleted", Path(event.src_path))
        elif self._is_config(event.src_path):
            self._queue_change("config_changed", Path(event.src_path))

    def on_moved(self, event):
        if event.is_directory:
            return
        if self._is_plugin(event.src_path):
            self._queue_change("deleted", Path(event.src_path))
        if hasattr(event, "dest_path") and self._is_plugin(event.dest_path):
            self._queue_change("created", Path(event.dest_path))


def start_watcher(
    plugins_dir: Path,
    config_path: Path,
    on_change_callback,
) -> Observer:
    """
    Start a watchdog observer on the plugins dir and config dir.

    Args:
        plugins_dir: Directory containing plugin .py files.
        config_path: Full path to config.yaml.
        on_change_callback: Called with list of (event_type, path) tuples.

    Returns the Observer so the caller can stop it on exit.
    """
    handler = PluginEventHandler(
        plugins_dir=plugins_dir,
        config_filename=config_path.name,
        on_change_callback=on_change_callback,
    )
    observer = Observer()

    # Watch plugins directory
    plugins_str = str(plugins_dir.resolve())
    observer.schedule(handler, plugins_str, recursive=False)
    log.info("Watching plugins: %s", plugins_str)

    # Watch config directory (may be the same as plugins parent)
    config_dir_str = str(config_path.parent.resolve())
    if config_dir_str != plugins_str:
        observer.schedule(handler, config_dir_str, recursive=False)
        log.info("Watching config dir: %s", config_dir_str)

    observer.daemon = True
    observer.start()
    return observer
