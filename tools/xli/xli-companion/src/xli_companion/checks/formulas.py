"""Formula pattern analysis checks."""

from __future__ import annotations

import re
from collections import defaultdict

from xli_companion.models import Finding, Severity

# Volatile functions that trigger recalculation
_VOLATILE_FUNCTIONS = {"NOW", "RAND", "OFFSET", "INDIRECT", "INFO", "CELL"}

# Excel error values
_ERROR_VALUES = {"#REF!", "#N/A", "#DIV/0!", "#VALUE!", "#NAME?", "#NULL!", "#NUM!"}

# Common multiplier / divisor constants that should not be flagged
_COMMON_NUMBERS = {0, 1, 100, 1000}

# Regex to find literal numbers in a formula string (integers and decimals)
_NUMBER_RE = re.compile(r"(?<![A-Za-z_\$])(\d+(?:\.\d+)?)(?![A-Za-z_\(\$])")

# Regex to replace cell references with a placeholder so formulas can be grouped
_CELL_REF_RE = re.compile(r"\$?[A-Z]{1,3}\$?\d+")


def check_volatile_functions(cells: list[dict]) -> list[Finding]:
    """Flag cells whose formulas contain volatile functions."""
    findings: list[Finding] = []
    for cell in cells:
        formula = cell.get("formula") or ""
        if not formula:
            continue
        upper = formula.upper()
        found = [fn for fn in _VOLATILE_FUNCTIONS if f"{fn}(" in upper]
        if found:
            findings.append(Finding(
                code="VOLATILE_FUNCTION",
                severity=Severity.WARNING,
                sheet=cell.get("sheet"),
                cell=cell.get("address"),
                message=f"Volatile function(s) {', '.join(sorted(found))} in formula.",
                details={"formula": formula, "volatile": sorted(found)},
            ))
    return findings


def check_error_values(cells: list[dict]) -> list[Finding]:
    """Flag cells whose value is an Excel error string."""
    findings: list[Finding] = []
    for cell in cells:
        value = cell.get("value")
        if isinstance(value, str) and value.strip() in _ERROR_VALUES:
            findings.append(Finding(
                code="ERROR_VALUE",
                severity=Severity.ERROR,
                sheet=cell.get("sheet"),
                cell=cell.get("address"),
                message=f"Cell contains error value {value.strip()}.",
                details={"value": value.strip()},
            ))
    return findings


def check_hardcoded_in_formulas(cells: list[dict]) -> list[Finding]:
    """Flag formulas containing hardcoded literal numbers (excluding common ones)."""
    findings: list[Finding] = []
    for cell in cells:
        formula = cell.get("formula") or ""
        if not formula:
            continue
        matches = _NUMBER_RE.findall(formula)
        unusual = []
        for m in matches:
            try:
                num = float(m)
            except ValueError:
                continue
            if num not in _COMMON_NUMBERS:
                unusual.append(m)
        if unusual:
            findings.append(Finding(
                code="HARDCODED_NUMBER",
                severity=Severity.INFO,
                sheet=cell.get("sheet"),
                cell=cell.get("address"),
                message=f"Formula contains hardcoded number(s): {', '.join(unusual)}.",
                details={"formula": formula, "numbers": unusual},
            ))
    return findings


def _formula_pattern(formula: str) -> str:
    """Normalise a formula by replacing cell references with a placeholder."""
    return _CELL_REF_RE.sub("_REF_", formula)


def check_inconsistent_ranges(cells: list[dict]) -> list[Finding]:
    """Find columns where most cells have a formula but some don't (gaps)."""
    findings: list[Finding] = []

    # Group cells by (sheet, column letter)
    col_cells: dict[tuple[str, str], list[dict]] = defaultdict(list)
    col_re = re.compile(r"^(?:.*!)?([A-Z]+)\d+$", re.IGNORECASE)
    for cell in cells:
        address = cell.get("address", "")
        m = col_re.match(address)
        if m:
            col_letter = m.group(1).upper()
            sheet = cell.get("sheet", "")
            col_cells[(sheet, col_letter)].append(cell)

    for (sheet, col), group in col_cells.items():
        total = len(group)
        if total < 3:
            continue
        has_formula = [c for c in group if c.get("formula")]
        without_formula = [c for c in group if not c.get("formula")]
        # If the majority have a formula but some don't, flag the gaps
        if has_formula and without_formula and len(has_formula) > len(without_formula):
            for gap_cell in without_formula:
                findings.append(Finding(
                    code="INCONSISTENT_FORMULA",
                    severity=Severity.WARNING,
                    sheet=sheet,
                    cell=gap_cell.get("address"),
                    message=(
                        f"Column {col} has formulas in most rows but this cell is missing one."
                    ),
                ))

    return findings
