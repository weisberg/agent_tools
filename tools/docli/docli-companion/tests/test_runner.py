"""Tests for docli_companion.runner."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from unittest import mock

import pytest

from docli_companion.runner import (
    DocliError,
    DocliNotFoundError,
    DocliRunner,
    DocliTimeoutError,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

FAKE_DOCLI = "/usr/local/bin/docli"


def _make_completed(
    stdout: str = "",
    stderr: str = "",
    returncode: int = 0,
) -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(
        args=["docli"],
        returncode=returncode,
        stdout=stdout,
        stderr=stderr,
    )


def _runner() -> DocliRunner:
    """Return a runner that skips real binary discovery."""
    with mock.patch.object(DocliRunner, "_resolve_path", return_value=FAKE_DOCLI):
        return DocliRunner()


# ---------------------------------------------------------------------------
# Discovery tests
# ---------------------------------------------------------------------------

class TestDiscovery:
    """Tests for binary discovery logic."""

    def test_explicit_path_found(self, tmp_path: Path) -> None:
        fake = tmp_path / "docli"
        fake.touch()
        runner = DocliRunner(docli_path=fake)
        assert runner.path == str(fake)

    def test_explicit_path_missing_raises(self) -> None:
        with pytest.raises(DocliNotFoundError):
            DocliRunner(docli_path="/no/such/binary")

    def test_env_var_takes_precedence(self, tmp_path: Path) -> None:
        fake = tmp_path / "docli"
        fake.touch()
        with mock.patch.dict("os.environ", {"DOCLI_PATH": str(fake)}):
            runner = DocliRunner()
            assert runner.path == str(fake)

    def test_shutil_which_fallback(self, tmp_path: Path) -> None:
        fake = tmp_path / "docli"
        fake.touch()
        with (
            mock.patch.dict("os.environ", {}, clear=True),
            mock.patch("docli_companion.runner.shutil.which", return_value=str(fake)),
        ):
            runner = DocliRunner()
            assert runner.path == str(fake)

    def test_nothing_found_raises(self) -> None:
        with (
            mock.patch.dict("os.environ", {}, clear=True),
            mock.patch("docli_companion.runner.shutil.which", return_value=None),
        ):
            with pytest.raises(DocliNotFoundError):
                DocliRunner()


# ---------------------------------------------------------------------------
# call / call_json tests
# ---------------------------------------------------------------------------

class TestCall:
    """Tests for the low-level call and call_json methods."""

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_call_returns_completed_process(self, mock_run: mock.Mock) -> None:
        mock_run.return_value = _make_completed(stdout="ok\n")
        runner = _runner()
        result = runner.call(["doctor"])
        mock_run.assert_called_once()
        assert result.stdout == "ok\n"

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_call_raises_on_nonzero_exit(self, mock_run: mock.Mock) -> None:
        mock_run.return_value = _make_completed(returncode=1, stderr="boom")
        runner = _runner()
        with pytest.raises(DocliError) as exc_info:
            runner.call(["validate", "bad.docx"])
        assert exc_info.value.returncode == 1
        assert "boom" in str(exc_info.value)

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_call_no_check_allows_nonzero(self, mock_run: mock.Mock) -> None:
        mock_run.return_value = _make_completed(returncode=2, stderr="warn")
        runner = _runner()
        result = runner.call(["validate", "x.docx"], check=False)
        assert result.returncode == 2

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_call_json_parses_stdout(self, mock_run: mock.Mock) -> None:
        payload = {"status": "ok", "findings": []}
        mock_run.return_value = _make_completed(stdout=json.dumps(payload))
        runner = _runner()
        data = runner.call_json(["inspect", "doc.docx", "--format", "json"])
        assert data == payload

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_call_json_raises_on_bad_json(self, mock_run: mock.Mock) -> None:
        mock_run.return_value = _make_completed(stdout="not json at all")
        runner = _runner()
        with pytest.raises(ValueError, match="valid JSON"):
            runner.call_json(["inspect", "doc.docx"])

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_timeout_raises_docli_timeout_error(self, mock_run: mock.Mock) -> None:
        mock_run.side_effect = subprocess.TimeoutExpired(cmd=["docli"], timeout=5)
        runner = _runner()
        with pytest.raises(DocliTimeoutError) as exc_info:
            runner.call(["run", "big-job.yaml"], timeout=5)
        assert exc_info.value.timeout == 5


# ---------------------------------------------------------------------------
# Convenience subcommand tests
# ---------------------------------------------------------------------------

class TestSubcommands:
    """Tests for inspect, validate, read, doctor, run helpers."""

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_inspect_builds_correct_args(self, mock_run: mock.Mock) -> None:
        payload = {"headings": ["Intro"]}
        mock_run.return_value = _make_completed(stdout=json.dumps(payload))
        runner = _runner()
        data = runner.inspect("report.docx")
        cmd = mock_run.call_args[0][0]
        assert cmd == [FAKE_DOCLI, "inspect", "report.docx", "--format", "json"]
        assert data == payload

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_validate_builds_correct_args(self, mock_run: mock.Mock) -> None:
        payload = {"valid": True}
        mock_run.return_value = _make_completed(stdout=json.dumps(payload))
        runner = _runner()
        data = runner.validate("/tmp/doc.docx")
        cmd = mock_run.call_args[0][0]
        assert cmd == [FAKE_DOCLI, "validate", "/tmp/doc.docx", "--format", "json"]
        assert data == payload

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_read_builds_correct_args(self, mock_run: mock.Mock) -> None:
        payload = {"text": "Hello"}
        mock_run.return_value = _make_completed(stdout=json.dumps(payload))
        runner = _runner()
        data = runner.read("readme.docx")
        cmd = mock_run.call_args[0][0]
        assert cmd == [FAKE_DOCLI, "read", "readme.docx", "--format", "json"]
        assert data == payload

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_doctor_builds_correct_args(self, mock_run: mock.Mock) -> None:
        payload = {"ok": True}
        mock_run.return_value = _make_completed(stdout=json.dumps(payload))
        runner = _runner()
        data = runner.doctor()
        cmd = mock_run.call_args[0][0]
        assert cmd == [FAKE_DOCLI, "doctor", "--format", "json"]
        assert data == payload

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_run_builds_correct_args(self, mock_run: mock.Mock) -> None:
        payload = {"completed": 3}
        mock_run.return_value = _make_completed(stdout=json.dumps(payload))
        runner = _runner()
        data = runner.run("job.yaml", extra_args=["--dry-run"])
        cmd = mock_run.call_args[0][0]
        assert cmd == [
            FAKE_DOCLI, "run", "job.yaml", "--format", "json", "--dry-run",
        ]
        assert data == payload

    @mock.patch("docli_companion.runner.subprocess.run")
    def test_extra_args_appended(self, mock_run: mock.Mock) -> None:
        mock_run.return_value = _make_completed(stdout='{"ok":true}')
        runner = _runner()
        runner.inspect("x.docx", extra_args=["--verbose", "--depth=2"])
        cmd = mock_run.call_args[0][0]
        assert "--verbose" in cmd
        assert "--depth=2" in cmd
