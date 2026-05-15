"""Tests for vaultli."""

from __future__ import annotations

import json
import shutil
import subprocess
import textwrap
from pathlib import Path

import pytest

from tools.vaultli import (
    INDEX_FILENAME,
    VAULT_MARKER,
    VaultliError,
    add_file,
    assemble_context,
    build_index,
    cat_record,
    federated_search,
    find_root,
    git_info,
    infer_frontmatter,
    ingest_path,
    init_vault,
    is_sidecar_markdown,
    load_index_records,
    load_vault_defaults,
    make_id,
    parse_markdown_file,
    refresh_metadata,
    resolve_record,
    scaffold_file,
    search_index,
    set_metadata_field,
    show_record,
    unset_metadata_field,
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


def test_ingest_path_dry_run_reports_bulk_scaffold_plan(vault: Path) -> None:
    _write(vault / "docs" / "notes.md", "# Notes\n")
    _write(vault / "queries" / "report.sql", "select 1;\n")
    _write(
        vault / "docs" / "existing.md",
        _md(
            """
            id: docs/existing
            title: Existing
            description: Already scaffolded
            """,
            "Body.\n",
        ),
    )

    result = ingest_path(vault, root=vault, dry_run=True)

    assert result["dry_run"] is True
    assert result["indexed"] is False
    assert {entry["file"] for entry in result["scaffolded"]} == {
        "docs/notes.md",
        "queries/report.sql.md",
    }
    assert result["skipped"][0]["code"] == "FRONTMATTER_EXISTS"
    assert not (vault / "queries" / "report.sql.md").exists()


def test_ingest_path_scaffolds_directory_and_indexes(vault: Path) -> None:
    _write(vault / "docs" / "notes.md", "# Notes\n")
    _write(vault / "queries" / "report.sql", "select 1;\n")

    result = ingest_path(vault, root=vault, index=True)
    records = load_index_records(vault)

    assert result["indexed"] is True
    assert (vault / "docs" / "notes.md").read_text(encoding="utf-8").startswith("---\n")
    assert (vault / "queries" / "report.sql.md").exists()
    assert {record["id"] for record in records} == {"docs/notes", "queries/report"}


def test_ingest_path_respects_include_and_exclude_patterns(vault: Path) -> None:
    _write(vault / "docs" / "notes.md", "# Notes\n")
    _write(vault / "queries" / "report.sql", "select 1;\n")
    _write(vault / "queries" / "skip.sql", "select 2;\n")

    result = ingest_path(
        vault,
        root=vault,
        dry_run=True,
        include=["queries/*.sql"],
        exclude=["queries/skip.sql"],
    )

    assert result["include"] == ["queries/*.sql"]
    assert result["exclude"] == ["queries/skip.sql"]
    assert {entry["file"] for entry in result["scaffolded"]} == {"queries/report.sql.md"}


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


def test_search_supports_field_filters_tags_and_limit(vault: Path) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            category: reference
            status: active
            domain: tooling
            scope: team
            tags: [tooling, onboarding]
            """,
            "Body.\n",
        ),
    )
    _write(
        vault / "docs" / "draft.md",
        _md(
            """
            id: docs/draft
            title: Draft
            description: Draft note
            category: note
            status: draft
            domain: finance
            scope: personal
            tags: [finance]
            """,
            "Body.\n",
        ),
    )
    build_index(vault, full=True)

    results = search_index(
        root=vault,
        category="reference",
        status="active",
        domain="tooling",
        scope="team",
        tags=["tooling", "onboarding"],
        limit=1,
    )

    assert [record["id"] for record in results] == ["docs/guide"]


def test_search_supports_sort_explain_and_semantic(vault: Path) -> None:
    _write(
        vault / "docs" / "b.md",
        _md("id: docs/b\ntitle: Beta\ndescription: alpha alpha\npriority: 3", "Body.\n"),
    )
    _write(
        vault / "docs" / "a.md",
        _md("id: docs/a\ntitle: Gamma\ndescription: alpha\npriority: 1", "Body.\n"),
    )
    build_index(vault, full=True)

    by_priority = search_index(root=vault, sort="priority", order="asc")
    by_score = search_index("alpha", root=vault, explain=True, sort="score", order="desc")
    semantic = search_index("alpha beta", root=vault, semantic=True, explain=True)

    assert [record["id"] for record in by_priority] == ["docs/a", "docs/b"]
    assert by_score[0]["id"] == "docs/b"
    assert by_score[0]["_match"]["score"] > by_score[1]["_match"]["score"]
    assert {record["id"] for record in semantic} == {"docs/a", "docs/b"}
    assert semantic[0]["_match"]["semantic"] is True
    with pytest.raises(VaultliError, match="Unsupported sort"):
        search_index(root=vault, sort="unknown")


def test_resolve_cat_and_context_retrieval_helpers(vault: Path) -> None:
    _write(vault / "queries" / "report.sql", "select 1;\n")
    _write(
        vault / "queries" / "report.sql.md",
        _md(
            """
            id: queries/report
            title: Report Query
            description: SQL report query
            source: ./report.sql
            depends_on:
              - docs/guide
            tokens: 4
            """,
            "Sidecar notes.\n",
        ),
    )
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            tokens: 3
            """,
            "Guide body.\n",
        ),
    )
    build_index(vault, full=True)

    resolved = resolve_record("queries/report", root=vault, include_body=True, include_source=True)
    body = cat_record("queries/report", root=vault)
    source = cat_record("queries/report", root=vault, source=True)
    context = assemble_context(root=vault, ids=["queries/report"], token_budget=10)
    tight_context = assemble_context(root=vault, ids=["queries/report"], token_budget=3)

    assert resolved["file"] == "queries/report.sql.md"
    assert resolved["source_file"] == "queries/report.sql"
    assert resolved["body"] == "Sidecar notes.\n"
    assert resolved["source_content"] == "select 1;\n"
    assert body["content"] == "Sidecar notes.\n"
    assert source["content"] == "select 1;\n"
    assert [entry["id"] for entry in context["records"]] == ["queries/report", "docs/guide"]
    assert tight_context["records"][0]["id"] == "docs/guide"
    with pytest.raises(VaultliError, match="Source target does not exist"):
        (vault / "queries" / "report.sql").unlink()
        cat_record("queries/report", root=vault, source=True)


