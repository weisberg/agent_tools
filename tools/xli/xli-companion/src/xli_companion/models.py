"""Pydantic models for XLI companion input/output contracts."""

from __future__ import annotations

import platform
import sys
from enum import Enum
from typing import Any

from pydantic import BaseModel, Field


class Severity(str, Enum):
    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


class Finding(BaseModel):
    """A single validation finding."""
    code: str
    severity: Severity
    sheet: str | None = None
    cell: str | None = None
    message: str
    details: dict[str, Any] | None = None


class FixOp(BaseModel):
    """A deterministic fix expressible as an xli batch operation."""
    kind: str = "xli-batch-op"
    op: str
    address: str | None = None
    value: Any | None = None
    formula: str | None = None


class PlatformInfo(BaseModel):
    python: str = Field(default_factory=lambda: f"{sys.version_info.major}.{sys.version_info.minor}")
    os: str = Field(default_factory=lambda: platform.system())


class Summary(BaseModel):
    checks_run: int = 0
    errors: int = 0
    warnings: int = 0
    info: int = 0


class CompanionResult(BaseModel):
    """Top-level output envelope for the Python companion."""
    status: str = "ok"
    workbook: str
    validated_fingerprint: str | None = None
    platform: PlatformInfo = Field(default_factory=PlatformInfo)
    summary: Summary = Field(default_factory=Summary)
    findings: list[Finding] = Field(default_factory=list)
    fix_plan: list[FixOp] = Field(default_factory=list)
    artifacts: dict[str, str] = Field(default_factory=dict)
