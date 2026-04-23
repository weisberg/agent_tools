"""Tests for formula pattern analysis checks."""

from xli_companion.checks.formulas import (
    check_error_values,
    check_hardcoded_in_formulas,
    check_inconsistent_ranges,
    check_volatile_functions,
)


def test_volatile_detected():
    cells = [{"address": "Sheet1!A1", "value": None, "formula": "=NOW()", "sheet": "Sheet1"}]
    findings = check_volatile_functions(cells)
    assert len(findings) == 1
    assert findings[0].code == "VOLATILE_FUNCTION"
    assert findings[0].severity.value == "warning"


def test_no_volatile():
    cells = [{"address": "Sheet1!A1", "value": 55, "formula": "=SUM(A1:A10)", "sheet": "Sheet1"}]
    findings = check_volatile_functions(cells)
    assert len(findings) == 0


def test_error_value_detected():
    cells = [{"address": "Sheet1!B2", "value": "#REF!", "formula": "=A1+B1", "sheet": "Sheet1"}]
    findings = check_error_values(cells)
    assert len(findings) == 1
    assert findings[0].code == "ERROR_VALUE"
    assert findings[0].severity.value == "error"


def test_no_errors():
    cells = [{"address": "Sheet1!A1", "value": 42, "formula": "=SUM(A1:A10)", "sheet": "Sheet1"}]
    findings = check_error_values(cells)
    assert len(findings) == 0


def test_hardcoded_numbers():
    cells = [{"address": "Sheet1!C1", "value": 108, "formula": "=A1*1.08", "sheet": "Sheet1"}]
    findings = check_hardcoded_in_formulas(cells)
    assert len(findings) == 1
    assert findings[0].code == "HARDCODED_NUMBER"
    assert "1.08" in findings[0].message


def test_common_numbers_not_flagged():
    cells = [{"address": "Sheet1!C1", "value": 500, "formula": "=A1*100", "sheet": "Sheet1"}]
    findings = check_hardcoded_in_formulas(cells)
    assert len(findings) == 0


def test_inconsistent_range():
    cells = []
    for row in range(1, 10):
        cell = {
            "address": f"Sheet1!A{row}",
            "value": row,
            "formula": f"=B{row}+C{row}" if row != 5 else None,
            "sheet": "Sheet1",
        }
        cells.append(cell)
    findings = check_inconsistent_ranges(cells)
    assert len(findings) == 1
    assert findings[0].code == "INCONSISTENT_FORMULA"
    assert findings[0].cell == "Sheet1!A5"
