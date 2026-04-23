"""Tests for validation checks."""

import polars as pl

from xli_companion.checks.data_quality import (
    check_duplicate_keys,
    check_null_rates,
    check_type_consistency,
)
from xli_companion.checks.structure import check_named_ranges, check_required_sheets


def test_required_sheets_all_present():
    inspect = {"sheets": [{"name": "Summary"}, {"name": "Data"}]}
    findings = check_required_sheets(inspect, ["Summary", "Data"])
    assert len(findings) == 0


def test_required_sheets_missing():
    inspect = {"sheets": [{"name": "Data"}]}
    findings = check_required_sheets(inspect, ["Summary", "Data"])
    assert len(findings) == 1
    assert findings[0].code == "MISSING_REQUIRED_SHEET"
    assert findings[0].sheet == "Summary"


def test_named_ranges_broken_ref():
    inspect = {"defined_names": {"totals": "#REF!A1:A10"}}
    findings = check_named_ranges(inspect)
    assert len(findings) == 1
    assert findings[0].code == "BROKEN_NAMED_RANGE"


def test_named_ranges_expected_missing():
    inspect = {"defined_names": {"totals": "Sheet1!A1:A10"}}
    findings = check_named_ranges(inspect, expected=["totals", "headers"])
    assert any(f.code == "MISSING_NAMED_RANGE" for f in findings)


def test_null_rates_below_threshold():
    df = pl.DataFrame({"a": [1, 2, 3], "b": [4, 5, 6]})
    findings = check_null_rates(df, sheet_name="Sheet1", threshold=0.5)
    assert len(findings) == 0


def test_null_rates_above_threshold():
    df = pl.DataFrame({"a": [1, None, None, None], "b": [1, 2, 3, 4]})
    findings = check_null_rates(df, sheet_name="Sheet1", threshold=0.5)
    assert len(findings) == 1
    assert findings[0].code == "HIGH_NULL_RATE"


def test_type_consistency_clean():
    df = pl.DataFrame({"a": ["hello", "world"], "b": [1, 2]})
    findings = check_type_consistency(df, sheet_name="Sheet1")
    assert len(findings) == 0


def test_type_consistency_mixed():
    df = pl.DataFrame({"a": ["hello", "42", "world"]})
    findings = check_type_consistency(df, sheet_name="Sheet1")
    assert len(findings) == 1
    assert findings[0].code == "MIXED_TYPES"


def test_duplicate_keys_none():
    df = pl.DataFrame({"id": [1, 2, 3], "name": ["a", "b", "c"]})
    findings = check_duplicate_keys(df, sheet_name="Sheet1", key_columns=["id"])
    assert len(findings) == 0


def test_duplicate_keys_found():
    df = pl.DataFrame({"id": [1, 1, 2], "name": ["a", "b", "c"]})
    findings = check_duplicate_keys(df, sheet_name="Sheet1", key_columns=["id"])
    assert len(findings) == 1
    assert findings[0].code == "DUPLICATE_KEYS"
