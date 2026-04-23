"""Tests for xli_companion.reporting."""

from __future__ import annotations

import tempfile
from pathlib import Path

from xli_companion.models import (
    CompanionResult,
    Finding,
    FixOp,
    Severity,
    Summary,
)
from xli_companion.reporting import render_html, render_markdown, write_report


def _make_result(**kwargs) -> CompanionResult:
    """Helper to build a CompanionResult with sensible defaults."""
    defaults = {"workbook": "test.xlsx", "status": "ok"}
    defaults.update(kwargs)
    return CompanionResult(**defaults)


def test_render_markdown_basic():
    result = _make_result()
    md = render_markdown(result)
    assert "test.xlsx" in md
    assert "ok" in md
    assert "Checks run" in md


def test_render_markdown_with_findings():
    findings = [
        Finding(
            code="E001",
            severity=Severity.ERROR,
            sheet="Sheet1",
            cell="A1",
            message="Missing required value",
        ),
        Finding(
            code="W002",
            severity=Severity.WARNING,
            message="Column header mismatch",
        ),
    ]
    result = _make_result(
        summary=Summary(checks_run=2, errors=1, warnings=1, info=0),
        findings=findings,
    )
    md = render_markdown(result)
    assert "E001" in md
    assert "W002" in md
    assert "Missing required value" in md
    assert "Sheet1" in md
    assert "A1" in md


def test_render_html():
    result = _make_result()
    html_out = render_html(result)
    assert html_out  # non-empty
    assert "test.xlsx" in html_out
    assert "<html>" in html_out
    assert "<pre>" in html_out


def test_write_report():
    result = _make_result(
        summary=Summary(checks_run=3, errors=0, warnings=1, info=2),
        findings=[
            Finding(
                code="W010",
                severity=Severity.WARNING,
                message="Deprecated format",
            ),
        ],
        fix_plan=[
            FixOp(op="set", address="Sheet1!B2", value="fixed"),
        ],
    )
    with tempfile.TemporaryDirectory() as tmp:
        md_path = Path(tmp) / "report.md"
        write_report(result, md_path)
        assert md_path.exists()
        content = md_path.read_text(encoding="utf-8")
        assert len(content) > 0
        assert "test.xlsx" in content

        html_path = Path(tmp) / "report.html"
        write_report(result, html_path, fmt="html")
        assert html_path.exists()
        html_content = html_path.read_text(encoding="utf-8")
        assert "<html>" in html_content
