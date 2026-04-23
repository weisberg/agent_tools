"""Propose-fix engine: diagnose document issues and emit docli Job YAML.

The Python companion diagnoses; docli executes. This module produces a
structured remediation plan that compiles to the same Job YAML format
that ``docli run`` consumes.
"""

from __future__ import annotations

import io
import json
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable, Sequence

# ---------------------------------------------------------------------------
# YAML serialisation — prefer PyYAML when available, fall back to a minimal
# built-in serialiser that covers the subset needed by docli Job files.
# ---------------------------------------------------------------------------

try:
    import yaml as _yaml  # type: ignore[import-untyped]

    def _dump_yaml(data: Any) -> str:
        return _yaml.dump(data, default_flow_style=False, sort_keys=False, allow_unicode=True)

except ImportError:  # pragma: no cover — tested via the pyyaml path
    _yaml = None  # type: ignore[assignment]

    def _dump_yaml(data: Any) -> str:  # type: ignore[misc]
        """Minimal YAML serialiser for docli Job dicts."""
        buf = io.StringIO()
        _write_yaml(buf, data, indent=0)
        return buf.getvalue()

    def _write_yaml(buf: io.StringIO, obj: Any, indent: int) -> None:
        prefix = "  " * indent
        if isinstance(obj, dict):
            for key, value in obj.items():
                if isinstance(value, (dict, list)):
                    buf.write(f"{prefix}{key}:\n")
                    _write_yaml(buf, value, indent + 1)
                else:
                    buf.write(f"{prefix}{key}: {_scalar(value)}\n")
        elif isinstance(obj, list):
            for item in obj:
                if isinstance(item, dict):
                    first = True
                    for key, value in item.items():
                        dash = "- " if first else "  "
                        first = False
                        if isinstance(value, (dict, list)):
                            buf.write(f"{prefix}{dash}{key}:\n")
                            _write_yaml(buf, value, indent + 2)
                        else:
                            buf.write(f"{prefix}{dash}{key}: {_scalar(value)}\n")
                else:
                    buf.write(f"{prefix}- {_scalar(item)}\n")
        else:
            buf.write(f"{prefix}{_scalar(obj)}\n")

    def _scalar(v: Any) -> str:
        if v is None:
            return "null"
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, (int, float)):
            return str(v)
        s = str(v)
        # Quote strings that could be misread by a YAML parser.
        if s == "" or any(ch in s for ch in ":{}[]#&*!|>'\"%@`?,\n"):
            return json.dumps(s)
        return s


# ---------------------------------------------------------------------------
# Severity
# ---------------------------------------------------------------------------


class Severity(str, Enum):
    """How urgent is the proposed fix."""

    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


# ---------------------------------------------------------------------------
# ProposedOperation
# ---------------------------------------------------------------------------


@dataclass
class ProposedOperation:
    """One atomic fix that maps to a single docli operation.

    Attributes:
        op:          The docli operation verb, e.g. ``"finalize.strip"``.
        description: Human-readable explanation of *why* this fix is proposed.
        severity:    How critical the issue is.
        params:      Remaining key/value pairs forwarded as-is into the Job
                     YAML entry (``target``, ``content``, ``position``, …).
    """

    op: str
    description: str
    severity: Severity = Severity.WARNING
    params: dict[str, Any] = field(default_factory=dict)

    # -- Convenience helpers ------------------------------------------------

    def to_operation_dict(self) -> dict[str, Any]:
        """Return the dict for a single ``operations`` list entry."""
        entry: dict[str, Any] = {"op": self.op}
        entry.update(self.params)
        return entry


# ---------------------------------------------------------------------------
# FixProposer
# ---------------------------------------------------------------------------


# Type alias for a built-in proposer function.
ProposerFn = Callable[[dict[str, Any]], Sequence[ProposedOperation]]


