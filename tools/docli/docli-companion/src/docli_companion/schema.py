"""JSON Schema definitions and validators for docli output envelopes.

Provides schemas for:
- The common docli JSON envelope (ok, command, data, warnings, elapsed_ms)
- The ``inspect`` command output
- The ``validate`` command output

Functions:
    validate_envelope: Validate any docli JSON envelope.
    validate_inspect_output: Validate an ``inspect`` command envelope.
    validate_validate_output: Validate a ``validate`` command envelope.
    check_docli_compatibility: Check a payload against a minimum docli version contract.
"""

from __future__ import annotations

from typing import Any

import jsonschema

# ---------------------------------------------------------------------------
# Schema fragments
# ---------------------------------------------------------------------------

_ENVELOPE_SCHEMA: dict[str, Any] = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "docli JSON Envelope",
    "description": "Common envelope returned by every docli command.",
    "type": "object",
    "properties": {
        "ok": {"type": "boolean"},
        "command": {"type": "string", "minLength": 1},
        "data": {"type": "object"},
        "error": {
            "type": "object",
            "properties": {
                "code": {"type": "string", "minLength": 1},
                "message": {"type": "string"},
                "context": {"type": "object"},
            },
            "required": ["code", "message"],
        },
        "warnings": {
            "type": "array",
            "items": {"type": ["string", "object"]},
        },
        "elapsed_ms": {"type": "number", "minimum": 0},
    },
    "required": ["ok", "command"],
    "if": {"properties": {"ok": {"const": True}}},
    "then": {"required": ["ok", "command", "data"]},
    "else": {"required": ["ok", "command", "error"]},
}

_HEADING_ITEM_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "index": {"type": "integer", "minimum": 0},
        "level": {"type": "integer", "minimum": 1, "maximum": 9},
        "text": {"type": "string"},
    },
    "required": ["index", "level", "text"],
}

_TABLE_ITEM_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "index": {"type": "integer", "minimum": 0},
        "location_paragraph": {"type": "integer", "minimum": 0},
        "rows": {"type": "integer", "minimum": 0},
        "cols": {"type": "integer", "minimum": 0},
        "header_row": {"type": "array", "items": {"type": "string"}},
    },
    "required": ["index", "rows", "cols"],
}

_IMAGE_ITEM_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "index": {"type": "integer", "minimum": 0},
        "location_paragraph": {"type": "integer", "minimum": 0},
        "type": {"type": "string"},
        "width_inches": {"type": "number", "minimum": 0},
        "height_inches": {"type": "number", "minimum": 0},
        "alt_text": {"type": "string"},
    },
    "required": ["index"],
}

_INSPECT_DATA_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "file": {"type": "string", "minLength": 1},
        "file_size_bytes": {"type": "integer", "minimum": 0},
        "page_size": {"type": "string"},
        "orientation": {"type": "string", "enum": ["portrait", "landscape"]},
        "sections": {"type": "integer", "minimum": 0},
        "paragraphs": {
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 0},
                "by_style": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "array",
                        "items": {"type": "integer", "minimum": 0},
                    },
                },
            },
            "required": ["count"],
        },
        "headings": {
            "type": "array",
            "items": _HEADING_ITEM_SCHEMA,
        },
        "tables": {
            "type": "array",
            "items": _TABLE_ITEM_SCHEMA,
        },
        "images": {
            "type": "array",
            "items": _IMAGE_ITEM_SCHEMA,
        },
        "comments": {
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 0},
                "authors": {"type": "array", "items": {"type": "string"}},
                "unresolved": {"type": "integer", "minimum": 0},
            },
            "required": ["count"],
        },
        "tracked_changes": {
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 0},
                "insertions": {"type": "integer", "minimum": 0},
                "deletions": {"type": "integer", "minimum": 0},
                "authors": {"type": "array", "items": {"type": "string"}},
            },
            "required": ["count"],
        },
        "styles": {"type": "array", "items": {"type": "string"}},
        "fonts": {"type": "array", "items": {"type": "string"}},
        "word_count": {"type": "integer", "minimum": 0},
    },
    "required": ["file", "paragraphs"],
}

_INSPECT_ENVELOPE_SCHEMA: dict[str, Any] = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "docli inspect Envelope",
    "description": "Envelope returned by ``docli inspect``.",
    "type": "object",
    "allOf": [
        _ENVELOPE_SCHEMA,
        {
            "if": {"properties": {"ok": {"const": True}}},
            "then": {
                "properties": {
                    "command": {"const": "inspect"},
                    "data": _INSPECT_DATA_SCHEMA,
                },
            },
        },
    ],
}

_REPAIR_DETAIL_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "type": {"type": "string", "minLength": 1},
        "file": {"type": "string"},
        "element": {"type": "string"},
        "action": {"type": "string"},
    },
    "required": ["type", "action"],
}

_VALIDATE_WARNING_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "type": {"type": "string", "minLength": 1},
        "message": {"type": "string"},
    },
    "required": ["type", "message"],
}

_SCHEMA_ERROR_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "type": {"type": "string", "minLength": 1},
        "message": {"type": "string"},
        "path": {"type": "string"},
        "severity": {"type": "string", "enum": ["error", "warning"]},
    },
    "required": ["type"],
}

