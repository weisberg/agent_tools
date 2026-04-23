"""Report generation for docli-companion.

Functions for computing release state, building summary statistics,
assembling a full CompanionReport, rendering it to Markdown, and
writing the artifact bundle to disk.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

from .models import (
    ArtifactBundle,
    CheckResult,
    CompanionReport,
    DocumentIdentity,
    ReleaseState,
    ReportSummary,
    Severity,
    ToolVersions,
)


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def compute_release_state(checks: list[CheckResult]) -> ReleaseState:
    """Derive the overall release state from a list of check results.

    Rules (evaluated in priority order):
    1. Any check with severity ERROR and status "failed" -> FAIL
    2. Any check with status "errored" -> MANUAL_REVIEW
    3. Any check with severity WARNING and status "failed" -> PASS_WITH_WARNINGS
    4. Otherwise -> PASS
    """
    has_error_failure = False
    has_errored = False
    has_warning_failure = False

    for check in checks:
        if check.status == "failed" and check.severity == Severity.ERROR:
            has_error_failure = True
        if check.status == "errored":
            has_errored = True
        if check.status == "failed" and check.severity == Severity.WARNING:
            has_warning_failure = True

    if has_error_failure:
        return ReleaseState.FAIL
    if has_errored:
        return ReleaseState.MANUAL_REVIEW
    if has_warning_failure:
        return ReleaseState.PASS_WITH_WARNINGS
    return ReleaseState.PASS


def compute_summary(checks: list[CheckResult]) -> ReportSummary:
    """Aggregate check results into a ReportSummary."""
    passed = sum(1 for c in checks if c.status == "passed")
    failed = sum(1 for c in checks if c.status == "failed")
    errored = sum(1 for c in checks if c.status == "errored")
    skipped = sum(1 for c in checks if c.status == "skipped")
    warnings = sum(
        1 for c in checks if c.severity == Severity.WARNING and c.status == "failed"
    )
    return ReportSummary(
        total_checks=len(checks),
        passed=passed,
        failed=failed,
        errored=errored,
        skipped=skipped,
        warnings=warnings,
    )


def build_report(
    document: DocumentIdentity,
    tool_versions: ToolVersions,
    checks: list[CheckResult],
    *,
    elapsed_seconds: float = 0.0,
    timestamp: str | None = None,
) -> CompanionReport:
    """Assemble a full CompanionReport from its constituent parts.

    If *timestamp* is not supplied the current UTC time is used.
    The release state and summary are computed automatically from *checks*.
    A recommended action string is derived from the release state.
    """
    release_state = compute_release_state(checks)
    summary = compute_summary(checks)

    if timestamp is None:
        timestamp = datetime.now(timezone.utc).isoformat()

    recommended_action = _recommended_action(release_state, summary)

    return CompanionReport(
        timestamp=timestamp,
        document=document,
        tool_versions=tool_versions,
        release_state=release_state,
        checks=checks,
        summary=summary,
        recommended_action=recommended_action,
        elapsed_seconds=elapsed_seconds,
    )


def report_to_markdown(report: CompanionReport) -> str:
    """Render a CompanionReport as clean Markdown.

    Sections:
    - Document Identity
    - Release State
    - Summary (table)
    - Failed Checks
    - Warnings
    - Recommended Action
    - Tool Versions
    - Timing
    """
    lines: list[str] = []

    lines.append("# Companion Report")
    lines.append("")

    # --- Document Identity ---
    lines.append("## Document Identity")
    lines.append("")
    doc = report.document
    lines.append(f"- **Path:** `{doc.path}`")
    lines.append(f"- **SHA-256:** `{doc.sha256}`")
    lines.append(f"- **Size:** {doc.size_bytes:,} bytes")
    lines.append(f"- **Parts:** {doc.part_count}")
    lines.append("")

    # --- Release State ---
    lines.append("## Release State")
    lines.append("")
    lines.append(f"**{report.release_state.value}**")
    lines.append("")

    # --- Summary table ---
    lines.append("## Summary")
    lines.append("")
    s = report.summary
    lines.append("| Metric | Count |")
    lines.append("|--------|------:|")
    lines.append(f"| Total checks | {s.total_checks} |")
    lines.append(f"| Passed | {s.passed} |")
    lines.append(f"| Failed | {s.failed} |")
    lines.append(f"| Errored | {s.errored} |")
    lines.append(f"| Skipped | {s.skipped} |")
    lines.append(f"| Warnings | {s.warnings} |")
    lines.append("")

    # --- Failed Checks ---
    failed = [c for c in report.checks if c.status == "failed" and c.severity == Severity.ERROR]
    lines.append("## Failed Checks")
    lines.append("")
    if failed:
        for c in failed:
            lines.append(f"### {c.check_id}: {c.name}")
            lines.append("")
            lines.append(f"- **Severity:** {c.severity.value}")
            lines.append(f"- **Message:** {c.message}")
            if c.evidence:
                lines.append("- **Evidence:**")
                for ev in c.evidence:
                    loc = f" at `{ev.location}`" if ev.location else ""
                    lines.append(f"  - [{ev.evidence_class.value}] {ev.detail}{loc} (source: {ev.source})")
            lines.append("")
    else:
        lines.append("No failed checks.")
        lines.append("")

    # --- Warnings ---
    warns = [c for c in report.checks if c.severity == Severity.WARNING and c.status == "failed"]
    lines.append("## Warnings")
    lines.append("")
    if warns:
        for c in warns:
            lines.append(f"- **{c.check_id}:** {c.message}")
        lines.append("")
    else:
        lines.append("No warnings.")
        lines.append("")

    # --- Recommended Action ---
    lines.append("## Recommended Action")
    lines.append("")
    lines.append(report.recommended_action or "None.")
    lines.append("")

    # --- Tool Versions ---
    lines.append("## Tool Versions")
    lines.append("")
    tv = report.tool_versions
    lines.append(f"- **companion:** {tv.companion}")
    lines.append(f"- **python:** {tv.python}")
    if tv.docli:
        lines.append(f"- **docli:** {tv.docli}")
    if tv.lxml:
        lines.append(f"- **lxml:** {tv.lxml}")
    lines.append("")

    # --- Timing ---
    lines.append("## Timing")
    lines.append("")
    lines.append(f"- **Timestamp:** {report.timestamp}")
    lines.append(f"- **Elapsed:** {report.elapsed_seconds:.3f}s")
    lines.append("")

    return "\n".join(lines)


def write_artifact_bundle(
    report: CompanionReport,
    output_dir: Path,
    *,
    inspect_json: str | None = None,
    validate_json: str | None = None,
) -> ArtifactBundle:
    """Write report.json, report.md, and optional sidecar files to *output_dir*.

    Creates *output_dir* if it does not exist.  Returns an ArtifactBundle
    describing the paths that were written.
    """
    output_dir.mkdir(parents=True, exist_ok=True)

    report_json_path = output_dir / "report.json"
    report_json_path.write_text(
        report.model_dump_json(indent=2), encoding="utf-8"
    )

    report_md_path = output_dir / "report.md"
    report_md_path.write_text(report_to_markdown(report), encoding="utf-8")

    bundle = ArtifactBundle(
        report_json=str(report_json_path),
        report_md=str(report_md_path),
    )

    if inspect_json is not None:
        p = output_dir / "inspect.json"
        p.write_text(inspect_json, encoding="utf-8")
        bundle.inspect_json = str(p)

    if validate_json is not None:
        p = output_dir / "validate.json"
        p.write_text(validate_json, encoding="utf-8")
        bundle.validate_json = str(p)

    return bundle


# ---------------------------------------------------------------------------
# Helpers (private)
# ---------------------------------------------------------------------------


def _recommended_action(state: ReleaseState, summary: ReportSummary) -> str:
    """Return a human-readable recommended action string."""
    if state == ReleaseState.PASS:
        return "Document is ready for release."
    if state == ReleaseState.PASS_WITH_WARNINGS:
        return (
            f"Document may be released, but review {summary.warnings} "
            f"warning(s) first."
        )
    if state == ReleaseState.MANUAL_REVIEW:
        return (
            f"{summary.errored} check(s) errored. Manual review is required "
            "before release."
        )
    # FAIL
    return (
        f"{summary.failed} check(s) failed. Fix all errors before release."
    )
