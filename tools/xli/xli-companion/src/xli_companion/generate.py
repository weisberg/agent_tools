"""Scratch workbook generation using XlsxWriter."""

from __future__ import annotations

from pathlib import Path

import polars as pl
import xlsxwriter


def generate_from_dataframe(
    df: pl.DataFrame,
    path: Path,
    sheet_name: str = "Sheet1",
) -> Path:
    """Write a polars DataFrame to an xlsx file.

    Headers are written in row 0, data starts at row 1.
    Returns *path* for convenience.
    """
    workbook = xlsxwriter.Workbook(str(path))
    try:
        _write_sheet(workbook, sheet_name, df)
    finally:
        workbook.close()
    return path


def generate_from_csv(
    csv_path: Path,
    xlsx_path: Path,
    sheet_name: str = "Sheet1",
) -> Path:
    """Read a CSV with polars and write it to xlsx."""
    df = pl.read_csv(csv_path)
    return generate_from_dataframe(df, xlsx_path, sheet_name=sheet_name)


def generate_multi_sheet(
    sheets: dict[str, pl.DataFrame],
    path: Path,
) -> Path:
    """Create a workbook with multiple sheets, one per dict entry."""
    workbook = xlsxwriter.Workbook(str(path))
    try:
        for name, df in sheets.items():
            _write_sheet(workbook, name, df)
    finally:
        workbook.close()
    return path


def generate_from_parquet(
    parquet_path: Path,
    xlsx_path: Path,
    sheet_name: str = "Sheet1",
) -> Path:
    """Read a Parquet file with polars and write it to xlsx."""
    df = pl.read_parquet(parquet_path)
    return generate_from_dataframe(df, xlsx_path, sheet_name=sheet_name)


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

def _write_sheet(
    workbook: xlsxwriter.Workbook,
    sheet_name: str,
    df: pl.DataFrame,
) -> None:
    """Write *df* into a new worksheet inside *workbook*."""
    worksheet = workbook.add_worksheet(sheet_name)

    # Write headers (row 0)
    for col_idx, col_name in enumerate(df.columns):
        worksheet.write_string(0, col_idx, col_name)

    # Write data (starting at row 1)
    for row_idx in range(df.height):
        for col_idx, col_name in enumerate(df.columns):
            value = df[row_idx, col_idx]
            dtype = df.schema[col_name]

            if value is None:
                worksheet.write_blank(row_idx + 1, col_idx, None)
            elif dtype in (
                pl.Boolean,
            ):
                worksheet.write_boolean(row_idx + 1, col_idx, value)
            elif dtype.is_numeric():
                worksheet.write_number(row_idx + 1, col_idx, value)
            else:
                worksheet.write_string(row_idx + 1, col_idx, str(value))
