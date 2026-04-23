"""Tests for docli_companion.schema — JSON Schema validation of docli output."""

from __future__ import annotations

import copy

import pytest

from docli_companion.schema import (
    SchemaValidationError,
    check_docli_compatibility,
    get_envelope_schema,
    get_inspect_schema,
    get_validate_schema,
    validate_envelope,
    validate_inspect_output,
    validate_validate_output,
)

# ---------------------------------------------------------------------------
# Fixtures: canonical payloads taken from the docli spec
# ---------------------------------------------------------------------------

VALID_INSPECT_PAYLOAD: dict = {
    "ok": True,
    "command": "inspect",
    "data": {
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
                "ListParagraph": [8, 9, 10, 11],
            },
        },
        "headings": [
            {"index": 0, "level": 1, "text": "Executive Summary"},
            {"index": 1, "level": 2, "text": "Key Findings"},
            {"index": 5, "level": 2, "text": "Methodology"},
            {"index": 12, "level": 1, "text": "Experiment Design"},
            {"index": 13, "level": 2, "text": "CUPED Variance Reduction"},
        ],
        "tables": [
            {
                "index": 0,
                "location_paragraph": 4,
                "rows": 5,
                "cols": 3,
                "header_row": ["Metric", "Control", "Treatment"],
            }
        ],
        "images": [
            {
                "index": 0,
                "location_paragraph": 7,
                "type": "png",
                "width_inches": 6.5,
                "height_inches": 4.0,
                "alt_text": "Lift chart",
            }
        ],
        "comments": {"count": 3, "authors": ["Jane", "Claude"], "unresolved": 2},
        "tracked_changes": {
            "count": 8,
            "insertions": 5,
            "deletions": 3,
            "authors": ["Claude"],
        },
        "styles": ["Heading1", "Heading2", "Normal", "ListParagraph", "Caption"],
        "fonts": ["Arial", "Calibri"],
        "word_count": 2847,
    },
    "warnings": [],
    "elapsed_ms": 142,
}

VALID_VALIDATE_PAYLOAD: dict = {
    "ok": True,
    "command": "validate",
    "data": {
        "valid": True,
        "repairs": 2,
        "repairs_detail": [
            {
                "type": "durable_id_overflow",
                "file": "commentsIds.xml",
                "action": "regenerated",
            },
            {
                "type": "missing_xml_space",
                "file": "document.xml",
                "element": "w:t",
                "action": "added",
            },
        ],
        "warnings": [
            {
                "type": "mixed_fonts",
                "message": "Document uses 3 font families (Arial, Calibri, Times New Roman)",
            }
        ],
        "schema_errors": [],
    },
    "warnings": [],
    "elapsed_ms": 38,
}

ERROR_PAYLOAD: dict = {
    "ok": False,
    "command": "edit",
    "error": {
        "code": "INVALID_TARGET",
        "message": "Paragraph index 47 out of range (document has 32 paragraphs)",
        "context": {"max_index": 31},
    },
    "warnings": [],
    "elapsed_ms": 38,
}


# ===================================================================
# Test: validate_envelope
# ===================================================================


class TestValidateEnvelope:
    """Tests for the generic envelope validator."""

    def test_valid_success_envelope(self) -> None:
        validate_envelope(VALID_INSPECT_PAYLOAD)

    def test_valid_error_envelope(self) -> None:
        validate_envelope(ERROR_PAYLOAD)

    def test_missing_ok_field(self) -> None:
        payload = {"command": "inspect", "data": {}}
        with pytest.raises(SchemaValidationError) as exc_info:
            validate_envelope(payload)
        assert len(exc_info.value.errors) > 0

    def test_missing_command_field(self) -> None:
        payload = {"ok": True, "data": {}}
        with pytest.raises(SchemaValidationError) as exc_info:
            validate_envelope(payload)
        assert len(exc_info.value.errors) > 0

    def test_ok_true_requires_data(self) -> None:
        payload = {"ok": True, "command": "inspect"}
        with pytest.raises(SchemaValidationError) as exc_info:
            validate_envelope(payload)
        assert any("data" in e for e in exc_info.value.errors)

    def test_ok_false_requires_error(self) -> None:
        payload = {"ok": False, "command": "edit"}
        with pytest.raises(SchemaValidationError) as exc_info:
            validate_envelope(payload)
        assert any("error" in e for e in exc_info.value.errors)

    def test_minimal_success_envelope(self) -> None:
        payload = {"ok": True, "command": "doctor", "data": {}}
        validate_envelope(payload)

    def test_minimal_error_envelope(self) -> None:
        payload = {
            "ok": False,
            "command": "edit",
            "error": {"code": "FAIL", "message": "something broke"},
        }
        validate_envelope(payload)


# ===================================================================
# Test: validate_inspect_output
# ===================================================================


