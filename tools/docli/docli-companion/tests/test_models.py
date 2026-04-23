"""Tests for docli_companion.models — Pydantic v2 data models."""

from __future__ import annotations

import json

import pytest
from pydantic import ValidationError

from docli_companion.models import (
    ArtifactBundle,
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


# ---------------------------------------------------------------------------
# Helpers / fixtures
# ---------------------------------------------------------------------------

def _make_document_identity() -> DocumentIdentity:
    return DocumentIdentity(
        path="/tmp/test.docx",
        sha256="abc123",
        size_bytes=1024,
        part_count=3,
    )


def _make_tool_versions() -> ToolVersions:
    return ToolVersions(companion="0.1.0", python="3.12.0")


def _make_report_summary() -> ReportSummary:
    return ReportSummary(
        total_checks=5,
        passed=3,
        failed=1,
        errored=0,
        skipped=1,
        warnings=2,
    )


def _make_evidence() -> Evidence:
    return Evidence(
        evidence_class=EvidenceClass.STRUCTURAL,
        source="structural.required-parts",
        detail="Missing TOC",
    )


def _make_check_result() -> CheckResult:
    return CheckResult(
        check_id="structural.required-parts",
        name="Required parts",
        severity=Severity.ERROR,
        status="failed",
        message="Document is missing table of contents",
        evidence=[_make_evidence()],
    )


def _make_companion_report() -> CompanionReport:
    return CompanionReport(
        timestamp="2026-03-15T12:00:00Z",
        document=_make_document_identity(),
        tool_versions=_make_tool_versions(),
        release_state=ReleaseState.FAIL,
        checks=[_make_check_result()],
        summary=_make_report_summary(),
    )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

class TestReleaseStateEnum:
    def test_values(self):
        assert ReleaseState.PASS.value == "pass"
        assert ReleaseState.PASS_WITH_WARNINGS.value == "pass-with-warnings"
        assert ReleaseState.MANUAL_REVIEW.value == "manual-review"
        assert ReleaseState.FAIL.value == "fail"

    def test_membership(self):
        assert len(ReleaseState) == 4

    def test_string_coercion(self):
        assert str(ReleaseState.PASS) == "ReleaseState.PASS" or ReleaseState("pass") == ReleaseState.PASS


class TestSeverityEnum:
    def test_values(self):
        assert Severity.ERROR.value == "error"
        assert Severity.WARNING.value == "warning"
        assert Severity.INFO.value == "info"


class TestEvidence:
    def test_construct_without_location(self):
        ev = _make_evidence()
        assert ev.location is None
        assert ev.source == "structural.required-parts"

    def test_construct_with_location(self):
        ev = Evidence(
            evidence_class=EvidenceClass.RENDERING,
            source="render-check",
            detail="Page break issue",
            location="paragraph 3",
        )
        assert ev.location == "paragraph 3"

    def test_required_fields_missing(self):
        with pytest.raises(ValidationError):
            Evidence()  # type: ignore[call-arg]

    def test_json_round_trip(self):
        ev = Evidence(
            evidence_class=EvidenceClass.OCR,
            source="ocr-engine",
            detail="Low confidence",
            location="page 2",
        )
        json_str = ev.model_dump_json()
        restored = Evidence.model_validate_json(json_str)
        assert restored == ev


class TestCheckResult:
    def test_construct(self):
        cr = _make_check_result()
        assert cr.check_id == "structural.required-parts"
        assert cr.status == "failed"
        assert len(cr.evidence) == 1

    def test_invalid_status_rejected(self):
        with pytest.raises(ValidationError):
            CheckResult(
                check_id="x",
                name="x",
                severity=Severity.INFO,
                status="unknown",  # type: ignore[arg-type]
                message="bad",
            )

    def test_defaults(self):
        cr = CheckResult(
            check_id="a",
            name="a",
            severity=Severity.INFO,
            status="passed",
            message="ok",
        )
        assert cr.evidence == []
        assert cr.elapsed_seconds == 0.0


class TestDocumentIdentity:
    def test_construct(self):
        di = _make_document_identity()
        assert di.size_bytes == 1024

    def test_required_fields(self):
        with pytest.raises(ValidationError):
            DocumentIdentity(path="/tmp/x.docx")  # type: ignore[call-arg]


class TestToolVersions:
    def test_minimal(self):
        tv = ToolVersions(companion="1.0", python="3.12")
        assert tv.docli is None
        assert tv.lxml is None

    def test_full(self):
        tv = ToolVersions(companion="1.0", python="3.12", docli="0.5", lxml="5.1")
        assert tv.docli == "0.5"


class TestReportSummary:
    def test_construct(self):
        s = _make_report_summary()
        assert s.total_checks == 5
        assert s.warnings == 2


class TestCompanionReport:
    def test_construct(self):
        report = _make_companion_report()
        assert report.version == "1"
        assert report.release_state == ReleaseState.FAIL

    def test_empty_checks(self):
        report = CompanionReport(
            timestamp="2026-03-15T12:00:00Z",
            document=_make_document_identity(),
            tool_versions=_make_tool_versions(),
            release_state=ReleaseState.PASS,
            summary=_make_report_summary(),
        )
        assert report.checks == []

    def test_json_round_trip(self):
        report = _make_companion_report()
        json_str = report.model_dump_json()
        restored = CompanionReport.model_validate_json(json_str)
        assert restored == report

    def test_model_dump_clean_json(self):
        report = _make_companion_report()
        data = report.model_dump()
        # Ensure it's JSON-serializable (no non-serializable types)
        json_str = json.dumps(data)
        assert isinstance(json_str, str)
        parsed = json.loads(json_str)
        assert parsed["release_state"] == "fail"
        assert parsed["document"]["path"] == "/tmp/test.docx"

    def test_required_fields_missing(self):
        with pytest.raises(ValidationError):
            CompanionReport()  # type: ignore[call-arg]


class TestArtifactBundle:
    def test_minimal(self):
        ab = ArtifactBundle(report_json="/tmp/report.json")
        assert ab.report_md is None
        assert ab.page_images == []

    def test_full(self):
        ab = ArtifactBundle(
            report_json="/tmp/report.json",
            report_md="/tmp/report.md",
            inspect_json="/tmp/inspect.json",
            validate_json="/tmp/validate.json",
            semantic_html="/tmp/semantic.html",
            render_pdf="/tmp/render.pdf",
            page_images=["/tmp/page1.png", "/tmp/page2.png"],
        )
        assert len(ab.page_images) == 2

    def test_json_round_trip(self):
        ab = ArtifactBundle(
            report_json="/tmp/report.json",
            page_images=["/tmp/p1.png"],
        )
        restored = ArtifactBundle.model_validate_json(ab.model_dump_json())
        assert restored == ab
