"""Tests for docli_companion.batch — batch audit module."""

from __future__ import annotations

import json
import zipfile
from pathlib import Path

import pytest

from docli_companion.batch import (
    BatchConfig,
    BatchResult,
    audit_batch,
    audit_single,
    batch_summary_markdown,
    batch_to_csv,
    batch_to_json,
    discover_documents,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_docx(path: Path, parts: dict[str, str] | None = None) -> Path:
    """Create a minimal .docx (ZIP) at *path* with the given internal parts."""
    if parts is None:
        parts = {
            "[Content_Types].xml": "<Types/>",
            "word/document.xml": "<document/>",
        }
    with zipfile.ZipFile(path, "w") as zf:
        for name, content in parts.items():
            zf.writestr(name, content)
    return path


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestDiscoverDocuments:
    def test_finds_docx_files(self, tmp_path: Path) -> None:
        _make_docx(tmp_path / "a.docx")
        (tmp_path / "sub").mkdir(exist_ok=True)
        _make_docx(tmp_path / "sub" / "b.docx")

        cfg = BatchConfig(root_dir=tmp_path)
        found = discover_documents(cfg)

        assert len(found) == 2
        names = {p.name for p in found}
        assert names == {"a.docx", "b.docx"}

    def test_raises_on_missing_root(self, tmp_path: Path) -> None:
        cfg = BatchConfig(root_dir=tmp_path / "nonexistent")
        with pytest.raises(FileNotFoundError):
            discover_documents(cfg)


class TestAuditSingle:
    def test_valid_docx(self, tmp_path: Path) -> None:
        docx = _make_docx(tmp_path / "good.docx")
        result = audit_single(docx, ("[Content_Types].xml", "word/document.xml"))
        assert result.valid is True
        assert result.is_zip is True
        assert result.missing_parts == []

    def test_missing_parts(self, tmp_path: Path) -> None:
        docx = _make_docx(
            tmp_path / "bad.docx",
            parts={"[Content_Types].xml": "<Types/>"},
        )
        result = audit_single(docx, ("[Content_Types].xml", "word/document.xml"))
        assert result.valid is False
        assert "word/document.xml" in result.missing_parts

    def test_not_a_zip(self, tmp_path: Path) -> None:
        bad = tmp_path / "notzip.docx"
        bad.write_text("this is not a zip file")
        result = audit_single(bad, ("[Content_Types].xml",))
        assert result.valid is False
        assert result.is_zip is False
        assert result.error == "Not a valid ZIP archive"

    def test_nonexistent_file(self, tmp_path: Path) -> None:
        result = audit_single(tmp_path / "nope.docx", ())
        assert result.valid is False
        assert result.error == "File does not exist"


class TestAuditBatch:
    def test_audits_multiple_files(self, tmp_path: Path) -> None:
        _make_docx(tmp_path / "a.docx")
        _make_docx(
            tmp_path / "b.docx",
            parts={"[Content_Types].xml": "<Types/>"},
        )
        cfg = BatchConfig(root_dir=tmp_path, max_workers=2)
        results = audit_batch(cfg)

        assert len(results) == 2
        by_name = {Path(r.path).name: r for r in results}
        assert by_name["a.docx"].valid is True
        assert by_name["b.docx"].valid is False

    def test_empty_directory(self, tmp_path: Path) -> None:
        cfg = BatchConfig(root_dir=tmp_path)
        assert audit_batch(cfg) == []


class TestReporting:
    @pytest.fixture()
    def sample_results(self) -> list[BatchResult]:
        return [
            BatchResult(path="/docs/good.docx", valid=True),
            BatchResult(
                path="/docs/bad.docx",
                valid=False,
                missing_parts=["word/document.xml"],
            ),
        ]

    def test_batch_to_csv(self, sample_results: list[BatchResult]) -> None:
        text = batch_to_csv(sample_results)
        assert "path,valid,is_zip,missing_parts,error" in text
        assert "/docs/good.docx" in text
        assert "word/document.xml" in text

    def test_batch_to_json(self, sample_results: list[BatchResult]) -> None:
        text = batch_to_json(sample_results)
        data = json.loads(text)
        assert len(data) == 2
        assert data[0]["valid"] is True
        assert data[1]["missing_parts"] == ["word/document.xml"]

    def test_batch_summary_markdown(self, sample_results: list[BatchResult]) -> None:
        md = batch_summary_markdown(sample_results)
        assert "# Batch Audit Summary" in md
        assert "| Total  | 2 |" in md
        assert "| Passed | 1 |" in md
        assert "| Failed | 1 |" in md
        assert "## Failed Documents" in md
        assert "`/docs/bad.docx`" in md

    def test_summary_no_failures(self) -> None:
        results = [BatchResult(path="ok.docx", valid=True)]
        md = batch_summary_markdown(results)
        assert "Failed | 0" in md
        assert "## Failed Documents" not in md
