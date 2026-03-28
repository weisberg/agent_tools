"""
system_info.py — Plugin with system utility actions.

Shows how to add new plugins: just drop a .py file in plugins/,
decorate functions with @menu_action, and reference them in config.yaml.
The app hot-reloads automatically.
"""

import subprocess
from pathlib import Path

from actions import menu_action


@menu_action(name="system_info")
def system_info():
    """Show system information."""
    import platform
    import rumps

    info = (
        f"OS: {platform.system()} {platform.release()}\n"
        f"Python: {platform.python_version()}\n"
        f"Machine: {platform.machine()}"
    )
    rumps.notification("System Info", "", info)


@menu_action(name="disk_usage")
def disk_usage(path=None):
    """Show disk usage for a given path (defaults to ~)."""
    import shutil
    import rumps

    path = str(path) if path else str(Path.home())
    usage = shutil.disk_usage(path)
    free_gb = usage.free / (1024 ** 3)
    total_gb = usage.total / (1024 ** 3)
    rumps.notification(
        "Disk Usage",
        path,
        f"{free_gb:.1f} GB free of {total_gb:.1f} GB",
    )


@menu_action(name="ip_address")
def ip_address():
    """Copy the local IP address to clipboard."""
    import socket
    import rumps

    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(("8.8.8.8", 80))
        ip = s.getsockname()[0]
        s.close()
    except Exception:
        ip = "Could not determine IP"

    subprocess.run(["pbcopy"], input=ip.encode(), check=True)
    rumps.notification("IP Address", "Copied to clipboard", ip)
