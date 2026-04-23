"""OOXML artifact inspection using zipfile and lxml."""

from __future__ import annotations

import zipfile
from pathlib import Path

from lxml import etree

from xli_companion.models import Finding, Severity

# Common OOXML namespaces
_NS_CONTENT_TYPES = "http://schemas.openxmlformats.org/package/2006/content-types"
_NS_RELS = "http://schemas.openxmlformats.org/package/2006/relationships"
_NS_SST = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"


def check_content_types(path: Path) -> list[Finding]:
    """Open the xlsx as a zip, parse [Content_Types].xml, verify it has workbook and worksheet entries."""
    findings: list[Finding] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            if "[Content_Types].xml" not in zf.namelist():
                findings.append(Finding(
                    code="MISSING_CONTENT_TYPES",
                    severity=Severity.ERROR,
                    message="[Content_Types].xml is missing from the archive.",
                ))
                return findings

            tree = etree.fromstring(zf.read("[Content_Types].xml"))
            overrides = [
                el.get("ContentType", "")
                for el in tree.findall(f"{{{_NS_CONTENT_TYPES}}}Override")
            ]

            has_workbook = any(
                "spreadsheetml.sheet.main" in ct or "spreadsheetml.template.main" in ct
                for ct in overrides
            )
            has_worksheet = any("spreadsheetml.worksheet" in ct for ct in overrides)

            if not has_workbook:
                findings.append(Finding(
                    code="MISSING_WORKBOOK_CONTENT_TYPE",
                    severity=Severity.ERROR,
                    message="No workbook content type found in [Content_Types].xml.",
                ))
            if not has_worksheet:
                findings.append(Finding(
                    code="MISSING_WORKSHEET_CONTENT_TYPE",
                    severity=Severity.ERROR,
                    message="No worksheet content type found in [Content_Types].xml.",
                ))
    except zipfile.BadZipFile:
        findings.append(Finding(
            code="INVALID_ZIP",
            severity=Severity.ERROR,
            message=f"File is not a valid zip archive: {path}",
        ))
    except FileNotFoundError:
        findings.append(Finding(
            code="FILE_NOT_FOUND",
            severity=Severity.ERROR,
            message=f"File not found: {path}",
        ))
    return findings


def check_chart_relationships(path: Path) -> list[Finding]:
    """Parse xl/_rels/workbook.xml.rels looking for chart relationships, flag if chartN.xml referenced but missing."""
    findings: list[Finding] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            rels_path = "xl/_rels/workbook.xml.rels"
            if rels_path not in zf.namelist():
                return findings

            tree = etree.fromstring(zf.read(rels_path))
            namelist = set(zf.namelist())

            for rel in tree.findall(f"{{{_NS_RELS}}}Relationship"):
                target = rel.get("Target", "")
                if "chart" in target.lower():
                    # Resolve relative path against xl/
                    full_target = f"xl/{target}" if not target.startswith("/") else target.lstrip("/")
                    if full_target not in namelist:
                        findings.append(Finding(
                            code="MISSING_CHART_FILE",
                            severity=Severity.ERROR,
                            message=f"Chart relationship references '{target}' but file is missing from archive.",
                        ))
    except zipfile.BadZipFile:
        findings.append(Finding(
            code="INVALID_ZIP",
            severity=Severity.ERROR,
            message=f"File is not a valid zip archive: {path}",
        ))
    except FileNotFoundError:
        findings.append(Finding(
            code="FILE_NOT_FOUND",
            severity=Severity.ERROR,
            message=f"File not found: {path}",
        ))
    return findings


def check_vba_presence(path: Path) -> list[Finding]:
    """Check if xl/vbaProject.bin exists in the zip, return INFO finding if present."""
    findings: list[Finding] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            if "xl/vbaProject.bin" in zf.namelist():
                findings.append(Finding(
                    code="VBA_PROJECT_PRESENT",
                    severity=Severity.INFO,
                    message="Workbook contains a VBA project (xl/vbaProject.bin).",
                ))
    except zipfile.BadZipFile:
        findings.append(Finding(
            code="INVALID_ZIP",
            severity=Severity.ERROR,
            message=f"File is not a valid zip archive: {path}",
        ))
    except FileNotFoundError:
        findings.append(Finding(
            code="FILE_NOT_FOUND",
            severity=Severity.ERROR,
            message=f"File not found: {path}",
        ))
    return findings


def check_external_links(path: Path) -> list[Finding]:
    """Look for externalLink entries in workbook.xml.rels, flag as WARNING."""
    findings: list[Finding] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            rels_path = "xl/_rels/workbook.xml.rels"
            if rels_path not in zf.namelist():
                return findings

            tree = etree.fromstring(zf.read(rels_path))

            for rel in tree.findall(f"{{{_NS_RELS}}}Relationship"):
                rel_type = rel.get("Type", "")
                target = rel.get("Target", "")
                if "externalLink" in rel_type or "externalLink" in target:
                    findings.append(Finding(
                        code="EXTERNAL_LINK",
                        severity=Severity.WARNING,
                        message=f"Workbook contains an external link reference: {target}",
                    ))
    except zipfile.BadZipFile:
        findings.append(Finding(
            code="INVALID_ZIP",
            severity=Severity.ERROR,
            message=f"File is not a valid zip archive: {path}",
        ))
    except FileNotFoundError:
        findings.append(Finding(
            code="FILE_NOT_FOUND",
            severity=Severity.ERROR,
            message=f"File not found: {path}",
        ))
    return findings


def check_shared_strings_integrity(path: Path) -> list[Finding]:
    """Parse xl/sharedStrings.xml, verify count attribute matches actual si element count."""
    findings: list[Finding] = []
    try:
        with zipfile.ZipFile(path, "r") as zf:
            sst_path = "xl/sharedStrings.xml"
            if sst_path not in zf.namelist():
                # Not all xlsx files have sharedStrings.xml; this is fine.
                return findings

            tree = etree.fromstring(zf.read(sst_path))
            unique_count_attr = tree.get("uniqueCount")
            si_elements = tree.findall(f"{{{_NS_SST}}}si")
            actual_count = len(si_elements)

            if unique_count_attr is not None:
                declared_count = int(unique_count_attr)
                if declared_count != actual_count:
                    findings.append(Finding(
                        code="SST_COUNT_MISMATCH",
                        severity=Severity.WARNING,
                        message=(
                            f"sharedStrings.xml declares uniqueCount={declared_count} "
                            f"but contains {actual_count} <si> elements."
                        ),
                        details={"declared": declared_count, "actual": actual_count},
                    ))
    except zipfile.BadZipFile:
        findings.append(Finding(
            code="INVALID_ZIP",
            severity=Severity.ERROR,
            message=f"File is not a valid zip archive: {path}",
        ))
    except FileNotFoundError:
        findings.append(Finding(
            code="FILE_NOT_FOUND",
            severity=Severity.ERROR,
            message=f"File not found: {path}",
        ))
    return findings
