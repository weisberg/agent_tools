"""Tests for source-to-workbook reconciliation checks."""

import polars as pl

from xli_companion.checks.reconciliation import (
    diff_values,
    reconcile_row_counts,
    reconcile_schema,
    reconcile_totals,
)


# --- reconcile_totals ---

def test_totals_match():
    wb = pl.DataFrame({"amount": [10, 20, 30]})
    src = pl.DataFrame({"amount": [10, 20, 30]})
    findings = reconcile_totals(wb, src, ["amount"], sheet_name="Sheet1")
    assert len(findings) == 0


def test_totals_mismatch():
    wb = pl.DataFrame({"amount": [10, 20, 30]})
    src = pl.DataFrame({"amount": [10, 20, 99]})
    findings = reconcile_totals(wb, src, ["amount"], sheet_name="Sheet1")
    assert len(findings) == 1
    assert findings[0].code == "TOTAL_MISMATCH"
    assert findings[0].severity.value == "error"
    assert findings[0].details["column"] == "amount"


# --- reconcile_row_counts ---

def test_row_count_match():
    wb = pl.DataFrame({"a": [1, 2, 3]})
    src = pl.DataFrame({"a": [4, 5, 6]})
    findings = reconcile_row_counts(wb, src, sheet_name="Sheet1")
    assert len(findings) == 0


def test_row_count_mismatch():
    wb = pl.DataFrame({"a": [1, 2, 3]})
    src = pl.DataFrame({"a": [4, 5]})
    findings = reconcile_row_counts(wb, src, sheet_name="Sheet1")
    assert len(findings) == 1
    assert findings[0].code == "ROW_COUNT_MISMATCH"
    assert findings[0].severity.value == "warning"


# --- reconcile_schema ---

def test_schema_match():
    wb = pl.DataFrame({"id": [1], "name": ["a"]})
    src = pl.DataFrame({"id": [2], "name": ["b"]})
    findings = reconcile_schema(wb, src, sheet_name="Sheet1")
    assert len(findings) == 0


def test_schema_missing_column():
    wb = pl.DataFrame({"id": [1]})
    src = pl.DataFrame({"id": [2], "extra_col": ["x"]})
    findings = reconcile_schema(wb, src, sheet_name="Sheet1")
    assert len(findings) >= 1
    codes = [f.code for f in findings]
    assert "SCHEMA_MISSING_COLUMNS" in codes
    missing_finding = [f for f in findings if f.code == "SCHEMA_MISSING_COLUMNS"][0]
    assert "extra_col" in missing_finding.details["missing_columns"]


# --- diff_values ---

def test_diff_values_identical():
    wb = pl.DataFrame({"id": [1, 2], "value": [100, 200]})
    src = pl.DataFrame({"id": [1, 2], "value": [100, 200]})
    findings = diff_values(wb, src, ["id"], sheet_name="Sheet1")
    assert len(findings) == 0


def test_diff_values_changed():
    wb = pl.DataFrame({"id": [1, 2], "value": [100, 200]})
    src = pl.DataFrame({"id": [1, 2], "value": [100, 999]})
    findings = diff_values(wb, src, ["id"], sheet_name="Sheet1")
    assert len(findings) == 1
    assert findings[0].code == "VALUE_MISMATCH"
    assert findings[0].details["column"] == "value"
    assert findings[0].details["diff_count"] == 1
    assert len(findings[0].details["examples"]) == 1
    example = findings[0].details["examples"][0]
    assert example["key"] == {"id": 2}
    assert example["workbook_value"] == 200
    assert example["source_value"] == 999
