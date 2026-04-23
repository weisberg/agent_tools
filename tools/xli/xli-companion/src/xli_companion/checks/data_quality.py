"""Data quality checks using polars for fast analysis."""

from __future__ import annotations

from typing import Any

import polars as pl

from xli_companion.models import Finding, Severity


def check_null_rates(
    df: pl.DataFrame,
    *,
    sheet_name: str,
    threshold: float = 0.5,
) -> list[Finding]:
    """Flag columns where null rate exceeds the threshold."""
    findings: list[Finding] = []
    total = len(df)
    if total == 0:
        return findings

    for col in df.columns:
        null_count = df[col].null_count()
        rate = null_count / total
        if rate > threshold:
            findings.append(Finding(
                code="HIGH_NULL_RATE",
                severity=Severity.WARNING,
                sheet=sheet_name,
                message=f"Column '{col}' has {rate:.0%} null values ({null_count}/{total}).",
                details={"column": col, "null_rate": round(rate, 4)},
            ))
    return findings


def check_type_consistency(
    df: pl.DataFrame,
    *,
    sheet_name: str,
) -> list[Finding]:
    """Flag columns where mixed types suggest data quality issues."""
    findings: list[Finding] = []
    for col in df.columns:
        series = df[col].drop_nulls()
        if len(series) == 0:
            continue
        # In polars, string columns with numeric-looking values suggest mixed types
        if series.dtype == pl.String:
            numeric_count = series.str.contains(r"^-?\d+\.?\d*$").sum()
            if 0 < numeric_count < len(series):
                findings.append(Finding(
                    code="MIXED_TYPES",
                    severity=Severity.WARNING,
                    sheet=sheet_name,
                    message=f"Column '{col}' has {numeric_count} numeric-looking values in a string column.",
                    details={"column": col, "numeric_count": numeric_count, "total": len(series)},
                ))
    return findings


def check_duplicate_keys(
    df: pl.DataFrame,
    *,
    sheet_name: str,
    key_columns: list[str],
) -> list[Finding]:
    """Check for duplicate rows based on key columns."""
    findings: list[Finding] = []
    available = [c for c in key_columns if c in df.columns]
    if not available:
        return findings

    dupes = df.group_by(available).len().filter(pl.col("len") > 1)
    if len(dupes) > 0:
        findings.append(Finding(
            code="DUPLICATE_KEYS",
            severity=Severity.ERROR,
            sheet=sheet_name,
            message=f"Found {len(dupes)} duplicate key combinations in columns {available}.",
            details={"key_columns": available, "duplicate_count": len(dupes)},
        ))
    return findings
