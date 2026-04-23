"""Subprocess runner for the docli binary.

Discovers the docli binary, invokes subcommands, and parses JSON envelopes.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------

class DocliNotFoundError(FileNotFoundError):
    """Raised when the docli binary cannot be located."""

    def __init__(self, searched: str | None = None) -> None:
        detail = f" (searched: {searched})" if searched else ""
        super().__init__(f"docli binary not found{detail}")


class DocliError(RuntimeError):
    """Raised when docli exits with a non-zero return code."""

    def __init__(
        self,
        args: list[str],
        returncode: int,
        stdout: str,
        stderr: str,
    ) -> None:
        self.args_used = args
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr
        cmd_str = " ".join(args)
        super().__init__(
            f"docli exited {returncode}: {cmd_str}\nstderr: {stderr.strip()}"
        )


class DocliTimeoutError(DocliError):
    """Raised when docli exceeds the specified timeout."""

    def __init__(self, args: list[str], timeout: float) -> None:
        self.timeout = timeout
        cmd_str = " ".join(args)
        # Bypass DocliError.__init__ to set a custom message.
        RuntimeError.__init__(
            self, f"docli timed out after {timeout}s: {cmd_str}"
        )
        self.args_used = args
        self.returncode = -1
        self.stdout = ""
        self.stderr = ""


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

class DocliRunner:
    """Thin wrapper that shells out to the ``docli`` binary.

    Parameters
    ----------
    docli_path:
        Explicit path to the docli binary.  When *None* the runner checks
        the ``DOCLI_PATH`` environment variable and then falls back to
        ``shutil.which("docli")``.
    default_timeout:
        Default timeout in seconds for every invocation.  Can be overridden
        per-call.
    """

    def __init__(
        self,
        docli_path: str | Path | None = None,
        default_timeout: float | None = 30.0,
    ) -> None:
        self._path = self._resolve_path(docli_path)
        self.default_timeout = default_timeout

    # -- discovery -----------------------------------------------------------

    @staticmethod
    def _resolve_path(explicit: str | Path | None) -> str:
        """Return an absolute path to the docli binary."""
        if explicit is not None:
            p = str(explicit)
            if not os.path.isfile(p):
                raise DocliNotFoundError(p)
            return p

        env = os.environ.get("DOCLI_PATH")
        if env and os.path.isfile(env):
            return env

        which = shutil.which("docli")
        if which:
            return which

        raise DocliNotFoundError("DOCLI_PATH env / shutil.which('docli')")

    @property
    def path(self) -> str:
        """Return the resolved path to the docli binary."""
        return self._path

    # -- low-level call ------------------------------------------------------

    def call(
        self,
        args: list[str],
        *,
        timeout: float | None = None,
        check: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        """Run docli with *args* and return the ``CompletedProcess``.

        Parameters
        ----------
        args:
            Arguments to pass after the ``docli`` binary name.
        timeout:
            Timeout in seconds.  Falls back to ``self.default_timeout``.
        check:
            If *True* (default), raise :class:`DocliError` on non-zero exit.
        """
        cmd = [self._path, *args]
        effective_timeout = timeout if timeout is not None else self.default_timeout

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=effective_timeout,
            )
        except subprocess.TimeoutExpired as exc:
            raise DocliTimeoutError(cmd, effective_timeout or 0) from exc

        if check and result.returncode != 0:
            raise DocliError(cmd, result.returncode, result.stdout, result.stderr)

        return result

    # -- JSON envelope helper ------------------------------------------------

    def call_json(
        self,
        args: list[str],
        *,
        timeout: float | None = None,
    ) -> Any:
        """Run docli, parse its stdout as JSON, and return the result.

        Raises :class:`ValueError` if stdout is not valid JSON.
        """
        result = self.call(args, timeout=timeout)
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise ValueError(
                f"docli did not return valid JSON: {exc}"
            ) from exc

    # -- convenience subcommands ---------------------------------------------

    def inspect(
        self,
        doc: str | Path,
        *,
        timeout: float | None = None,
        extra_args: list[str] | None = None,
    ) -> Any:
        """Run ``docli inspect <doc> --format json`` and return parsed JSON."""
        args = ["inspect", str(doc), "--format", "json"]
        if extra_args:
            args.extend(extra_args)
        return self.call_json(args, timeout=timeout)

    def validate(
        self,
        doc: str | Path,
        *,
        timeout: float | None = None,
        extra_args: list[str] | None = None,
    ) -> Any:
        """Run ``docli validate <doc> --format json`` and return parsed JSON."""
        args = ["validate", str(doc), "--format", "json"]
        if extra_args:
            args.extend(extra_args)
        return self.call_json(args, timeout=timeout)

    def read(
        self,
        doc: str | Path,
        *,
        timeout: float | None = None,
        extra_args: list[str] | None = None,
    ) -> Any:
        """Run ``docli read <doc> --format json`` and return parsed JSON."""
        args = ["read", str(doc), "--format", "json"]
        if extra_args:
            args.extend(extra_args)
        return self.call_json(args, timeout=timeout)

    def doctor(
        self,
        *,
        timeout: float | None = None,
        extra_args: list[str] | None = None,
    ) -> Any:
        """Run ``docli doctor --format json`` and return parsed JSON."""
        args = ["doctor", "--format", "json"]
        if extra_args:
            args.extend(extra_args)
        return self.call_json(args, timeout=timeout)

    def run(
        self,
        job_file: str | Path,
        *,
        timeout: float | None = None,
        extra_args: list[str] | None = None,
    ) -> Any:
        """Run ``docli run <job_file> --format json`` and return parsed JSON."""
        args = ["run", str(job_file), "--format", "json"]
        if extra_args:
            args.extend(extra_args)
        return self.call_json(args, timeout=timeout)