class FixProposer:
    """Aggregates proposer functions, runs them, and serialises the result.

    Usage::

        proposer = FixProposer()
        proposer.register(propose_strip_tracked_changes)
        proposer.register(propose_strip_comments)
        proposer.register(propose_insert_missing_sections)

        ops = proposer.propose(inspect_envelope)
        yaml_text = proposer.to_yaml(ops)
    """

    def __init__(self) -> None:
        self._proposers: list[ProposerFn] = []

    # -- Registration -------------------------------------------------------

    def register(self, fn: ProposerFn) -> None:
        """Register a proposer function."""
        self._proposers.append(fn)

    # -- Core API -----------------------------------------------------------

    def propose(self, inspect_data: dict[str, Any]) -> list[ProposedOperation]:
        """Run all registered proposers and return a flat list of operations."""
        results: list[ProposedOperation] = []
        for fn in self._proposers:
            results.extend(fn(inspect_data))
        return results

    @staticmethod
    def to_docli_job(operations: Sequence[ProposedOperation]) -> dict[str, Any]:
        """Convert a sequence of proposed operations to a docli Job dict.

        The returned dict matches the shape consumed by ``docli run``:

        .. code-block:: yaml

            operations:
              - op: finalize.strip
              - op: edit.insert
                target: { heading: "Introduction" }
                position: after
                content:
                  - "New section body."
        """
        return {
            "operations": [op.to_operation_dict() for op in operations],
        }

    @staticmethod
    def to_yaml(operations: Sequence[ProposedOperation]) -> str:
        """Serialise proposed operations to valid docli Job YAML."""
        job = FixProposer.to_docli_job(operations)
        return _dump_yaml(job)


# ---------------------------------------------------------------------------
# Built-in proposer functions
# ---------------------------------------------------------------------------


def propose_strip_tracked_changes(
    inspect_data: dict[str, Any],
) -> list[ProposedOperation]:
    """Propose ``finalize.strip`` if tracked changes remain in the document.

    Looks for a ``tracked_changes`` (or ``trackedChanges``) key with a
    non-empty list in *inspect_data*.
    """
    changes = inspect_data.get("tracked_changes") or inspect_data.get("trackedChanges") or []
    if not changes:
        return []
    return [
        ProposedOperation(
            op="finalize.strip",
            description=f"Strip {len(changes)} tracked change(s) before release.",
            severity=Severity.ERROR,
            params={},
        )
    ]


def propose_strip_comments(
    inspect_data: dict[str, Any],
) -> list[ProposedOperation]:
    """Propose ``edit.delete`` for each comment, or a bulk strip.

    If the inspect envelope contains ``comments`` with entries, emit a
    single ``finalize.strip`` (comments are removed together with tracked
    changes by docli's strip operation).  If there are comments but *no*
    tracked changes, emit targeted ``edit.delete`` operations for each
    comment anchor.
    """
    comments = inspect_data.get("comments") or []
    if not comments:
        return []

    # If tracked changes also present, a single finalize.strip covers both.
    tracked = inspect_data.get("tracked_changes") or inspect_data.get("trackedChanges") or []
    if tracked:
        # The strip proposer already covers this case.
        return []

    ops: list[ProposedOperation] = []
    for comment in comments:
        cid = comment.get("id", "?")
        author = comment.get("author", "unknown")
        ops.append(
            ProposedOperation(
                op="edit.delete",
                description=f"Remove comment {cid} by {author}.",
                severity=Severity.WARNING,
                params={"target": {"comment_id": cid}},
            )
        )
    return ops


def propose_insert_missing_sections(
    inspect_data: dict[str, Any],
    required_sections: Sequence[str] | None = None,
) -> list[ProposedOperation]:
    """Propose ``edit.insert`` for each missing required section.

    Parameters:
        inspect_data:      The ``docli inspect`` JSON envelope.
        required_sections: An ordered list of headings that must exist.
                           Defaults to ``["Introduction", "Methodology",
                           "Results", "Conclusion"]``.
    """
    if required_sections is None:
        required_sections = [
            "Introduction",
            "Methodology",
            "Results",
            "Conclusion",
        ]

    # Collect existing headings from the inspect envelope.
    headings_raw = inspect_data.get("headings") or []
    existing: set[str] = set()
    for h in headings_raw:
        if isinstance(h, dict):
            existing.add(h.get("text", ""))
        elif isinstance(h, str):
            existing.add(h)

    ops: list[ProposedOperation] = []
    prev_heading: str | None = None
    for section in required_sections:
        if section not in existing:
            target: dict[str, Any]
            position: str
            if prev_heading is not None:
                target = {"heading": prev_heading}
                position = "after"
            else:
                # Insert at the very beginning if no prior heading exists.
                target = {"paragraph": 0}
                position = "before"
            ops.append(
                ProposedOperation(
                    op="edit.insert",
                    description=f'Insert missing required section "{section}".',
                    severity=Severity.ERROR,
                    params={
                        "target": target,
                        "position": position,
                        "content": [f"TODO: write {section} section."],
                    },
                )
            )
        else:
            prev_heading = section

    return ops
