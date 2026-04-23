"""Tests for the policy engine."""

from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from docli_companion.checks.policy import (
    PolicyEngine,
    PolicyResult,
    Severity,
    Violation,
)


# ---------------------------------------------------------------------------
# Shared fixtures
# ---------------------------------------------------------------------------

CLEAN_INSPECT: dict = {
    "file": "report.docx",
    "file_size_bytes": 48230,
    "page_size": "letter",
    "orientation": "portrait",
    "sections": 3,
    "paragraphs": {
        "count": 47,
        "by_style": {
            "Heading1": [0, 12, 28],
            "Heading2": [1, 5, 13, 19, 29, 35],
            "Normal": [2, 3, 4, 6, 7],
        },
    },
    "headings": [
        {"index": 0, "level": 1, "text": "Executive Summary"},
        {"index": 1, "level": 2, "text": "Key Findings"},
        {"index": 5, "level": 2, "text": "Methodology"},
        {"index": 12, "level": 1, "text": "Experiment Design"},
        {"index": 13, "level": 2, "text": "CUPED Variance Reduction"},
    ],
    "tables": [],
    "images": [],
    "comments": {"count": 0, "authors": [], "unresolved": 0},
    "tracked_changes": {"count": 0, "insertions": 0, "deletions": 0, "authors": []},
    "styles": ["Heading1", "Heading2", "Normal"],
    "fonts": ["Arial", "Calibri"],
    "word_count": 2847,
}


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestRequiredSections:
    def test_passes_when_all_present(self) -> None:
        engine = PolicyEngine(rules=[
            {"name": "required_sections", "sections": ["Executive Summary", "Methodology"]},
        ])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed
        assert result.violations == []

    def test_fails_when_section_missing(self) -> None:
        engine = PolicyEngine(rules=[
            {"name": "required_sections", "sections": ["Executive Summary", "Conclusion"]},
        ])
        result = engine.evaluate(CLEAN_INSPECT)
        assert not result.passed
        assert len(result.errors) == 1
        assert "Conclusion" in result.errors[0].message

    def test_case_insensitive_by_default(self) -> None:
        engine = PolicyEngine(rules=[
            {"name": "required_sections", "sections": ["executive summary"]},
        ])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed


class TestHeadingHierarchy:
    def test_passes_with_valid_hierarchy(self) -> None:
        engine = PolicyEngine(rules=[{"name": "heading_hierarchy"}])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_fails_on_skipped_level(self) -> None:
        bad_data = {
            **CLEAN_INSPECT,
            "headings": [
                {"index": 0, "level": 1, "text": "Intro"},
                {"index": 1, "level": 3, "text": "Deep Dive"},  # skips level 2
            ],
        }
        engine = PolicyEngine(rules=[{"name": "heading_hierarchy"}])
        result = engine.evaluate(bad_data)
        assert not result.passed
        assert any("skips" in v.message for v in result.violations)


class TestProhibitedTerms:
    def test_passes_when_no_prohibited_terms(self) -> None:
        engine = PolicyEngine(rules=[
            {"name": "prohibited_terms", "terms": ["DRAFT", "CONFIDENTIAL"]},
        ])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_fails_when_term_found(self) -> None:
        data = {
            **CLEAN_INSPECT,
            "headings": [
                {"index": 0, "level": 1, "text": "DRAFT Executive Summary"},
            ],
        }
        engine = PolicyEngine(rules=[
            {"name": "prohibited_terms", "terms": ["DRAFT"]},
        ])
        result = engine.evaluate(data)
        assert not result.passed
        assert result.errors[0].rule == "prohibited_terms"

    def test_severity_override(self) -> None:
        data = {
            **CLEAN_INSPECT,
            "headings": [{"index": 0, "level": 1, "text": "DRAFT Plan"}],
        }
        engine = PolicyEngine(rules=[
            {"name": "prohibited_terms", "terms": ["DRAFT"], "severity": "warning"},
        ])
        result = engine.evaluate(data)
        # Warnings do not cause failure.
        assert result.passed
        assert len(result.warnings) == 1


class TestMetadataPresent:
    def test_passes_with_defaults(self) -> None:
        engine = PolicyEngine(rules=[{"name": "metadata_present"}])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_fails_when_key_missing(self) -> None:
        engine = PolicyEngine(rules=[
            {"name": "metadata_present", "keys": ["file", "nonexistent_key"]},
        ])
        result = engine.evaluate(CLEAN_INSPECT)
        assert not result.passed
        assert "nonexistent_key" in result.errors[0].message


