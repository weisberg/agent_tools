"""Tests for docli_companion.checks.structural."""

from __future__ import annotations

import zipfile
from io import BytesIO

import pytest

from docli_companion.checks.structural import (
    check_content_types,
    check_relationships,
    check_required_parts,
    check_xml_wellformedness,
    run_all_structural,
)

# ---------------------------------------------------------------------------
# Helpers — programmatically build minimal .docx archives
# ---------------------------------------------------------------------------

_CONTENT_TYPES_XML = b"""\
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
            ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>
"""

_RELS_XML = b"""\
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>
"""

_DOCUMENT_XML = b"""\
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
  </w:body>
</w:document>
"""


def _build_docx(
    *,
    include_content_types: bool = True,
    include_rels: bool = True,
    include_document: bool = True,
    content_types_data: bytes | None = None,
    rels_data: bytes | None = None,
    document_data: bytes | None = None,
    extra_files: dict[str, bytes] | None = None,
) -> bytes:
    """Build a minimal .docx (ZIP) in memory and return its bytes."""
    buf = BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        if include_content_types:
            zf.writestr("[Content_Types].xml", content_types_data or _CONTENT_TYPES_XML)
        if include_rels:
            zf.writestr("_rels/.rels", rels_data or _RELS_XML)
        if include_document:
            zf.writestr("word/document.xml", document_data or _DOCUMENT_XML)
        if extra_files:
            for name, data in extra_files.items():
                zf.writestr(name, data)
    return buf.getvalue()


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestCheckRequiredParts:
    """Tests for check_required_parts."""

    def test_valid_docx_all_parts_present(self) -> None:
        data = _build_docx()
        results = check_required_parts(data)
        assert len(results) >= 3
        assert all(r["status"] == "pass" for r in results)
        # Verify result dict shape
        for r in results:
            assert set(r.keys()) == {"check_id", "name", "severity", "status", "message"}

    def test_missing_document_xml(self) -> None:
        data = _build_docx(include_document=False)
        results = check_required_parts(data)
        statuses = {r["message"]: r["status"] for r in results}
        assert statuses["Part 'word/document.xml' missing"] == "fail"

    def test_not_a_zip(self) -> None:
        results = check_required_parts(b"this is not a zip")
        assert len(results) == 1
        assert results[0]["status"] == "fail"
        assert "Cannot open ZIP archive" in results[0]["message"]


class TestCheckXmlWellformedness:
    """Tests for check_xml_wellformedness."""

    def test_all_xml_wellformed(self) -> None:
        data = _build_docx()
        results = check_xml_wellformedness(data)
        assert all(r["status"] == "pass" for r in results)
        assert len(results) >= 3  # [Content_Types].xml, _rels/.rels, word/document.xml

    def test_malformed_document_xml(self) -> None:
        bad_xml = b"<w:document><not closed"
        data = _build_docx(document_data=bad_xml)
        results = check_xml_wellformedness(data)
        failed = [r for r in results if r["status"] == "fail"]
        assert len(failed) >= 1
        assert any("word/document.xml" in r["message"] for r in failed)


class TestCheckContentTypes:
    """Tests for check_content_types."""

    def test_correct_content_type(self) -> None:
        data = _build_docx()
        results = check_content_types(data)
        assert len(results) >= 1
        assert all(r["status"] == "pass" for r in results)

    def test_missing_override(self) -> None:
        ct_no_override = b"""\
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
</Types>
"""
        data = _build_docx(content_types_data=ct_no_override)
        results = check_content_types(data)
        failed = [r for r in results if r["status"] == "fail"]
        assert len(failed) >= 1
        assert any("No Override" in r["message"] for r in failed)


class TestCheckRelationships:
    """Tests for check_relationships."""

    def test_valid_relationships(self) -> None:
        data = _build_docx()
        results = check_relationships(data)
        assert len(results) >= 1
        assert all(r["status"] == "pass" for r in results)

    def test_broken_relationship_target(self) -> None:
        bad_rels = b"""\
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/nonexistent.xml"/>
</Relationships>
"""
        data = _build_docx(rels_data=bad_rels)
        results = check_relationships(data)
        failed = [r for r in results if r["status"] == "fail"]
        assert len(failed) >= 1
        assert any("nonexistent.xml" in r["message"] for r in failed)


class TestRunAllStructural:
    """Tests for the aggregate runner."""

    def test_valid_docx_all_pass(self) -> None:
        data = _build_docx()
        results = run_all_structural(data)
        assert len(results) >= 6
        assert all(r["status"] == "pass" for r in results)
        # Verify every result has the required keys
        for r in results:
            assert set(r.keys()) == {"check_id", "name", "severity", "status", "message"}

    def test_multiple_failures_reported(self) -> None:
        """An archive missing key parts should surface failures from several checks."""
        data = _build_docx(include_document=False, include_content_types=False)
        results = run_all_structural(data)
        failed = [r for r in results if r["status"] == "fail"]
        assert len(failed) >= 2
