"""Built-in validation checks for xli-companion."""

from xli_companion.checks.structure import check_required_sheets, check_named_ranges
from xli_companion.checks.data_quality import check_null_rates, check_type_consistency, check_duplicate_keys

__all__ = [
    "check_required_sheets",
    "check_named_ranges",
    "check_null_rates",
    "check_type_consistency",
    "check_duplicate_keys",
]