def test_federated_search_annotates_vault_origin(tmp_path: Path) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    init_vault(first)
    init_vault(second)
    _write(
        first / "docs" / "guide.md",
        _md("id: docs/guide\ntitle: First Guide\ndescription: shared alpha", "First.\n"),
    )
    _write(
        second / "docs" / "guide.md",
        _md("id: docs/guide\ntitle: Second Guide\ndescription: shared alpha", "Second.\n"),
    )
    build_index(first, full=True)
    build_index(second, full=True)

    result = federated_search([first, second], "alpha")

    assert result["total"] == 2
    assert {record["_vault"]["name"] for record in result["results"]} == {"first", "second"}
    assert {record["global_id"] for record in result["results"]} == {
        "first:docs/guide",
        "second:docs/guide",
    }
    with pytest.raises(VaultliError, match="at least one vault"):
        federated_search([])


@pytest.mark.skipif(shutil.which("git") is None, reason="git not installed")
def test_git_info_reports_repo_and_file_state(vault: Path) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md("id: docs/guide\ntitle: Guide\ndescription: Helpful guide", "Guide body.\n"),
    )
    build_index(vault, full=True)
    subprocess.run(["git", "-C", str(vault), "init"], check=True, capture_output=True)
    subprocess.run(["git", "-C", str(vault), "config", "user.email", "agent@example.com"], check=True)
    subprocess.run(["git", "-C", str(vault), "config", "user.name", "Agent"], check=True)
    subprocess.run(["git", "-C", str(vault), "add", "."], check=True)
    subprocess.run(["git", "-C", str(vault), "commit", "-m", "initial vault"], check=True, capture_output=True)

    info = git_info("docs/guide", root=vault)
    (vault / "docs" / "guide.md").write_text(
        _md("id: docs/guide\ntitle: Guide\ndescription: Helpful guide", "Changed.\n"),
        encoding="utf-8",
    )
    dirty = git_info("docs/guide.md", root=vault)

    assert info["available"] is True
    assert info["file"]["tracked"] is True
    assert info["file"]["last_commit"]["author"] == "Agent"
    assert dirty["dirty"] is True
    assert dirty["file"]["status"].startswith(" M")


