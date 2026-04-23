"""Source-to-workbook reconciliation checks using polars."""

from __future__ import annotations

import polars as pl

from xli_companion.models import Finding, Severity

# Tolerance for floating-point comparison
_EPSILON = 1e-9


def reconcile_totals(
    wb_df: pl.DataFrame,
    source_df: pl.DataFrame,
    columns: list[str],
    *,
    sheet_name: str = "",
) -> list[Finding]:
    """Compare column sums between workbook and source DataFrames.

    For each column in *columns*, flag an ERROR if the sums differ by more
    than a small epsilon.
    """
    findings: list[Finding] = []
    for col in columns:
        if col not in wb_df.columns or col not in source_df.columns:
            findings.append(Finding(
                code="RECONCILE_TOTAL_MISSING_COL",
                severity=Severity.ERROR,
                sheet=sheet_name or None,
                message=f"Column '{col}' not present in both DataFrames; cannot reconcile totals.",
                details={"column": col},
            ))
            continue

        wb_sum = wb_df[col].cast(pl.Float64).sum()
        src_sum = source_df[col].cast(pl.Float64).sum()

        if abs(wb_sum - src_sum) > _EPSILON:
            findings.append(Finding(
                code="TOTAL_MISMATCH",
                severity=Severity.ERROR,
                sheet=sheet_name or None,
                message=(
                    f"Column '{col}' totals differ: "
                    f"workbook={wb_sum}, source={src_sum}, delta={wb_sum - src_sum}."
                ),
                details={
                    "column": col,
                    "workbook_total": wb_sum,
                    "source_total": src_sum,
                    "delta": wb_sum - src_sum,
                },
            ))
    return findings


def reconcile_row_counts(
    wb_df: pl.DataFrame,
    source_df: pl.DataFrame,
    *,
    sheet_name: str = "",
) -> list[Finding]:
    """Flag a WARNING if the row counts of the two DataFrames don't match."""
    findings: list[Finding] = []
    wb_rows = len(wb_df)
    src_rows = len(source_df)
    if wb_rows != src_rows:
        findings.append(Finding(
            code="ROW_COUNT_MISMATCH",
            severity=Severity.WARNING,
            sheet=sheet_name or None,
            message=(
                f"Row counts differ: workbook={wb_rows}, source={src_rows}."
            ),
            details={
                "workbook_rows": wb_rows,
                "source_rows": src_rows,
            },
        ))
    return findings


def reconcile_schema(
    wb_df: pl.DataFrame,
    source_df: pl.DataFrame,
    *,
    sheet_name: str = "",
) -> list[Finding]:
    """Check that column names match between workbook and source.

    Flags missing columns (in source but not workbook) and extra columns
    (in workbook but not source).
    """
    findings: list[Finding] = []
    wb_cols = set(wb_df.columns)
    src_cols = set(source_df.columns)

    missing = src_cols - wb_cols
    extra = wb_cols - src_cols

    if missing:
        findings.append(Finding(
            code="SCHEMA_MISSING_COLUMNS",
            severity=Severity.WARNING,
            sheet=sheet_name or None,
            message=f"Columns in source but missing from workbook: {sorted(missing)}.",
            details={"missing_columns": sorted(missing)},
        ))

    if extra:
        findings.append(Finding(
            code="SCHEMA_EXTRA_COLUMNS",
            severity=Severity.WARNING,
            sheet=sheet_name or None,
            message=f"Columns in workbook but not in source: {sorted(extra)}.",
            details={"extra_columns": sorted(extra)},
        ))

    return findings


def diff_values(
    wb_df: pl.DataFrame,
    source_df: pl.DataFrame,
    key_columns: list[str],
    *,
    sheet_name: str = "",
) -> list[Finding]:
    """Join on key_columns and find rows where non-key columns differ."""
    findings: list[Finding] = []

    non_key = [c for c in wb_df.columns if c not in key_columns]
    common_non_key = [c for c in non_key if c in source_df.columns]

    if not common_non_key:
        return findings

    # Rename non-key columns in source to avoid collisions
    src_renamed = source_df.rename({c: f"{c}__src" for c in common_non_key})

    joined = wb_df.join(src_renamed, on=key_columns, how="inner")

    for col in common_non_key:
        wb_col = col
        src_col = f"{col}__src"

        # Cast both to string for safe comparison across types
        diff_mask = joined[wb_col].cast(pl.String) != joined[src_col].cast(pl.String)
        diff_rows = joined.filter(diff_mask)

        if len(diff_rows) > 0:
            # Collect up to 10 example differences
            examples = []
            for row in diff_rows.head(10).iter_rows(named=True):
                key_vals = {k: row[k] for k in key_columns}
                examples.append({
                    "key": key_vals,
                    "workbook_value": row[wb_col],
                    "source_value": row[src_col],
                })

            findings.append(Finding(
                code="VALUE_MISMATCH",
                severity=Severity.ERROR,
                sheet=sheet_name or None,
                message=(
                    f"Column '{col}' has {len(diff_rows)} differing "
                    f"row(s) when joined on {key_columns}."
                ),
                details={
                    "column": col,
                    "diff_count": len(diff_rows),
                    "examples": examples,
                },
            ))

    return findings
