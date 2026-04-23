"""Batch audit: discover, audit, and report on collections of .docx files."""

from __future__ import annotations

import csv
import io
import json
import zipfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import List, Optional, Sequence


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class BatchConfig:
    """Configuration for a batch audit run."""

    root_dir: Path
    glob_pattern: str = "**/*.docx"
    max_workers: int = 4
    required_parts: tuple[str, ...] = (
        "[Content_Types].xml",
        "word/document.xml",
    )


@dataclass
class BatchResult:
    """Result of auditing a single document."""

    path: str
    valid: bool
    is_zip: bool = True
    missing_parts: List[str] = field(default_factory=list)
    error: Optional[str] = None


# ---------------------------------------------------------------------------
# Discovery
# ---------------------------------------------------------------------------

def discover_documents(config: BatchConfig) -> List[Path]:
    """Return sorted list of paths matching *config.glob_pattern* under *config.root_dir*."""
    root = Path(config.root_dir)
    if not root.is_dir():
        raise FileNotFoundError(f"Root directory does not exist: {root}")
    return sorted(root.glob(config.glob_pattern))


# ---------------------------------------------------------------------------
# Single-file audit (self-contained, no companion imports)
# ---------------------------------------------------------------------------

def audit_single(path: Path, required_parts: Sequence[str]) -> BatchResult:
    """Perform a structural check on a single .docx file.

    Opens the file as a ZIP archive and verifies that every entry listed in
    *required_parts* is present.  Returns a :class:`BatchResult` describing
    the outcome.
    """
    str_path = str(path)
    try:
        if not path.is_file():
            return BatchResult(
                path=str_path, valid=False, is_zip=False,
                error="File does not exist",
            )

        if not zipfile.is_zipfile(path):
            return BatchResult(
                path=str_path, valid=False, is_zip=False,
                error="Not a valid ZIP archive",
            )

        with zipfile.ZipFile(path, "r") as zf:
            names = set(zf.namelist())
            missing = [p for p in required_parts if p not in names]

        return BatchResult(
            path=str_path,
            valid=len(missing) == 0,
            is_zip=True,
            missing_parts=missing,
        )

    except Exception as exc:  # noqa: BLE001
        return BatchResult(
            path=str_path, valid=False, is_zip=False,
            error=str(exc),
        )


# ---------------------------------------------------------------------------
# Batch audit (parallel)
# ---------------------------------------------------------------------------

def audit_batch(config: BatchConfig) -> List[BatchResult]:
    """Discover and audit all documents under *config.root_dir* in parallel.

    Uses a :class:`~concurrent.futures.ThreadPoolExecutor` bounded by
    *config.max_workers*.
    """
    paths = discover_documents(config)
    results: List[BatchResult] = []

    with ThreadPoolExecutor(max_workers=config.max_workers) as pool:
        futures = {
            pool.submit(audit_single, p, config.required_parts): p
            for p in paths
        }
        for future in as_completed(futures):
            results.append(future.result())

    # Return in deterministic (sorted-path) order for reproducibility.
    results.sort(key=lambda r: r.path)
    return results


# ---------------------------------------------------------------------------
# Reporting helpers
# ---------------------------------------------------------------------------

def batch_to_csv(results: Sequence[BatchResult]) -> str:
    """Serialise *results* to a CSV string."""
    buf = io.StringIO()
    writer = csv.DictWriter(
        buf,
        fieldnames=["path", "valid", "is_zip", "missing_parts", "error"],
    )
    writer.writeheader()
    for r in results:
        row = asdict(r)
        row["missing_parts"] = ";".join(row["missing_parts"])
        writer.writerow(row)
    return buf.getvalue()


def batch_to_json(results: Sequence[BatchResult]) -> str:
    """Serialise *results* to a JSON string."""
    return json.dumps([asdict(r) for r in results], indent=2)


def batch_summary_markdown(results: Sequence[BatchResult]) -> str:
    """Return a Markdown summary table for *results*."""
    total = len(results)
    passed = sum(1 for r in results if r.valid)
    failed = total - passed

    lines = [
        f"# Batch Audit Summary",
        "",
        f"| Metric | Value |",
        f"|--------|-------|",
        f"| Total  | {total} |",
        f"| Passed | {passed} |",
        f"| Failed | {failed} |",
        "",
    ]

    if failed:
        lines.append("## Failed Documents")
        lines.append("")
        lines.append("| Path | Reason |")
        lines.append("|------|--------|")
        for r in results:
            if not r.valid:
                reason = r.error or f"Missing parts: {', '.join(r.missing_parts)}"
                lines.append(f"| `{r.path}` | {reason} |")
        lines.append("")

    return "\n".join(lines)