_VALIDATE_DATA_SCHEMA: dict[str, Any] = {
    "type": "object",
    "properties": {
        "valid": {"type": "boolean"},
        "repairs": {"type": "integer", "minimum": 0},
        "repairs_detail": {
            "type": "array",
            "items": _REPAIR_DETAIL_SCHEMA,
        },
        "warnings": {
            "type": "array",
            "items": _VALIDATE_WARNING_SCHEMA,
        },
        "schema_errors": {
            "type": "array",
            "items": _SCHEMA_ERROR_SCHEMA,
        },
    },
    "required": ["valid"],
}

_VALIDATE_ENVELOPE_SCHEMA: dict[str, Any] = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "title": "docli validate Envelope",
    "description": "Envelope returned by ``docli validate``.",
    "type": "object",
    "allOf": [
        _ENVELOPE_SCHEMA,
        {
            "if": {"properties": {"ok": {"const": True}}},
            "then": {
                "properties": {
                    "command": {"const": "validate"},
                    "data": _VALIDATE_DATA_SCHEMA,
                },
            },
        },
    ],
}

# ---------------------------------------------------------------------------
# Public schema accessors
# ---------------------------------------------------------------------------


def get_envelope_schema() -> dict[str, Any]:
    """Return a copy of the base docli envelope JSON Schema."""
    return _ENVELOPE_SCHEMA.copy()


def get_inspect_schema() -> dict[str, Any]:
    """Return a copy of the ``inspect`` envelope JSON Schema."""
    return _INSPECT_ENVELOPE_SCHEMA.copy()


def get_validate_schema() -> dict[str, Any]:
    """Return a copy of the ``validate`` envelope JSON Schema."""
    return _VALIDATE_ENVELOPE_SCHEMA.copy()


# ---------------------------------------------------------------------------
# Validation helpers
# ---------------------------------------------------------------------------


class SchemaValidationError(Exception):
    """Raised when a payload fails JSON Schema validation."""

    def __init__(self, message: str, errors: list[str]) -> None:
        super().__init__(message)
        self.errors = errors


def _run_validation(payload: dict[str, Any], schema: dict[str, Any]) -> list[str]:
    """Validate *payload* against *schema* and return a list of error messages."""
    validator_cls = jsonschema.Draft202012Validator
    validator = validator_cls(schema)
    return sorted(
        e.message for e in validator.iter_errors(payload)
    )


def validate_envelope(payload: dict[str, Any]) -> None:
    """Validate a generic docli JSON envelope.

    Raises:
        SchemaValidationError: If validation fails.
    """
    errors = _run_validation(payload, _ENVELOPE_SCHEMA)
    if errors:
        raise SchemaValidationError(
            f"Envelope validation failed with {len(errors)} error(s)", errors
        )


def validate_inspect_output(payload: dict[str, Any]) -> None:
    """Validate a ``docli inspect`` JSON envelope.

    Raises:
        SchemaValidationError: If validation fails.
    """
    errors = _run_validation(payload, _INSPECT_ENVELOPE_SCHEMA)
    if errors:
        raise SchemaValidationError(
            f"Inspect output validation failed with {len(errors)} error(s)", errors
        )


def validate_validate_output(payload: dict[str, Any]) -> None:
    """Validate a ``docli validate`` JSON envelope.

    Raises:
        SchemaValidationError: If validation fails.
    """
    errors = _run_validation(payload, _VALIDATE_ENVELOPE_SCHEMA)
    if errors:
        raise SchemaValidationError(
            f"Validate output validation failed with {len(errors)} error(s)", errors
        )


def check_docli_compatibility(
    payload: dict[str, Any],
    *,
    required_fields: list[str] | None = None,
) -> list[str]:
    """Check whether a docli payload meets a minimum compatibility contract.

    This is a lightweight check that does not perform full schema validation.
    It verifies:

    1. The payload is a valid envelope (has ``ok`` and ``command``).
    2. If *required_fields* is provided, each dot-separated path exists inside
       ``data`` (only checked when ``ok`` is ``True``).

    Args:
        payload: The docli JSON output to check.
        required_fields: Optional list of dot-separated paths expected inside
            ``data`` (e.g. ``["paragraphs.count", "headings"]``).

    Returns:
        A list of human-readable incompatibility messages.  An empty list
        means the payload is compatible.
    """
    issues: list[str] = []

    if not isinstance(payload, dict):
        return ["Payload is not a JSON object"]

    if "ok" not in payload:
        issues.append("Missing required field: ok")
    if "command" not in payload:
        issues.append("Missing required field: command")

    # If the envelope is an error response, skip data-path checks.
    if not payload.get("ok", False):
        return issues

    data = payload.get("data")
    if data is None:
        issues.append("Missing required field: data (ok=true)")
        return issues

    for field_path in required_fields or []:
        obj: Any = data
        parts = field_path.split(".")
        for part in parts:
            if not isinstance(obj, dict) or part not in obj:
                issues.append(f"Missing expected data field: {field_path}")
                break
            obj = obj[part]

    return issues
