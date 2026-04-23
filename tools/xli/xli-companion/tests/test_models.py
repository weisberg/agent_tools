"""Tests for pydantic models."""

from xli_companion.models import (
    CompanionResult,
    Finding,
    FixOp,
    PlatformInfo,
    Severity,
    Summary,
)


def test_finding_serialization():
    f = Finding(
        code="TEST_CODE",
        severity=Severity.WARNING,
        sheet="Sheet1",
        message="Test message",
    )
    data = f.model_dump()
    assert data["code"] == "TEST_CODE"
    assert data["severity"] == "warning"
    assert data["sheet"] == "Sheet1"


def test_companion_result_defaults():
    result = CompanionResult(workbook="test.xlsx")
    assert result.status == "ok"
    assert result.summary.checks_run == 0
    assert result.findings == []
    assert result.fix_plan == []
    assert result.platform.python
    assert result.platform.os


def test_companion_result_json_roundtrip():
    result = CompanionResult(
        workbook="model.xlsx",
        validated_fingerprint="sha256:abc123",
        summary=Summary(checks_run=5, errors=1, warnings=2),
        findings=[
            Finding(
                code="MISSING_SHEET",
                severity=Severity.ERROR,
                sheet="Summary",
                message="Required sheet missing",
            )
        ],
    )
    json_str = result.model_dump_json()
    restored = CompanionResult.model_validate_json(json_str)
    assert restored.workbook == "model.xlsx"
    assert restored.summary.errors == 1
    assert len(restored.findings) == 1


def test_fix_op():
    op = FixOp(op="write", address="Sheet1!A1", value=42)
    data = op.model_dump()
    assert data["kind"] == "xli-batch-op"
    assert data["op"] == "write"
    assert data["address"] == "Sheet1!A1"
