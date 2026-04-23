"""Wrapper for invoking the xli CLI binary from Python."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any


class XliError(Exception):
    """Raised when an xli command returns an error envelope."""

    def __init__(self, envelope: dict[str, Any]) -> None:
        self.envelope = envelope
        errors = envelope.get("errors", [])
        msg = errors[0].get("message", errors[0].get("code", "unknown")) if errors else "unknown"
        super().__init__(f"xli error: {msg}")


def run(args: list[str], *, cwd: Path | None = None) -> dict[str, Any]:
    """Run an xli command and return the parsed JSON envelope."""
    result = subprocess.run(
        ["xli", *args],
        capture_output=True,
        text=True,
        cwd=cwd,
    )
    envelope = json.loads(result.stdout)
    if envelope.get("status") == "error":
        raise XliError(envelope)
    return envelope


def inspect(path: Path) -> dict[str, Any]:
    """Run xli inspect and return the output."""
    return run(["inspect", str(path)])


def read_range(path: Path, range_ref: str, *, headers: bool = False) -> dict[str, Any]:
    """Run xli read for a range."""
    args = ["read", str(path), range_ref]
    if headers:
        args.append("--headers")
    return run(args)


def read_cell(path: Path, address: str) -> dict[str, Any]:
    """Run xli read for a single cell."""
    return run(["read", str(path), address])


def doctor(path: Path) -> dict[str, Any]:
    """Run xli doctor and return the output."""
    return run(["doctor", str(path)])


def batch(path: Path, ops_ndjson: str, *, expect_fingerprint: str | None = None) -> dict[str, Any]:
    """Run xli batch with ndjson ops via stdin."""
    args = ["batch", str(path), "--stdin"]
    if expect_fingerprint:
        args.extend(["--expect-fingerprint", expect_fingerprint])
    result = subprocess.run(
        ["xli", *args],
        input=ops_ndjson,
        capture_output=True,
        text=True,
    )
    envelope = json.loads(result.stdout)
    if envelope.get("status") == "error":
        raise XliError(envelope)
    return envelope
