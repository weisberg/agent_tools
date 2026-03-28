"""Tests for vaultli."""

from __future__ import annotations

import json
import shutil
import textwrap
from pathlib import Path

import pytest

from tools.vaultli import (
    INDEX_FILENAME,
    VAULT_MARKER,
    VaultliError,
    add_file,
    build_index,
    find_root,
    infer_frontmatter,
    init_vault,
    is_sidecar_markdown,
    load_index_records,
    make_id,
    parse_markdown_file,
    scaffold_file,
    search_index,
    show_record,
    validate_vault,
)
from tools.vaultli.cli import main


def _write(path: Path, content: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return path


def _md(frontmatter: str, body: str = "") -> str:
    return f"---\n{textwrap.dedent(frontmatter).strip()}\n---\n{body}"


@pytest.fixture
def vault(tmp_path: Path) -> Path:
    root = tmp_path / "vault"
    init_vault(root)
    return root


def test_find_root_walks_upward(tmp_path: Path) -> None:
    root = tmp_path / "vault"
    nested = root / "docs" / "notes"
    nested.mkdir(parents=True)
    (root / VAULT_MARKER).write_text("", encoding="utf-8")

    assert find_root(nested) == root


def test_find_root_raises_when_marker_missing(tmp_path: Path) -> None:
    with pytest.raises(VaultliError, match=r"No \.kbroot found"):
        find_root(tmp_path)


def test_is_sidecar_markdown_distinguishes_sidecars() -> None:
    assert is_sidecar_markdown("queries/report.sql.md") is True
    assert is_sidecar_markdown("docs/report.md") is False


def test_make_id_for_native_markdown(tmp_path: Path) -> None:
    root = tmp_path / "vault"
    doc = root / "docs" / "experimentation_guide.md"
    doc.parent.mkdir(parents=True)
    doc.write_text("# Guide\n", encoding="utf-8")

    assert make_id(doc, root) == "docs/experimentation-guide"


def test_make_id_for_sidecar_markdown(tmp_path: Path) -> None:
    root = tmp_path / "vault"
    doc = root / "queries" / "retention_holdout.sql.md"
    doc.parent.mkdir(parents=True)
    doc.write_text("---\n---\n", encoding="utf-8")

    assert make_id(doc, root) == "queries/retention-holdout"


def test_init_vault_creates_marker_and_index(tmp_path: Path) -> None:
    target = tmp_path / "new-vault"
    result = init_vault(target)

    assert result["root"] == str(target.resolve())
    assert (target / VAULT_MARKER).exists()
    assert (target / INDEX_FILENAME).read_text(encoding="utf-8") == ""


def test_init_vault_rejects_nested_vaults(tmp_path: Path) -> None:
    root = tmp_path / "vault"
    root.mkdir()
    (root / VAULT_MARKER).write_text("", encoding="utf-8")

    with pytest.raises(VaultliError, match="Vault root already exists"):
        init_vault(root / "nested")


def test_parse_markdown_file_reads_frontmatter(vault: Path) -> None:
    path = _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            tags:
              - docs
            """,
            "# Heading\n",
        ),
    )

    parsed = parse_markdown_file(path, vault)

    assert parsed.doc_id == "docs/guide"
    assert parsed.metadata["title"] == "Guide"
    assert parsed.body.strip() == "# Heading"


def test_build_index_full_indexes_native_and_sidecar(vault: Path) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            category: reference
            """,
            "Guide body.\n",
        ),
    )
    _write(vault / "queries" / "report.sql", "select 1;\n")
    _write(
        vault / "queries" / "report.sql.md",
        _md(
            """
            id: queries/report
            title: Report Query
            description: SQL report query
            category: query
            source: ./report.sql
            """,
            "Used for reporting.\n",
        ),
    )

    result = build_index(vault, full=True)
    records = load_index_records(vault)

    assert result.indexed == 2
    assert result.updated == 0
    assert result.pruned == 0
    assert result.skipped == 0
    assert result.warnings == []
    assert {record["id"] for record in records} == {"docs/guide", "queries/report"}
    assert all(len(record["hash"]) == 12 for record in records)


def test_incremental_index_updates_and_prunes(vault: Path) -> None:
    doc = _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            """,
            "Old body.\n",
        ),
    )
    removable = _write(
        vault / "docs" / "remove.md",
        _md(
            """
            id: docs/remove
            title: Remove
            description: To be deleted
            """,
            "Remove me.\n",
        ),
    )
    build_index(vault, full=True)

    doc.write_text(
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            """,
            "New body.\n",
        ),
        encoding="utf-8",
    )
    removable.unlink()
    _write(
        vault / "docs" / "new.md",
        _md(
            """
            id: docs/new
            title: New
            description: Newly added
            """,
            "Fresh body.\n",
        ),
    )

    result = build_index(vault, full=False)

    assert result.indexed == 1
    assert result.updated == 1
    assert result.pruned == 1
    assert result.skipped == 0


def test_sidecar_hash_uses_source_content_not_sidecar_body(vault: Path) -> None:
    _write(vault / "queries" / "report.sql", "select 1;\n")
    sidecar = _write(
        vault / "queries" / "report.sql.md",
        _md(
            """
            id: queries/report
            title: Report Query
            description: SQL report query
            source: ./report.sql
            """,
            "Original prose.\n",
        ),
    )
    first = build_index(vault, full=True)
    assert first.indexed == 1
    original_record = show_record("queries/report", root=vault)

    sidecar.write_text(
        _md(
            """
            id: queries/report
            title: Report Query
            description: SQL report query
            source: ./report.sql
            """,
            "Edited prose only.\n",
        ),
        encoding="utf-8",
    )
    second = build_index(vault, full=False)
    updated_record = show_record("queries/report", root=vault)

    assert second.skipped == 1
    assert second.updated == 0
    assert updated_record["hash"] == original_record["hash"]


