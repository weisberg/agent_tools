"""CLI entry point for xli-companion."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from xli_companion.runner import run_companion


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="xli-companion",
        description="Python companion for XLI — heavyweight validation and reporting",
    )
    parser.add_argument("workbook", type=Path, help="Path to the .xlsx workbook")
    parser.add_argument("--doctor", type=Path, help="Path to xli doctor JSON output")
    parser.add_argument("--inspect", type=Path, help="Path to xli inspect JSON output")
    parser.add_argument("--source", type=Path, help="Path to source data (parquet/csv) for reconciliation")
    parser.add_argument("--required-sheets", nargs="*", help="Sheet names that must exist")
    parser.add_argument("--expected-names", nargs="*", help="Named ranges that should exist")
    parser.add_argument("--key-columns", nargs="*", help="Columns to check for duplicate keys")
    parser.add_argument("--out", type=Path, help="Output JSON path (default: stdout)")
    parser.add_argument("--fix-plan", type=Path, help="Output xli batch ndjson fix plan")
    parser.add_argument("--null-threshold", type=float, default=0.5, help="Null rate warning threshold (0-1)")

    args = parser.parse_args(argv)

    result = run_companion(
        workbook=args.workbook,
        doctor_path=args.doctor,
        inspect_path=args.inspect,
        source_path=args.source,
        required_sheets=args.required_sheets or [],
        expected_names=args.expected_names or [],
        key_columns=args.key_columns or [],
        null_threshold=args.null_threshold,
        fix_plan_path=args.fix_plan,
    )

    output = result.model_dump_json(indent=2)

    if args.out:
        args.out.write_text(output)
    else:
        print(output)

    if result.summary.errors > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
