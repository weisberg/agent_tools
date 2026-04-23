"""Structural validation checks using xli inspect output."""

from __future__ import annotations

from typing import Any

from xli_companion.models import Finding, Severity


def check_required_sheets(
    inspect_output: dict[str, Any],
    required: list[str],
) -> list[Finding]:
    """Check that all required sheets exist in the workbook."""
    findings: list[Finding] = []
    sheet_names = {s["name"] for s in inspect_output.get("sheets", [])}
    for name in required:
        if name not in sheet_names:
            findings.append(Finding(
                code="MISSING_REQUIRED_SHEET",
                severity=Severity.ERROR,
                sheet=name,
                message=f"Required sheet '{name}' is missing from the workbook.",
            ))
    return findings


def check_named_ranges(
    inspect_output: dict[str, Any],
    expected: list[str] | None = None,
) -> list[Finding]:
    """Check defined names / named ranges for issues."""
    findings: list[Finding] = []
    defined = inspect_output.get("defined_names", {})

    if expected:
        for name in expected:
            if name not in defined:
                findings.append(Finding(
                    code="MISSING_NAMED_RANGE",
                    severity=Severity.WARNING,
                    message=f"Expected named range '{name}' not found.",
                ))

    for name, formula in defined.items():
        if "#REF!" in formula:
            findings.append(Finding(
                code="BROKEN_NAMED_RANGE",
                severity=Severity.ERROR,
                message=f"Named range '{name}' references an error: {formula}",
            ))

    return findings
