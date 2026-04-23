"""Tests for docli_companion.report."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from docli_companion.models import (
    CheckResult,
    CompanionReport,
    DocumentIdentity,
    Evidence,
    EvidenceClass,
    ReleaseState,
    ReportSummary,
    Severity,
    ToolVersions,
)
from docli_companion.report import (
    build_report,
    compute_release_state,
    compute_summary,
    report_to_markdown,
    write_artifact_bundle,
)


# ---------------------------------------------------------------------------
# Helpers / fixtures
# ---------------------------------------------------------------------------

def _make_check(
    *,
    check_id: str = "CHK-001",
    name: str = "sample check",
    severity: Severity = Severity.ERROR,
    status: str = "passed",
    message: str = "ok",
    evidence: list[Evidence] | None = None,
    elapsed: float = 0.01,
) -> CheckResult:
    return CheckResult(
        check_id=check_id,
        name=name,
        severity=severity,
        status=status,
        message=message,
        evidence=evidence or [],
        elapsed_seconds=elapsed,
    )


_DOC = DocumentIdentity(
    path="/tmp/report.docx",
    sha256="abc123" * 8,
    size_bytes=123456,
    part_count=12,
)

_TOOLS = ToolVersions(
    companion="0.1.0",
    python="3.12.0",
    docli="0.1.0",
    lxml="5.1.0",
)


# ---------------------------------------------------------------------------
# compute_release_state
# ---------------------------------------------------------------------------


class TestComputeReleaseState:
    def test_all_passed_returns_pass(self) -> None:
        checks = [_make_check(status="passed")]
        assert compute_release_state(checks) == ReleaseState.PASS

    def test_empty_checks_returns_pass(self) -> None:
        assert compute_release_state([]) == ReleaseState.PASS

    def test_error_failure_returns_fail(self) -> None:
        checks = [
            _make_check(status="passed"),
            _make_check(severity=Severity.ERROR, status="failed", message="bad"),
        ]
        assert compute_release_state(checks) == ReleaseState.FAIL

    def test_errored_check_returns_manual_review(self) -> None:
        checks = [_make_check(status="errored")]
        assert compute_release_state(checks) == ReleaseState.MANUAL_REVIEW

    def test_warning_failure_returns_pass_with_warnings(self) -> None:
        checks = [
            _make_check(severity=Severity.WARNING, status="failed", message="meh"),
        ]
        assert compute_release_state(checks) == ReleaseState.PASS_WITH_WARNINGS

    def test_error_failure_takes_precedence_over_errored(self) -> None:
        checks = [
            _make_check(severity=Severity.ERROR, status="failed", message="bad"),
            _make_check(status="errored"),
        ]
        assert compute_release_state(checks) == ReleaseState.FAIL

    def test_errored_takes_precedence_over_warning(self) -> None:
        checks = [
            _make_check(status="errored"),
            _make_check(severity=Severity.WARNING, status="failed"),
        ]
        assert compute_release_state(checks) == ReleaseState.MANUAL_REVIEW


# ---------------------------------------------------------------------------
# compute_summary
# ---------------------------------------------------------------------------


class TestComputeSummary:
    def test_counts_all_statuses(self) -> None:
        checks = [
            _make_check(status="passed"),
            _make_check(status="failed", severity=Severity.ERROR, message="x"),
            _make_check(status="errored"),
            _make_check(status="skipped"),
            _make_check(status="failed", severity=Severity.WARNING, message="w"),
        ]
        s = compute_summary(checks)
        assert s.total_checks == 5
        assert s.passed == 1
        assert s.failed == 2
        assert s.errored == 1
        assert s.skipped == 1
        assert s.warnings == 1

    def test_empty_checks(self) -> None:
        s = compute_summary([])
        assert s == ReportSummary(
            total_checks=0, passed=0, failed=0, errored=0, skipped=0, warnings=0
        )


# ---------------------------------------------------------------------------
# build_report
# ---------------------------------------------------------------------------


class TestBuildReport:
    def test_builds_report_with_correct_state(self) -> None:
        checks = [
            _make_check(status="failed", severity=Severity.ERROR, message="err"),
        ]
        report = build_report(_DOC, _TOOLS, checks, timestamp="2026-01-01T00:00:00Z")
        assert report.release_state == ReleaseState.FAIL
        assert report.summary.failed == 1
        assert report.recommended_action is not None
        assert "failed" in report.recommended_action.lower() or "fix" in report.recommended_action.lower()

    def test_defaults_timestamp(self) -> None:
        report = build_report(_DOC, _TOOLS, [])
        assert report.timestamp  # not None or empty
        assert "T" in report.timestamp  # ISO-8601-ish

    def test_pass_report_action(self) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        assert report.release_state == ReleaseState.PASS
        assert "ready" in report.recommended_action.lower()


# ---------------------------------------------------------------------------
# report_to_markdown
# ---------------------------------------------------------------------------


class TestReportToMarkdown:
    def test_contains_all_sections(self) -> None:
        checks = [
            _make_check(status="passed"),
            _make_check(
                check_id="CHK-002",
                name="policy check",
                severity=Severity.ERROR,
                status="failed",
                message="policy violated",
                evidence=[
                    Evidence(
                        evidence_class=EvidenceClass.POLICY,
                        source="policy-engine",
                        detail="banned term found",
                        location="paragraph 3",
                    )
                ],
            ),
            _make_check(
                check_id="CHK-003",
                name="style warning",
                severity=Severity.WARNING,
                status="failed",
                message="heading style mismatch",
            ),
        ]
        report = build_report(
            _DOC, _TOOLS, checks, elapsed_seconds=1.234, timestamp="2026-01-01T00:00:00Z"
        )
        md = report_to_markdown(report)

        assert "## Document Identity" in md
        assert "## Release State" in md
        assert "## Summary" in md
        assert "## Failed Checks" in md
        assert "## Warnings" in md
        assert "## Recommended Action" in md
        assert "## Tool Versions" in md
        assert "## Timing" in md

    def test_failed_checks_section_lists_errors(self) -> None:
        checks = [
            _make_check(
                check_id="CHK-F1",
                name="broken",
                severity=Severity.ERROR,
                status="failed",
                message="it broke",
            ),
        ]
        report = build_report(_DOC, _TOOLS, checks, timestamp="2026-01-01T00:00:00Z")
        md = report_to_markdown(report)
        assert "CHK-F1" in md
        assert "it broke" in md

    def test_no_failures_says_no_failed_checks(self) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        md = report_to_markdown(report)
        assert "No failed checks." in md
        assert "No warnings." in md

    def test_document_identity_values(self) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        md = report_to_markdown(report)
        assert "/tmp/report.docx" in md
        assert "123,456 bytes" in md

    def test_timing_section(self) -> None:
        report = build_report(
            _DOC, _TOOLS, [], elapsed_seconds=2.5, timestamp="2026-01-01T00:00:00Z"
        )
        md = report_to_markdown(report)
        assert "2.500s" in md

    def test_tool_versions_in_markdown(self) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        md = report_to_markdown(report)
        assert "0.1.0" in md
        assert "3.12.0" in md


# ---------------------------------------------------------------------------
# write_artifact_bundle
# ---------------------------------------------------------------------------


class TestWriteArtifactBundle:
    def test_creates_report_files(self, tmp_path: Path) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        bundle = write_artifact_bundle(report, tmp_path)

        assert Path(bundle.report_json).exists()
        assert Path(bundle.report_md).exists()

        # report.json is valid JSON and round-trips
        data = json.loads(Path(bundle.report_json).read_text())
        assert data["release_state"] == "pass"

        # report.md contains the header
        md_text = Path(bundle.report_md).read_text()
        assert "# Companion Report" in md_text

    def test_creates_output_dir_if_missing(self, tmp_path: Path) -> None:
        out = tmp_path / "nested" / "deep"
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        bundle = write_artifact_bundle(report, out)
        assert out.is_dir()
        assert Path(bundle.report_json).exists()

    def test_writes_sidecar_files(self, tmp_path: Path) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        bundle = write_artifact_bundle(
            report,
            tmp_path,
            inspect_json='{"ok": true}',
            validate_json='{"valid": true}',
        )
        assert bundle.inspect_json is not None
        assert bundle.validate_json is not None
        assert json.loads(Path(bundle.inspect_json).read_text()) == {"ok": True}
        assert json.loads(Path(bundle.validate_json).read_text()) == {"valid": True}

    def test_bundle_without_sidecars(self, tmp_path: Path) -> None:
        report = build_report(_DOC, _TOOLS, [], timestamp="2026-01-01T00:00:00Z")
        bundle = write_artifact_bundle(report, tmp_path)
        assert bundle.inspect_json is None
        assert bundle.validate_json is None