def test_scaffold_non_markdown_creates_sidecar(vault: Path) -> None:
    source = _write(vault / "queries" / "campaign_metrics.sql", "select * from metrics;\n")

    result = scaffold_file(source, root=vault)
    sidecar = vault / result["file"]
    text = sidecar.read_text(encoding="utf-8")

    assert result["mode"] == "sidecar"
    assert sidecar.name == "campaign_metrics.sql.md"
    assert "source: ./campaign_metrics.sql" in text
    assert "category: query" in text


def test_add_markdown_injects_frontmatter_and_indexes(vault: Path) -> None:
    doc = _write(vault / "docs" / "notes.md", "# Notes\n")

    result = add_file(doc, root=vault)
    text = doc.read_text(encoding="utf-8")
    record = show_record("docs/notes", root=vault)

    assert result["mode"] == "frontmatter"
    assert text.startswith("---\n")
    assert record["id"] == "docs/notes"
    assert record["title"] == "Notes"


def test_search_and_show_read_index(vault: Path) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: CUPED Guide
            description: Variance reduction methodology for experiments
            tags:
              - experimentation
            """,
            "Guide body.\n",
        ),
    )
    build_index(vault, full=True)

    results = search_index("variance", root=vault)
    shown = show_record("docs/guide", root=vault)

    assert len(results) == 1
    assert results[0]["id"] == "docs/guide"
    assert shown["title"] == "CUPED Guide"


def test_validate_reports_broken_sources_and_dangling_refs(vault: Path) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            depends_on:
              - docs/missing
            related:
              - docs/also-missing
            """,
            "Guide body.\n",
        ),
    )
    _write(
        vault / "queries" / "broken.sql.md",
        _md(
            """
            id: queries/broken
            title: Broken Query
            description: Broken source
            source: ./broken.sql
            """,
            "Broken.\n",
        ),
    )
    build_index(vault, full=True)

    result = validate_vault(vault)
    codes = {issue["code"] for issue in result["issues"]}

    assert result["valid"] is False
    assert "BROKEN_SOURCE" in codes
    assert "ORPHANED_SIDECAR" in codes
    assert "DANGLING_DEPENDENCY" in codes
    assert "DANGLING_RELATED" in codes


def test_validate_reports_duplicate_ids_and_stale_index(vault: Path) -> None:
    first = _write(
        vault / "docs" / "one.md",
        _md(
            """
            id: docs/dup
            title: One
            description: First doc
            """,
            "Original body.\n",
        ),
    )
    _write(
        vault / "docs" / "two.md",
        _md(
            """
            id: docs/dup
            title: Two
            description: Second doc
            """,
            "Another body.\n",
        ),
    )
    build_index(vault, full=True)
    first.write_text(
        _md(
            """
            id: docs/dup
            title: One
            description: First doc
            """,
            "Changed body.\n",
        ),
        encoding="utf-8",
    )

    result = validate_vault(vault)
    codes = [issue["code"] for issue in result["issues"]]

    assert "DUPLICATE_ID" in codes
    assert "STALE_INDEX" in codes


def test_infer_frontmatter_uses_spec_defaults(vault: Path) -> None:
    source = _write(vault / "templates" / "campaign_report.j2", "Hello {{ name }}\n")

    metadata = infer_frontmatter(source, vault)

    assert metadata["id"] == "templates/campaign-report"
    assert metadata["category"] == "template"
    assert metadata["source"] == "./campaign_report.j2"
    assert metadata["title"] == "Campaign Report"


def test_cli_make_id_json_output(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    root = tmp_path / "vault"
    doc = root / "templates" / "campaign_report.j2.md"
    doc.parent.mkdir(parents=True)
    doc.write_text("---\n---\n", encoding="utf-8")

    exit_code = main(["--json", "make-id", str(doc), "--root", str(root)])

    captured = capsys.readouterr()
    assert exit_code == 0
    assert '"id": "templates/campaign-report"' in captured.out


def test_cli_index_and_show_smoke(vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            """,
            "Body.\n",
        ),
    )

    exit_code = main(["index", "--root", str(vault)])
    assert exit_code == 0
    shown_code = main(["show", "docs/guide", "--root", str(vault)])
    captured = capsys.readouterr()

    assert shown_code == 0
    assert "id: docs/guide" in captured.out


def test_cli_validate_returns_nonzero_when_invalid(vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
    _write(vault / "queries" / "broken.sql.md", _md("id: queries/broken\ntitle: Broken\ndescription: Broken\nsource: ./broken.sql"))
    build_index(vault, full=True)

    exit_code = main(["validate", "--root", str(vault)])
    captured = capsys.readouterr()

    assert exit_code == 1
    assert "Validation failed" in captured.out


@pytest.mark.skipif(shutil.which("jq") is None, reason="jq not installed")
def test_search_supports_jq_filter(vault: Path) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            category: reference
            """,
            "Body.\n",
        ),
    )
    build_index(vault, full=True)

    results = search_index(root=vault, jq_filter='select(.category=="reference")')

    assert len(results) == 1
    assert results[0]["id"] == "docs/guide"


def test_dump_index_json_envelope(vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            """,
            "Body.\n",
        ),
    )
    build_index(vault, full=True)

    exit_code = main(["dump-index", "--root", str(vault), "--json"])
    captured = capsys.readouterr()
    payload = json.loads(captured.out)

    assert exit_code == 0
    assert payload["ok"] is True
    assert payload["result"]["records"][0]["id"] == "docs/guide"
