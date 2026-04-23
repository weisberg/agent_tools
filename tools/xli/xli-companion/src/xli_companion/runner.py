"""Main orchestration logic for the xli companion validator."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import polars as pl

from xli_companion import xli
from xli_companion.checks import (
    check_duplicate_keys,
    check_named_ranges,
    check_null_rates,
    check_required_sheets,
    check_type_consistency,
)
from xli_companion.models import CompanionResult, Finding, Severity, Summary


def run_companion(
    workbook: Path,
    *,
    doctor_path: Path | None = None,
    inspect_path: Path | None = None,
    source_path: Path | None = None,
    required_sheets: list[str] | None = None,
    expected_names: list[str] | None = None,
    key_columns: list[str] | None = None,
    null_threshold: float = 0.5,
    fix_plan_path: Path | None = None,
) -> CompanionResult:
    """Run all configured checks and return a structured result."""
    findings: list[Finding] = []
    checks_run = 0

    # Get inspect output (from file or by running xli)
    if inspect_path and inspect_path.exists():
        inspect_data = json.loads(inspect_path.read_text())
        inspect_output = inspect_data.get("output", inspect_data)
    else:
        try:
            envelope = xli.inspect(workbook)
            inspect_output = envelope.get("output", {})
        except (xli.XliError, FileNotFoundError) as exc:
            return CompanionResult(
                status="error",
                workbook=str(workbook),
                findings=[Finding(
                    code="XLI_INSPECT_FAILED",
                    severity=Severity.ERROR,
                    message=str(exc),
                )],
                summary=Summary(checks_run=1, errors=1),
            )

    fingerprint = inspect_output.get("fingerprint")

    # Structural checks
    if required_sheets:
        findings.extend(check_required_sheets(inspect_output, required_sheets))
        checks_run += 1

    if expected_names is not None:
        findings.extend(check_named_ranges(inspect_output, expected_names or None))
        checks_run += 1

    # Ingest doctor output if provided
    if doctor_path and doctor_path.exists():
        doctor_data = json.loads(doctor_path.read_text())
        doctor_errors = doctor_data.get("errors", [])
        for err in doctor_errors:
            findings.append(Finding(
                code=f"DOCTOR_{err.get('code', 'UNKNOWN')}",
                severity=Severity.ERROR,
                message=err.get("details", err.get("message", str(err))),
            ))
        checks_run += 1

    # Data quality checks per sheet
    for sheet_info in inspect_output.get("sheets", []):
        sheet_name = sheet_info["name"]
        if sheet_info.get("rows", 0) == 0:
            continue
        if sheet_info.get("is_chart_sheet", False):
            continue

        try:
            dims = sheet_info.get("dimensions")
            if not dims:
                continue
            range_ref = f"{sheet_name}!{dims}"
            envelope = xli.run(["read", str(workbook), range_ref, "--headers"])
            rows = envelope.get("output", {}).get("rows", [])
            headers = envelope.get("output", {}).get("headers")
            if not rows:
                continue

            df = pl.DataFrame(rows)

            findings.extend(check_null_rates(df, sheet_name=sheet_name, threshold=null_threshold))
            checks_run += 1

            findings.extend(check_type_consistency(df, sheet_name=sheet_name))
            checks_run += 1

            if key_columns:
                findings.extend(check_duplicate_keys(df, sheet_name=sheet_name, key_columns=key_columns))
                checks_run += 1
        except Exception:
            # Non-fatal: skip sheet if read fails
            continue

    # Build summary
    error_count = sum(1 for f in findings if f.severity == Severity.ERROR)
    warning_count = sum(1 for f in findings if f.severity == Severity.WARNING)
    info_count = sum(1 for f in findings if f.severity == Severity.INFO)

    result = CompanionResult(
        status="error" if error_count > 0 else "ok",
        workbook=str(workbook),
        validated_fingerprint=fingerprint,
        summary=Summary(
            checks_run=checks_run,
            errors=error_count,
            warnings=warning_count,
            info=info_count,
        ),
        findings=findings,
    )

    # Write fix plan if requested
    if fix_plan_path and result.fix_plan:
        lines = [op.model_dump_json() for op in result.fix_plan]
        fix_plan_path.write_text("\n".join(lines) + "\n")
        result.artifacts["fix_plan"] = str(fix_plan_path)

    return result
