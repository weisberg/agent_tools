"""Command-line entrypoint for docli-companion."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import sys
import time
import zipfile
from pathlib import Path
from typing import Any

from .batch import BatchConfig, audit_batch, batch_summary_markdown, batch_to_csv, batch_to_json
from .checks.policy import PolicyEngine
from .checks.structural import run_all_structural
from .models import CheckResult, DocumentIdentity, Evidence, EvidenceClass, Severity, ToolVersions
from .propose_fix import (
    FixProposer,
    propose_insert_missing_sections,
    propose_strip_comments,
    propose_strip_tracked_changes,
)
from .report import build_report, write_artifact_bundle
from .runner import DocliRunner
from .schema import validate_inspect_output, validate_validate_output

VERSION = "0.1.0"


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except Exception as exc:  # noqa: BLE001
        print(json.dumps({"ok": False, "error": str(exc)}), file=sys.stderr)
        return 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="docli-companion",
        description="QA and release gates around the docli DOCX CLI.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {VERSION}")
    sub = parser.add_subparsers(required=True)

    final = sub.add_parser("final-check", help="Run release checks for one DOCX.")
    final.add_argument("document", type=Path)
    final.add_argument("--out", type=Path, required=True, help="Artifact output directory.")
    final.add_argument("--docli", type=Path, help="Path to docli binary.")
    final.add_argument("--policy", type=Path, help="Optional policy YAML.")
    final.add_argument("--timeout", type=float, default=30.0)
    final.set_defaults(func=cmd_final_check)

    batch = sub.add_parser("batch-audit", help="Audit a directory of DOCX files.")
    batch.add_argument("root", type=Path)
    batch.add_argument("--glob", default="**/*.docx")
    batch.add_argument("--max-workers", type=int, default=4)
    batch.add_argument("--format", choices=["json", "csv", "md"], default="json")
    batch.set_defaults(func=cmd_batch_audit)

    policy = sub.add_parser("policy-check", help="Run a policy YAML against inspect JSON.")
    policy.add_argument("inspect_json", type=Path)
    policy.add_argument("--policy", type=Path, required=True)
    policy.set_defaults(func=cmd_policy_check)

    fix = sub.add_parser("propose-fix", help="Emit a docli job YAML from inspect JSON.")
    fix.add_argument("inspect_json", type=Path)
    fix.add_argument("--out", type=Path)
    fix.set_defaults(func=cmd_propose_fix)

    return parser


def cmd_final_check(args: argparse.Namespace) -> int:
    started = time.monotonic()
    runner = DocliRunner(docli_path=args.docli, default_timeout=args.timeout)

    inspect_payload = runner.inspect(args.document)
    validate_payload = runner.validate(args.document)
    checks = collect_final_checks(args.document, inspect_payload, validate_payload, args.policy)

    report = build_report(
        document=document_identity(args.document),
        tool_versions=tool_versions(runner),
        checks=checks,
        elapsed_seconds=time.monotonic() - started,
    )
    bundle = write_artifact_bundle(
        report,
        args.out,
        inspect_json=json.dumps(inspect_payload, indent=2),
        validate_json=json.dumps(validate_payload, indent=2),
    )
    print(
        json.dumps(
            {
                "ok": report.release_state.value in {"pass", "pass-with-warnings"},
                "release_state": report.release_state.value,
                "summary": report.summary.model_dump(),
                "recommended_action": report.recommended_action,
                "artifacts": bundle.model_dump(),
            },
            indent=2,
        )
    )
    return 0 if report.release_state.value in {"pass", "pass-with-warnings"} else 2


def cmd_batch_audit(args: argparse.Namespace) -> int:
    results = audit_batch(
        BatchConfig(
            root_dir=args.root,
            glob_pattern=args.glob,
            max_workers=args.max_workers,
        )
    )
    if args.format == "csv":
        print(batch_to_csv(results), end="")
    elif args.format == "md":
        print(batch_summary_markdown(results))
    else:
        print(batch_to_json(results))
    return 0 if all(result.valid for result in results) else 2


def cmd_policy_check(args: argparse.Namespace) -> int:
    payload = json.loads(args.inspect_json.read_text(encoding="utf-8"))
    data = payload.get("data", payload)
    result = PolicyEngine.from_yaml(args.policy).evaluate(data)
    print(
        json.dumps(
            {
                "ok": result.passed,
                "violations": [
                    {
                        "rule": violation.rule,
                        "message": violation.message,
                        "severity": violation.severity.value,
                    }
                    for violation in result.violations
                ],
            },
            indent=2,
        )
    )
    return 0 if result.passed else 2


def cmd_propose_fix(args: argparse.Namespace) -> int:
    payload = json.loads(args.inspect_json.read_text(encoding="utf-8"))
    data = payload.get("data", payload)
    proposer = make_default_proposer()
    yaml_text = proposer.to_yaml(proposer.propose(data))
    if args.out:
        args.out.write_text(yaml_text, encoding="utf-8")
    else:
        print(yaml_text, end="")
    return 0


def make_default_proposer() -> FixProposer:
    proposer = FixProposer()
    proposer.register(propose_strip_tracked_changes)
    proposer.register(propose_strip_comments)
    proposer.register(propose_insert_missing_sections)
    return proposer


def collect_final_checks(
    document: Path,
    inspect_payload: dict[str, Any],
    validate_payload: dict[str, Any],
    policy_path: Path | None,
) -> list[CheckResult]:
    checks: list[CheckResult] = []
    checks.extend(schema_checks(inspect_payload, validate_payload))
    checks.extend(structural_checks(document))
    checks.extend(validate_checks(validate_payload))
    if policy_path:
        checks.extend(policy_checks(inspect_payload.get("data", {}), policy_path))
    return checks


def schema_checks(
    inspect_payload: dict[str, Any],
    validate_payload: dict[str, Any],
) -> list[CheckResult]:
    checks = []
    for check_id, name, payload, validator in [
        ("SCHEMA-INSPECT", "inspect envelope schema", inspect_payload, validate_inspect_output),
        ("SCHEMA-VALIDATE", "validate envelope schema", validate_payload, validate_validate_output),
    ]:
        try:
            validator(payload)
            checks.append(
                CheckResult(
                    check_id=check_id,
                    name=name,
                    severity=Severity.ERROR,
                    status="passed",
                    message="Envelope matches schema.",
                )
            )
        except Exception as exc:  # noqa: BLE001
            checks.append(
                CheckResult(
                    check_id=check_id,
                    name=name,
                    severity=Severity.ERROR,
                    status="failed",
                    message=str(exc),
                )
            )
    return checks


def structural_checks(document: Path) -> list[CheckResult]:
    checks = []
    for item in run_all_structural(document):
        status = "passed" if item["status"] == "pass" else "failed"
        severity = severity_from_string(item.get("severity", "error"))
        evidence = Evidence(
            evidence_class=EvidenceClass.STRUCTURAL,
            source="docli-companion",
            detail=item["message"],
        )
        checks.append(
            CheckResult(
                check_id=item["check_id"],
                name=item["name"],
                severity=severity,
                status=status,
                message=item["message"],
                evidence=[evidence],
            )
        )
    return checks


def validate_checks(validate_payload: dict[str, Any]) -> list[CheckResult]:
    data = validate_payload.get("data", {})
    error_count = data.get("error_count")
    warning_count = data.get("warning_count", 0)
    if error_count == 0:
        return [
            CheckResult(
                check_id="DOCLI-VALIDATE",
                name="docli validate",
                severity=Severity.ERROR,
                status="passed",
                message=f"docli validate reported 0 errors and {warning_count} warnings.",
            )
        ]
    return [
        CheckResult(
            check_id="DOCLI-VALIDATE",
            name="docli validate",
            severity=Severity.ERROR,
            status="failed",
            message=f"docli validate reported {error_count!r} errors.",
            evidence=[
                Evidence(
                    evidence_class=EvidenceClass.STRUCTURAL,
                    source="docli validate",
                    detail=json.dumps(data, sort_keys=True),
                )
            ],
        )
    ]


def policy_checks(inspect_data: dict[str, Any], policy_path: Path) -> list[CheckResult]:
    result = PolicyEngine.from_yaml(policy_path).evaluate(inspect_data)
    if result.passed and not result.violations:
        return [
            CheckResult(
                check_id="POLICY",
                name="policy checks",
                severity=Severity.ERROR,
                status="passed",
                message="All configured policy checks passed.",
            )
        ]
    checks = []
    for idx, violation in enumerate(result.violations, start=1):
        checks.append(
            CheckResult(
                check_id=f"POLICY-{idx:03d}",
                name=violation.rule,
                severity=severity_from_string(violation.severity.value),
                status="failed",
                message=violation.message,
                evidence=[
                    Evidence(
                        evidence_class=EvidenceClass.POLICY,
                        source=str(policy_path),
                        detail=violation.message,
                    )
                ],
            )
        )
    return checks


def document_identity(document: Path) -> DocumentIdentity:
    data = document.read_bytes()
    part_count = 0
    if zipfile.is_zipfile(document):
        with zipfile.ZipFile(document, "r") as zf:
            part_count = len(zf.namelist())
    return DocumentIdentity(
        path=str(document),
        sha256=hashlib.sha256(data).hexdigest(),
        size_bytes=len(data),
        part_count=part_count,
    )


def tool_versions(runner: DocliRunner) -> ToolVersions:
    docli_version = None
    try:
        result = runner.call(["--version"], check=False)
        docli_version = result.stdout.strip() or None
    except Exception:  # noqa: BLE001
        docli_version = None
    try:
        import lxml  # type: ignore[import-untyped]

        lxml_version = lxml.__version__
    except Exception:  # noqa: BLE001
        lxml_version = None
    return ToolVersions(
        companion=VERSION,
        python=platform.python_version(),
        docli=docli_version,
        lxml=lxml_version,
    )


def severity_from_string(value: str) -> Severity:
    if value == "warning":
        return Severity.WARNING
    if value == "info" or value == "pass":
        return Severity.INFO
    return Severity.ERROR


if __name__ == "__main__":
    raise SystemExit(main())
