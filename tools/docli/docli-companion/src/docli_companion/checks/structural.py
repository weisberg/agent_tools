"""Structural integrity checks for OOXML (.docx) files.

Validates that a .docx file is a well-formed ZIP archive containing
the required OOXML parts, valid XML, proper content types, and
consistent relationships.
"""

from __future__ import annotations

import zipfile
from io import BytesIO
from pathlib import Path
from typing import Union

from lxml import etree

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Minimum parts every conformant .docx must contain (ECMA-376 Part 2)
_REQUIRED_PARTS: list[str] = [
    "[Content_Types].xml",
    "word/document.xml",
    "_rels/.rels",
]

_CONTENT_TYPES_NS = "http://schemas.openxmlformats.org/package/2006/content-types"
_RELS_NS = "http://schemas.openxmlformats.org/package/2006/relationships"

# Content-type overrides we expect for core parts
_EXPECTED_CONTENT_TYPES: dict[str, str] = {
    "/word/document.xml": "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
}

# Result type alias
CheckResult = dict[str, str]


def _result(
    check_id: str,
    name: str,
    severity: str,
    status: str,
    message: str,
) -> CheckResult:
    """Build a standardised result dict."""
    return {
        "check_id": check_id,
        "name": name,
        "severity": severity,
        "status": status,
        "message": message,
    }


# ---------------------------------------------------------------------------
# Individual checks
# ---------------------------------------------------------------------------


def check_required_parts(source: Union[str, Path, bytes, BytesIO]) -> list[CheckResult]:
    """Verify the archive contains every required OOXML part.

    Returns one result per required part, each with status "pass" or "fail".
    """
    results: list[CheckResult] = []
    try:
        zf = _open_zip(source)
    except (zipfile.BadZipFile, Exception) as exc:
        return [
            _result(
                "STRUCT-REQ-001",
                "required_parts",
                "error",
                "fail",
                f"Cannot open ZIP archive: {exc}",
            )
        ]

    with zf:
        names = set(zf.namelist())
        for idx, part in enumerate(_REQUIRED_PARTS, start=1):
            present = part in names
            results.append(
                _result(
                    f"STRUCT-REQ-{idx:03d}",
                    "required_parts",
                    "error",
                    "pass" if present else "fail",
                    f"Part '{part}' {'found' if present else 'missing'}",
                )
            )
    return results


def check_xml_wellformedness(source: Union[str, Path, bytes, BytesIO]) -> list[CheckResult]:
    """Parse every .xml and .rels file inside the archive and report parse errors."""
    results: list[CheckResult] = []
    try:
        zf = _open_zip(source)
    except (zipfile.BadZipFile, Exception) as exc:
        return [
            _result(
                "STRUCT-XML-001",
                "xml_wellformedness",
                "error",
                "fail",
                f"Cannot open ZIP archive: {exc}",
            )
        ]

    with zf:
        xml_names = [n for n in zf.namelist() if n.endswith(".xml") or n.endswith(".rels")]
        for idx, name in enumerate(sorted(xml_names), start=1):
            try:
                data = zf.read(name)
                etree.fromstring(data)
                results.append(
                    _result(
                        f"STRUCT-XML-{idx:03d}",
                        "xml_wellformedness",
                        "error",
                        "pass",
                        f"'{name}' is well-formed XML",
                    )
                )
            except etree.XMLSyntaxError as exc:
                results.append(
                    _result(
                        f"STRUCT-XML-{idx:03d}",
                        "xml_wellformedness",
                        "error",
                        "fail",
                        f"'{name}' XML parse error: {exc}",
                    )
                )
    return results


