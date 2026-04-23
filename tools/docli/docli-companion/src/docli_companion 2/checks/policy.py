"""Policy engine for docli-companion.

Operates on ``docli inspect`` JSON output dicts and evaluates configurable
policy rules.  Rules are expressed as a YAML configuration and loaded into
a :class:`PolicyEngine` that returns structured pass/fail results.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any

import yaml  # type: ignore[import-untyped]


# ---------------------------------------------------------------------------
# Result types
# ---------------------------------------------------------------------------

class Severity(str, Enum):
    """Severity level for a policy violation."""

    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


@dataclass(frozen=True)
class Violation:
    """A single policy violation."""

    rule: str
    message: str
    severity: Severity = Severity.ERROR


@dataclass
class PolicyResult:
    """Aggregated result of running the policy engine."""

    passed: bool
    violations: list[Violation] = field(default_factory=list)

    @property
    def errors(self) -> list[Violation]:
        return [v for v in self.violations if v.severity is Severity.ERROR]

    @property
    def warnings(self) -> list[Violation]:
        return [v for v in self.violations if v.severity is Severity.WARNING]


# ---------------------------------------------------------------------------
# Individual rule implementations
# ---------------------------------------------------------------------------

def _check_required_sections(
    data: dict[str, Any],
    params: dict[str, Any],
) -> list[Violation]:
    """Ensure that specified headings exist in the document."""
    required: list[str] = params.get("sections", [])
    if not required:
        return []

    headings = data.get("headings", [])
    heading_texts = [h.get("text", "") for h in headings]

    case_sensitive: bool = params.get("case_sensitive", False)
    if not case_sensitive:
        heading_texts = [t.lower() for t in heading_texts]

    violations: list[Violation] = []
    for section in required:
        needle = section if case_sensitive else section.lower()
        if needle not in heading_texts:
            violations.append(
                Violation(
                    rule="required_sections",
                    message=f"Required section missing: '{section}'",
                )
            )
    return violations


def _check_heading_hierarchy(
    data: dict[str, Any],
    _params: dict[str, Any],
) -> list[Violation]:
    """Ensure headings do not skip levels (e.g. H1 -> H3)."""
    headings = data.get("headings", [])
    if not headings:
        return []

    violations: list[Violation] = []
    prev_level: int = 0
    for h in headings:
        level: int = h.get("level", 1)
        if prev_level > 0 and level > prev_level + 1:
            violations.append(
                Violation(
                    rule="heading_hierarchy",
                    message=(
                        f"Heading '{h.get('text', '')}' at level {level} "
                        f"skips from level {prev_level}"
                    ),
                )
            )
        prev_level = level
    return violations


def _check_prohibited_terms(
    data: dict[str, Any],
    params: dict[str, Any],
) -> list[Violation]:
    """Flag prohibited terms found in heading text.

    A lightweight check operating on the inspect output.  For full-text
    scanning, the engine would need the ``docli read`` output as well.
    """
    terms: list[str] = params.get("terms", [])
    if not terms:
        return []

    headings = data.get("headings", [])
    heading_texts = [h.get("text", "") for h in headings]
    all_text = " ".join(heading_texts)

    case_sensitive: bool = params.get("case_sensitive", False)
    if not case_sensitive:
        all_text = all_text.lower()

    violations: list[Violation] = []
    for term in terms:
        needle = term if case_sensitive else term.lower()
        if needle in all_text:
            severity = Severity(params.get("severity", "error"))
            violations.append(
                Violation(
                    rule="prohibited_terms",
                    message=f"Prohibited term found: '{term}'",
                    severity=severity,
                )
            )
    return violations


def _check_metadata_present(
    data: dict[str, Any],
    params: dict[str, Any],
) -> list[Violation]:
    """Verify that certain top-level metadata keys exist in the inspect output."""
    required_keys: list[str] = params.get("keys", [
        "file",
        "file_size_bytes",
        "word_count",
    ])

    violations: list[Violation] = []
    for key in required_keys:
        if key not in data:
            violations.append(
                Violation(
                    rule="metadata_present",
                    message=f"Missing metadata key: '{key}'",
                )
            )
    return violations


def _check_no_tracked_changes(
    data: dict[str, Any],
    _params: dict[str, Any],
) -> list[Violation]:
    """Document must have zero tracked changes."""
    tc = data.get("tracked_changes", {})
    count = tc.get("count", 0) if isinstance(tc, dict) else 0
    if count > 0:
        return [
            Violation(
                rule="no_tracked_changes",
                message=f"Document has {count} tracked change(s)",
            )
        ]
    return []


def _check_no_comments(
    data: dict[str, Any],
    _params: dict[str, Any],
) -> list[Violation]:
    """Document must have zero unresolved comments."""
    comments = data.get("comments", {})
    unresolved = comments.get("unresolved", 0) if isinstance(comments, dict) else 0
    if unresolved > 0:
        return [
            Violation(
                rule="no_comments",
                message=f"Document has {unresolved} unresolved comment(s)",
            )
        ]
    return []


def _check_max_heading_depth(
    data: dict[str, Any],
    params: dict[str, Any],
) -> list[Violation]:
    """No heading should exceed a specified depth."""
    max_depth: int = params.get("max_depth", 3)
    headings = data.get("headings", [])

    violations: list[Violation] = []
    for h in headings:
        level: int = h.get("level", 1)
        if level > max_depth:
            violations.append(
                Violation(
                    rule="max_heading_depth",
                    message=(
                        f"Heading '{h.get('text', '')}' at level {level} "
                        f"exceeds max depth {max_depth}"
                    ),
                )
            )
    return violations


# ---------------------------------------------------------------------------
# Rule registry
# ---------------------------------------------------------------------------

_RULE_REGISTRY: dict[str, Any] = {
    "required_sections": _check_required_sections,
    "heading_hierarchy": _check_heading_hierarchy,
    "prohibited_terms": _check_prohibited_terms,
    "metadata_present": _check_metadata_present,
    "no_tracked_changes": _check_no_tracked_changes,
    "no_comments": _check_no_comments,
    "max_heading_depth": _check_max_heading_depth,
}


# ---------------------------------------------------------------------------
# PolicyEngine
# ---------------------------------------------------------------------------

class PolicyEngine:
    """Configurable policy engine that evaluates rules against inspect data.

    Parameters
    ----------
    rules:
        A list of rule dicts, each with at least a ``"name"`` key
        matching a registered rule.  Additional keys are forwarded as
        parameters to the rule function.
    """

    def __init__(self, rules: list[dict[str, Any]] | None = None) -> None:
        self.rules: list[dict[str, Any]] = rules or []

    # -- Factory helpers ----------------------------------------------------

    @classmethod
    def from_yaml(cls, path: str | Path) -> "PolicyEngine":
        """Load rules from a YAML configuration file.

        Expected format::

            rules:
              - name: required_sections
                sections:
                  - Executive Summary
                  - Methodology
              - name: heading_hierarchy
              - name: no_tracked_changes
        """
        path = Path(path)
        with path.open("r") as fh:
            cfg = yaml.safe_load(fh)

        if not isinstance(cfg, dict) or "rules" not in cfg:
            raise ValueError(
                f"Policy config at {path} must be a YAML mapping with a 'rules' key"
            )

        return cls(rules=cfg["rules"])

    @classmethod
    def from_dict(cls, cfg: dict[str, Any]) -> "PolicyEngine":
        """Create an engine from an in-memory config dict."""
        if "rules" not in cfg:
            raise ValueError("Config dict must contain a 'rules' key")
        return cls(rules=cfg["rules"])

    # -- Execution ----------------------------------------------------------

    def evaluate(self, inspect_data: dict[str, Any]) -> PolicyResult:
        """Run all configured rules against *inspect_data*.

        *inspect_data* should be the ``"data"`` value from a
        ``docli inspect`` JSON envelope.
        """
        all_violations: list[Violation] = []

        for rule_cfg in self.rules:
            name = rule_cfg.get("name")
            if name is None:
                continue

            handler = _RULE_REGISTRY.get(name)
            if handler is None:
                all_violations.append(
                    Violation(
                        rule=name,
                        message=f"Unknown rule: '{name}'",
                        severity=Severity.WARNING,
                    )
                )
                continue

            # Everything except "name" is forwarded as params.
            params = {k: v for k, v in rule_cfg.items() if k != "name"}
            all_violations.extend(handler(inspect_data, params))

        passed = not any(v.severity is Severity.ERROR for v in all_violations)
        return PolicyResult(passed=passed, violations=all_violations)