class TestNoTrackedChanges:
    def test_passes_when_clean(self) -> None:
        engine = PolicyEngine(rules=[{"name": "no_tracked_changes"}])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_fails_with_tracked_changes(self) -> None:
        data = {
            **CLEAN_INSPECT,
            "tracked_changes": {"count": 3, "insertions": 2, "deletions": 1, "authors": ["Claude"]},
        }
        engine = PolicyEngine(rules=[{"name": "no_tracked_changes"}])
        result = engine.evaluate(data)
        assert not result.passed


class TestNoComments:
    def test_passes_when_clean(self) -> None:
        engine = PolicyEngine(rules=[{"name": "no_comments"}])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_fails_with_unresolved_comments(self) -> None:
        data = {
            **CLEAN_INSPECT,
            "comments": {"count": 2, "authors": ["Jane"], "unresolved": 2},
        }
        engine = PolicyEngine(rules=[{"name": "no_comments"}])
        result = engine.evaluate(data)
        assert not result.passed


class TestMaxHeadingDepth:
    def test_passes_within_limit(self) -> None:
        engine = PolicyEngine(rules=[{"name": "max_heading_depth", "max_depth": 3}])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_fails_when_exceeding_depth(self) -> None:
        data = {
            **CLEAN_INSPECT,
            "headings": [
                {"index": 0, "level": 1, "text": "Top"},
                {"index": 1, "level": 4, "text": "Too Deep"},
            ],
        }
        engine = PolicyEngine(rules=[{"name": "max_heading_depth", "max_depth": 3}])
        result = engine.evaluate(data)
        assert not result.passed
        assert "exceeds max depth" in result.errors[0].message


class TestYAMLConfig:
    def test_load_from_yaml(self, tmp_path: Path) -> None:
        config = tmp_path / "policy.yaml"
        config.write_text(textwrap.dedent("""\
            rules:
              - name: required_sections
                sections:
                  - Executive Summary
                  - Methodology
              - name: heading_hierarchy
              - name: no_tracked_changes
        """))
        engine = PolicyEngine.from_yaml(config)
        assert len(engine.rules) == 3

        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_invalid_yaml_config(self, tmp_path: Path) -> None:
        config = tmp_path / "bad.yaml"
        config.write_text("just_a_string\n")
        with pytest.raises(ValueError, match="must be a YAML mapping"):
            PolicyEngine.from_yaml(config)


class TestFromDict:
    def test_from_dict(self) -> None:
        cfg = {"rules": [{"name": "no_tracked_changes"}, {"name": "no_comments"}]}
        engine = PolicyEngine.from_dict(cfg)
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed

    def test_from_dict_missing_key(self) -> None:
        with pytest.raises(ValueError, match="'rules'"):
            PolicyEngine.from_dict({"not_rules": []})


class TestUnknownRule:
    def test_unknown_rule_produces_warning(self) -> None:
        engine = PolicyEngine(rules=[{"name": "nonexistent_rule"}])
        result = engine.evaluate(CLEAN_INSPECT)
        # Unknown rules produce a warning, not an error, so the result still passes.
        assert result.passed
        assert len(result.warnings) == 1
        assert "Unknown rule" in result.warnings[0].message


class TestCombinedPolicies:
    def test_multiple_rules_all_pass(self) -> None:
        engine = PolicyEngine(rules=[
            {"name": "required_sections", "sections": ["Executive Summary"]},
            {"name": "heading_hierarchy"},
            {"name": "no_tracked_changes"},
            {"name": "no_comments"},
            {"name": "max_heading_depth", "max_depth": 3},
            {"name": "metadata_present"},
        ])
        result = engine.evaluate(CLEAN_INSPECT)
        assert result.passed
        assert result.violations == []

    def test_multiple_failures_collected(self) -> None:
        bad_data = {
            **CLEAN_INSPECT,
            "headings": [
                {"index": 0, "level": 1, "text": "DRAFT Intro"},
                {"index": 1, "level": 3, "text": "Skipped"},
            ],
            "tracked_changes": {"count": 1, "insertions": 1, "deletions": 0, "authors": []},
            "comments": {"count": 1, "authors": ["X"], "unresolved": 1},
        }
        engine = PolicyEngine(rules=[
            {"name": "required_sections", "sections": ["Conclusion"]},
            {"name": "heading_hierarchy"},
            {"name": "prohibited_terms", "terms": ["DRAFT"]},
            {"name": "no_tracked_changes"},
            {"name": "no_comments"},
        ])
        result = engine.evaluate(bad_data)
        assert not result.passed
        # At least one violation per failing rule.
        assert len(result.errors) >= 5
