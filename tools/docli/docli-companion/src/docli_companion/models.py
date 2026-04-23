"""Pydantic v2 data models for docli-companion.

Pure data definitions — no business logic, no imports from other companion modules.
"""

from __future__ import annotations

from enum import Enum
from typing import Literal

from pydantic import BaseModel


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------

class ReleaseState(str, Enum):
    PASS = "pass"
    PASS_WITH_WARNINGS = "pass-with-warnings"
    MANUAL_REVIEW = "manual-review"
    FAIL = "fail"


class Severity(str, Enum):
    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


class EvidenceClass(str, Enum):
    STRUCTURAL = "structural"
    POLICY = "policy"
    EXTRACTION_DISAGREEMENT = "extraction-disagreement"
    RENDERING = "rendering"
    OCR = "ocr"
    ADVISORY = "advisory"


# ---------------------------------------------------------------------------
# Core models
# ---------------------------------------------------------------------------

class Evidence(BaseModel):
    evidence_class: EvidenceClass
    source: str
    detail: str
    location: str | None = None


class CheckResult(BaseModel):
    check_id: str
    name: str
    severity: Severity
    status: Literal["passed", "failed", "errored", "skipped"]
    message: str
    evidence: list[Evidence] = []
    elapsed_seconds: float = 0.0


class DocumentIdentity(BaseModel):
    path: str
    sha256: str
    size_bytes: int
    part_count: int


class ToolVersions(BaseModel):
    docli: str | None = None
    companion: str
    python: str
    lxml: str | None = None


class ReportSummary(BaseModel):
    total_checks: int
    passed: int
    failed: int
    errored: int
    skipped: int
    warnings: int


class CompanionReport(BaseModel):
    version: str = "1"
    timestamp: str
    document: DocumentIdentity
    tool_versions: ToolVersions
    release_state: ReleaseState
    checks: list[CheckResult] = []
    summary: ReportSummary
    recommended_action: str | None = None
    elapsed_seconds: float = 0.0


class ArtifactBundle(BaseModel):
    report_json: str
    report_md: str | None = None
    inspect_json: str | None = None
    validate_json: str | None = None
    semantic_html: str | None = None
    render_pdf: str | None = None
    page_images: list[str] = []