def test_git_info_is_safe_outside_git_repo(vault: Path) -> None:
    info = git_info(root=vault)

    assert info["available"] is False
    assert info["reason"] == "not a git repository"


def test_metadata_set_unset_refresh_and_defaults(vault: Path) -> None:
    (vault / VAULT_MARKER).write_text(
        "defaults:\n  author: brian\n  scope: team\n  domain: tooling\n",
        encoding="utf-8",
    )
    doc = _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Old
            description: Old description
            tags: [old]
            created: 2026-01-01
            """,
            "Important body.\n",
        ),
    )
    build_index(vault, full=True)

    assert load_vault_defaults(vault)["author"] == "brian"
    assert infer_frontmatter(doc, vault)["scope"] == "team"

    set_result = set_metadata_field("docs/guide", "tags", "alpha,beta", root=vault, index=True)
    assert set_result["value"] == ["alpha", "beta"]
    assert show_record("docs/guide", root=vault)["tags"] == ["alpha", "beta"]

    unset_result = unset_metadata_field("docs/guide", "tags", root=vault, index=True)
    assert unset_result["removed"] == ["alpha", "beta"]
    assert "tags" not in show_record("docs/guide", root=vault)

    refresh_result = refresh_metadata("docs/guide", root=vault, fields=["title", "domain"], index=True)
    refreshed = parse_markdown_file(doc, vault)
    assert refresh_result["fields"]["title"] == "Guide"
    assert refreshed.metadata["title"] == "Guide"
    assert refreshed.metadata["domain"] == "tooling"
    assert refreshed.metadata["created"] == "2026-01-01"
    assert refreshed.body == "Important body.\n"

    with pytest.raises(VaultliError, match="must be an integer"):
        set_metadata_field("docs/guide", "priority", "not-an-int", root=vault)


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


def test_cli_ingest_json_output(vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
    _write(vault / "docs" / "notes.md", "# Notes\n")

    exit_code = main(["--json", "ingest", str(vault), "--root", str(vault), "--dry-run"])
    captured = capsys.readouterr()
    payload = json.loads(captured.out)

    assert exit_code == 0
    assert payload["ok"] is True
    assert payload["result"]["scaffolded"][0]["file"] == "docs/notes.md"


def test_cli_search_filters_json_output(vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            category: reference
            status: active
            tags: [tooling]
            """,
            "Body.\n",
        ),
    )
    build_index(vault, full=True)

    exit_code = main(
        [
            "--json",
            "search",
            "--root",
            str(vault),
            "--category",
            "reference",
            "--status",
            "active",
            "--tag",
            "tooling",
            "--limit",
            "1",
        ]
    )
    captured = capsys.readouterr()
    payload = json.loads(captured.out)

    assert exit_code == 0
    assert payload["result"]["total"] == 1
    assert payload["result"]["results"][0]["id"] == "docs/guide"


def test_cli_resolve_cat_and_context_json_output(vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
    _write(
        vault / "docs" / "guide.md",
        _md(
            """
            id: docs/guide
            title: Guide
            description: Helpful guide
            tokens: 2
            """,
            "Guide body.\n",
        ),
    )
    build_index(vault, full=True)

    assert main(["--json", "resolve", "docs/guide", "--root", str(vault), "--body"]) == 0
    resolved = json.loads(capsys.readouterr().out)
    assert resolved["result"]["body"] == "Guide body.\n"

    assert main(["cat", "docs/guide", "--root", str(vault)]) == 0
    assert capsys.readouterr().out == "Guide body.\n"

    assert main(["--json", "context", "--root", str(vault), "--id", "docs/guide"]) == 0
    context = json.loads(capsys.readouterr().out)
    assert context["result"]["records"][0]["id"] == "docs/guide"


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