class TestValidateInspectOutput:
    """Tests for the inspect-specific validator."""

    def test_valid_inspect_output(self) -> None:
        validate_inspect_output(VALID_INSPECT_PAYLOAD)

    def test_inspect_missing_file_in_data(self) -> None:
        payload = copy.deepcopy(VALID_INSPECT_PAYLOAD)
        del payload["data"]["file"]
        with pytest.raises(SchemaValidationError):
            validate_inspect_output(payload)

    def test_inspect_missing_paragraphs(self) -> None:
        payload = copy.deepcopy(VALID_INSPECT_PAYLOAD)
        del payload["data"]["paragraphs"]
        with pytest.raises(SchemaValidationError):
            validate_inspect_output(payload)

    def test_inspect_invalid_heading_level(self) -> None:
        payload = copy.deepcopy(VALID_INSPECT_PAYLOAD)
        payload["data"]["headings"][0]["level"] = 0  # must be >= 1
        with pytest.raises(SchemaValidationError):
            validate_inspect_output(payload)

    def test_inspect_invalid_orientation(self) -> None:
        payload = copy.deepcopy(VALID_INSPECT_PAYLOAD)
        payload["data"]["orientation"] = "sideways"
        with pytest.raises(SchemaValidationError):
            validate_inspect_output(payload)

    def test_inspect_minimal_data(self) -> None:
        """Only the required fields in data — should still pass."""
        payload = {
            "ok": True,
            "command": "inspect",
            "data": {
                "file": "test.docx",
                "paragraphs": {"count": 0},
            },
        }
        validate_inspect_output(payload)


# ===================================================================
# Test: validate_validate_output
# ===================================================================


class TestValidateValidateOutput:
    """Tests for the validate-specific validator."""

    def test_valid_validate_output(self) -> None:
        validate_validate_output(VALID_VALIDATE_PAYLOAD)

    def test_validate_missing_valid_field(self) -> None:
        payload = copy.deepcopy(VALID_VALIDATE_PAYLOAD)
        del payload["data"]["valid"]
        with pytest.raises(SchemaValidationError):
            validate_validate_output(payload)

    def test_validate_repair_detail_missing_type(self) -> None:
        payload = copy.deepcopy(VALID_VALIDATE_PAYLOAD)
        payload["data"]["repairs_detail"] = [{"action": "regenerated"}]
        with pytest.raises(SchemaValidationError):
            validate_validate_output(payload)

    def test_validate_minimal_data(self) -> None:
        payload = {
            "ok": True,
            "command": "validate",
            "data": {"valid": False},
        }
        validate_validate_output(payload)


# ===================================================================
# Test: check_docli_compatibility
# ===================================================================


class TestCheckDocliCompatibility:
    """Tests for the lightweight compatibility checker."""

    def test_compatible_payload(self) -> None:
        issues = check_docli_compatibility(
            VALID_INSPECT_PAYLOAD,
            required_fields=["paragraphs.count", "headings", "file"],
        )
        assert issues == []

    def test_missing_ok_field(self) -> None:
        issues = check_docli_compatibility({"command": "inspect"})
        assert any("ok" in i for i in issues)

    def test_missing_command_field(self) -> None:
        issues = check_docli_compatibility({"ok": True, "data": {}})
        assert any("command" in i for i in issues)

    def test_missing_data_on_success(self) -> None:
        issues = check_docli_compatibility({"ok": True, "command": "inspect"})
        assert any("data" in i for i in issues)

    def test_missing_nested_field(self) -> None:
        issues = check_docli_compatibility(
            VALID_INSPECT_PAYLOAD,
            required_fields=["paragraphs.count", "nonexistent_field"],
        )
        assert len(issues) == 1
        assert "nonexistent_field" in issues[0]

    def test_error_payload_skips_data_checks(self) -> None:
        issues = check_docli_compatibility(
            ERROR_PAYLOAD,
            required_fields=["paragraphs.count"],
        )
        assert issues == []

    def test_non_dict_payload(self) -> None:
        issues = check_docli_compatibility("not a dict")  # type: ignore[arg-type]
        assert any("not a JSON object" in i for i in issues)

    def test_no_required_fields(self) -> None:
        issues = check_docli_compatibility(VALID_INSPECT_PAYLOAD)
        assert issues == []


# ===================================================================
# Test: schema accessor functions
# ===================================================================


class TestSchemaAccessors:
    """Verify the public schema getters return valid dicts."""

    def test_get_envelope_schema(self) -> None:
        schema = get_envelope_schema()
        assert isinstance(schema, dict)
        assert schema.get("type") == "object"

    def test_get_inspect_schema(self) -> None:
        schema = get_inspect_schema()
        assert isinstance(schema, dict)
        assert "inspect" in schema.get("title", "").lower()

    def test_get_validate_schema(self) -> None:
        schema = get_validate_schema()
        assert isinstance(schema, dict)
        assert "validate" in schema.get("title", "").lower()
