"""Tests for the propose-fix engine."""

from __future__ import annotations

import json

import pytest

from docli_companion.propose_fix import (
    FixProposer,
    ProposedOperation,
    Severity,
    propose_insert_missing_sections,
    propose_strip_comments,
    propose_strip_tracked_changes,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_inspect(**kwargs) -> dict:
    """Build a minimal ``docli inspect`` envelope with given overrides."""
    base: dict = {
        "headings": [],
        "tracked_changes": [],
        "comments": [],
    }
    base.update(kwargs)
    return base


# ---------------------------------------------------------------------------
# ProposedOperation
# ---------------------------------------------------------------------------


class TestProposedOperation:
    def test_to_operation_dict_basic(self):
        op = ProposedOperation(
            op="finalize.strip",
            description="Strip tracked changes.",
            severity=Severity.ERROR,
        )
        d = op.to_operation_dict()
        assert d == {"op": "finalize.strip"}

    def test_to_operation_dict_with_params(self):
        op = ProposedOperation(
            op="edit.insert",
            description="Insert section.",
            params={"target": {"heading": "Intro"}, "position": "after", "content": ["text"]},
        )
        d = op.to_operation_dict()
        assert d["op"] == "edit.insert"
        assert d["target"] == {"heading": "Intro"}
        assert d["position"] == "after"
        assert d["content"] == ["text"]


# ---------------------------------------------------------------------------
# propose_strip_tracked_changes
# ---------------------------------------------------------------------------


class TestProposeStripTrackedChanges:
    def test_no_changes_returns_empty(self):
        data = _make_inspect(tracked_changes=[])
        assert propose_strip_tracked_changes(data) == []

    def test_with_changes_returns_strip(self):
        data = _make_inspect(tracked_changes=[{"id": 1}, {"id": 2}])
        ops = propose_strip_tracked_changes(data)
        assert len(ops) == 1
        assert ops[0].op == "finalize.strip"
        assert ops[0].severity == Severity.ERROR
        assert "2" in ops[0].description

    def test_camel_case_key(self):
        """Supports the camelCase variant ``trackedChanges``."""
        data = {"trackedChanges": [{"id": 1}], "comments": []}
        ops = propose_strip_tracked_changes(data)
        assert len(ops) == 1


# ---------------------------------------------------------------------------
# propose_strip_comments
# ---------------------------------------------------------------------------


class TestProposeStripComments:
    def test_no_comments_returns_empty(self):
        data = _make_inspect(comments=[])
        assert propose_strip_comments(data) == []

    def test_comments_without_tracked_changes(self):
        data = _make_inspect(
            comments=[
                {"id": 10, "author": "Alice"},
                {"id": 11, "author": "Bob"},
            ]
        )
        ops = propose_strip_comments(data)
        assert len(ops) == 2
        assert all(o.op == "edit.delete" for o in ops)
        assert ops[0].params["target"]["comment_id"] == 10
        assert ops[1].params["target"]["comment_id"] == 11

    def test_comments_with_tracked_changes_defers(self):
        """When tracked changes also present, strip proposer handles both."""
        data = _make_inspect(
            tracked_changes=[{"id": 1}],
            comments=[{"id": 10, "author": "Alice"}],
        )
        ops = propose_strip_comments(data)
        assert ops == []


# ---------------------------------------------------------------------------
# propose_insert_missing_sections
# ---------------------------------------------------------------------------


class TestProposeInsertMissingSections:
    def test_all_present_returns_empty(self):
        data = _make_inspect(
            headings=[
                {"text": "Introduction"},
                {"text": "Methodology"},
                {"text": "Results"},
                {"text": "Conclusion"},
            ]
        )
        ops = propose_insert_missing_sections(data)
        assert ops == []

    def test_missing_sections_proposed(self):
        data = _make_inspect(
            headings=[
                {"text": "Introduction"},
                {"text": "Conclusion"},
            ]
        )
        ops = propose_insert_missing_sections(data)
        assert len(ops) == 2
        names = [o.description for o in ops]
        assert any("Methodology" in n for n in names)
        assert any("Results" in n for n in names)
        # Both should insert after the last existing heading before them.
        assert ops[0].params["target"] == {"heading": "Introduction"}
        assert ops[0].params["position"] == "after"

    def test_custom_required_sections(self):
        data = _make_inspect(headings=[{"text": "Abstract"}])
        ops = propose_insert_missing_sections(
            data,
            required_sections=["Abstract", "References"],
        )
        assert len(ops) == 1
        assert "References" in ops[0].description

    def test_all_missing_first_uses_paragraph_0(self):
        data = _make_inspect(headings=[])
        ops = propose_insert_missing_sections(data, required_sections=["A", "B"])
        assert ops[0].params["target"] == {"paragraph": 0}
        assert ops[0].params["position"] == "before"

    def test_string_headings_supported(self):
        """Inspect envelopes may contain headings as plain strings."""
        data = _make_inspect(headings=["Introduction", "Conclusion"])
        ops = propose_insert_missing_sections(data)
        assert len(ops) == 2


# ---------------------------------------------------------------------------
# FixProposer integration
# ---------------------------------------------------------------------------


class TestFixProposer:
    def test_register_and_propose(self):
        fp = FixProposer()
        fp.register(propose_strip_tracked_changes)
        fp.register(propose_strip_comments)
        fp.register(propose_insert_missing_sections)

        data = _make_inspect(
            tracked_changes=[{"id": 1}],
            comments=[{"id": 10, "author": "Eve"}],
            headings=[{"text": "Introduction"}],
        )
        ops = fp.propose(data)
        # 1 strip + 0 comment (deferred) + 3 missing sections
        assert len(ops) == 4
        assert ops[0].op == "finalize.strip"

    def test_to_docli_job_structure(self):
        ops = [
            ProposedOperation(op="finalize.strip", description="d"),
            ProposedOperation(
                op="edit.insert",
                description="d",
                params={"target": {"heading": "Intro"}, "position": "after", "content": ["x"]},
            ),
        ]
        job = FixProposer.to_docli_job(ops)
        assert "operations" in job
        assert len(job["operations"]) == 2
        assert job["operations"][0] == {"op": "finalize.strip"}
        assert job["operations"][1]["op"] == "edit.insert"

    def test_to_yaml_is_valid_yaml(self):
        """The YAML output must be parseable and contain the right keys."""
        ops = [
            ProposedOperation(op="finalize.strip", description="d"),
            ProposedOperation(
                op="edit.replace",
                description="d",
                params={"target": {"paragraph": 3}, "content": "Updated."},
            ),
        ]
        yaml_text = FixProposer.to_yaml(ops)

        # Must be parseable — we rely on PyYAML here since it is a dependency.
        import yaml

        parsed = yaml.safe_load(yaml_text)
        assert parsed["operations"][0]["op"] == "finalize.strip"
        assert parsed["operations"][1]["op"] == "edit.replace"
        assert parsed["operations"][1]["target"] == {"paragraph": 3}
        assert parsed["operations"][1]["content"] == "Updated."

    def test_empty_operations(self):
        job = FixProposer.to_docli_job([])
        assert job == {"operations": []}
        yaml_text = FixProposer.to_yaml([])
        assert "operations" in yaml_text