def check_content_types(source: Union[str, Path, bytes, BytesIO]) -> list[CheckResult]:
    """Validate [Content_Types].xml declares overrides for essential parts."""
    results: list[CheckResult] = []
    try:
        zf = _open_zip(source)
    except (zipfile.BadZipFile, Exception) as exc:
        return [
            _result(
                "STRUCT-CT-001",
                "content_types",
                "error",
                "fail",
                f"Cannot open ZIP archive: {exc}",
            )
        ]

    with zf:
        if "[Content_Types].xml" not in zf.namelist():
            return [
                _result(
                    "STRUCT-CT-001",
                    "content_types",
                    "error",
                    "fail",
                    "[Content_Types].xml missing from archive",
                )
            ]

        data = zf.read("[Content_Types].xml")
        try:
            root = etree.fromstring(data)
        except etree.XMLSyntaxError as exc:
            return [
                _result(
                    "STRUCT-CT-001",
                    "content_types",
                    "error",
                    "fail",
                    f"[Content_Types].xml parse error: {exc}",
                )
            ]

        # Build map of PartName -> ContentType from Override elements
        overrides: dict[str, str] = {}
        for override in root.findall(f"{{{_CONTENT_TYPES_NS}}}Override"):
            part_name = override.get("PartName", "")
            content_type = override.get("ContentType", "")
            overrides[part_name] = content_type

        idx = 0
        for part_name, expected_ct in _EXPECTED_CONTENT_TYPES.items():
            idx += 1
            actual = overrides.get(part_name)
            if actual is None:
                results.append(
                    _result(
                        f"STRUCT-CT-{idx:03d}",
                        "content_types",
                        "warning",
                        "fail",
                        f"No Override for '{part_name}' in [Content_Types].xml",
                    )
                )
            elif actual != expected_ct:
                results.append(
                    _result(
                        f"STRUCT-CT-{idx:03d}",
                        "content_types",
                        "warning",
                        "fail",
                        f"ContentType mismatch for '{part_name}': expected '{expected_ct}', got '{actual}'",
                    )
                )
            else:
                results.append(
                    _result(
                        f"STRUCT-CT-{idx:03d}",
                        "content_types",
                        "pass",
                        "pass",
                        f"ContentType for '{part_name}' is correct",
                    )
                )
    return results


def check_relationships(source: Union[str, Path, bytes, BytesIO]) -> list[CheckResult]:
    """Validate that relationship targets in _rels/.rels resolve to archive entries."""
    results: list[CheckResult] = []
    try:
        zf = _open_zip(source)
    except (zipfile.BadZipFile, Exception) as exc:
        return [
            _result(
                "STRUCT-REL-001",
                "relationships",
                "error",
                "fail",
                f"Cannot open ZIP archive: {exc}",
            )
        ]

    with zf:
        rels_path = "_rels/.rels"
        if rels_path not in zf.namelist():
            return [
                _result(
                    "STRUCT-REL-001",
                    "relationships",
                    "error",
                    "fail",
                    f"'{rels_path}' missing from archive",
                )
            ]

        data = zf.read(rels_path)
        try:
            root = etree.fromstring(data)
        except etree.XMLSyntaxError as exc:
            return [
                _result(
                    "STRUCT-REL-001",
                    "relationships",
                    "error",
                    "fail",
                    f"'{rels_path}' parse error: {exc}",
                )
            ]

        names = set(zf.namelist())
        idx = 0
        for rel in root.findall(f"{{{_RELS_NS}}}Relationship"):
            target = rel.get("Target", "")
            target_mode = rel.get("TargetMode", "Internal")

            # Skip external targets
            if target_mode == "External" or target.startswith("http://") or target.startswith("https://"):
                continue

            idx += 1
            # Normalise: targets are relative to the package root
            normalised = target.lstrip("/")
            if normalised in names:
                results.append(
                    _result(
                        f"STRUCT-REL-{idx:03d}",
                        "relationships",
                        "warning",
                        "pass",
                        f"Relationship target '{normalised}' found in archive",
                    )
                )
            else:
                results.append(
                    _result(
                        f"STRUCT-REL-{idx:03d}",
                        "relationships",
                        "warning",
                        "fail",
                        f"Relationship target '{normalised}' NOT found in archive",
                    )
                )

    return results


def run_all_structural(source: Union[str, Path, bytes, BytesIO]) -> list[CheckResult]:
    """Run every structural check and return the combined results."""
    results: list[CheckResult] = []
    results.extend(check_required_parts(source))
    results.extend(check_xml_wellformedness(source))
    results.extend(check_content_types(source))
    results.extend(check_relationships(source))
    return results


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _open_zip(source: Union[str, Path, bytes, BytesIO]) -> zipfile.ZipFile:
    """Open a ZipFile from a path, raw bytes, or BytesIO."""
    if isinstance(source, (str, Path)):
        return zipfile.ZipFile(source, "r")
    if isinstance(source, bytes):
        return zipfile.ZipFile(BytesIO(source), "r")
    if isinstance(source, BytesIO):
        return zipfile.ZipFile(source, "r")
    raise TypeError(f"Unsupported source type: {type(source)}")
