"""Tests for vaultli git integration, context assembly, and multi-vault federation."""

from __future__ import annotations

import json
import subprocess
import textwrap
from pathlib import Path

import pytest

from tools.vaultli import (
    INDEX_FILENAME,
    VAULT_MARKER,
    VaultliError,
    assemble_context,
    build_index,
    federated_load,
    federated_search,
    git_meta_for_file,
    git_meta_for_vault,
    apply_git_meta,
    GitMeta,
    init_vault,
    load_index_records,
    resolve_federated_id,
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


@pytest.fixture
def git_vault(tmp_path: Path) -> Path:
    """Create a vault that is also a git repo with committed files."""
    root = tmp_path / "git_vault"
    init_vault(root)

    subprocess.run(["git", "init"], cwd=str(root), capture_output=True, check=True)
    subprocess.run(
        ["git", "config", "user.email", "test@example.com"],
        cwd=str(root), capture_output=True, check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Test Author"],
        cwd=str(root), capture_output=True, check=True,
    )
    subprocess.run(
        ["git", "config", "commit.gpgsign", "false"],
        cwd=str(root), capture_output=True, check=True,
    )

    _write(
        root / "docs" / "guide.md",
        _md("id: docs/guide\ntitle: Guide\ndescription: Helpful guide", "Guide body.\n"),
    )
    subprocess.run(["git", "add", "."], cwd=str(root), capture_output=True, check=True)
    subprocess.run(
        ["git", "commit", "-m", "Initial commit"],
        cwd=str(root), capture_output=True, check=True,
    )

    build_index(root, full=True)
    return root


# ─── Git integration tests ───────────────────────────────────────────


class TestGitIntegration:
    def test_git_meta_for_file_returns_dates_and_author(self, git_vault: Path) -> None:
        doc = git_vault / "docs" / "guide.md"
        meta = git_meta_for_file(doc, git_vault)

        assert meta.created is not None
        assert meta.updated is not None
        assert meta.author == "Test Author"

    def test_git_meta_for_vault_returns_all_tracked_files(self, git_vault: Path) -> None:
        result = git_meta_for_vault(root=git_vault)

        assert "docs/guide.md" in result
        entry = result["docs/guide.md"]
        assert "created" in entry
        assert "updated" in entry
        assert entry["author"] == "Test Author"

    def test_git_meta_raises_for_non_git_vault(self, vault: Path) -> None:
        _write(
            vault / "docs" / "guide.md",
            _md("id: docs/guide\ntitle: Guide\ndescription: Helpful guide", "Body.\n"),
        )
        with pytest.raises(VaultliError, match="not inside a git repository"):
            git_meta_for_vault(root=vault)

    def test_apply_git_meta_fills_missing_fields(self, git_vault: Path) -> None:
        result = apply_git_meta(root=git_vault)

        assert result["updated"] >= 1
        text = (git_vault / "docs" / "guide.md").read_text(encoding="utf-8")
        assert "author: Test Author" in text

    def test_apply_git_meta_dry_run_does_not_write(self, git_vault: Path) -> None:
        original = (git_vault / "docs" / "guide.md").read_text(encoding="utf-8")
        result = apply_git_meta(root=git_vault, dry_run=True)

        assert result["dry_run"] is True
        assert result["updated"] >= 1
        after = (git_vault / "docs" / "guide.md").read_text(encoding="utf-8")
        assert after == original

    def test_apply_git_meta_skips_existing_fields(self, git_vault: Path) -> None:
        doc = git_vault / "docs" / "guide.md"
        doc.write_text(
            _md(
                "id: docs/guide\ntitle: Guide\ndescription: Helpful guide\nauthor: Brian",
                "Guide body.\n",
            ),
            encoding="utf-8",
        )
        subprocess.run(["git", "add", "."], cwd=str(git_vault), capture_output=True, check=True)
        subprocess.run(
            ["git", "commit", "-m", "Set author"],
            cwd=str(git_vault), capture_output=True, check=True,
        )

        apply_git_meta(root=git_vault, overwrite=False)
        text = doc.read_text(encoding="utf-8")
        # Original author should be preserved
        assert "author: Brian" in text

    def test_apply_git_meta_overwrite_replaces_fields(self, git_vault: Path) -> None:
        doc = git_vault / "docs" / "guide.md"
        doc.write_text(
            _md(
                "id: docs/guide\ntitle: Guide\ndescription: Helpful guide\nauthor: Brian",
                "Guide body.\n",
            ),
            encoding="utf-8",
        )
        subprocess.run(["git", "add", "."], cwd=str(git_vault), capture_output=True, check=True)
        subprocess.run(
            ["git", "commit", "-m", "Set author"],
            cwd=str(git_vault), capture_output=True, check=True,
        )

        apply_git_meta(root=git_vault, overwrite=True)
        text = doc.read_text(encoding="utf-8")
        assert "author: Test Author" in text

    def test_cli_git_meta_json(self, git_vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
        exit_code = main(["--json", "git-meta", "--root", str(git_vault)])
        captured = capsys.readouterr()

        assert exit_code == 0
        payload = json.loads(captured.out)
        assert payload["ok"] is True

    def test_cli_git_apply_dry_run(self, git_vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
        exit_code = main(["--json", "git-apply", "--root", str(git_vault), "--dry-run"])
        captured = capsys.readouterr()

        assert exit_code == 0
        payload = json.loads(captured.out)
        assert payload["ok"] is True
        assert payload["result"]["dry_run"] is True


# ─── Context assembly tests ──────────────────────────────────────────


class TestContextAssembly:
    def _build_multi_doc_vault(self, vault: Path) -> None:
        _write(
            vault / "docs" / "guide.md",
            _md(
                """\
                id: docs/guide
                title: CUPED Guide
                description: Variance reduction methodology for experiments
                tags: [experimentation, cuped]
                category: reference
                status: active
                tokens: 500
                priority: 1
                scope: team
                domain: experimentation
                depends_on: [docs/stats-primer]
                """,
                "Guide body.\n",
            ),
        )
        _write(
            vault / "docs" / "stats-primer.md",
            _md(
                """\
                id: docs/stats-primer
                title: Statistics Primer
                description: Basic statistical concepts for experimentation
                tags: [statistics, experimentation]
                category: reference
                status: active
                tokens: 300
                priority: 2
                scope: team
                domain: experimentation
                """,
                "Stats primer body.\n",
            ),
        )
        _write(
            vault / "docs" / "runbook.md",
            _md(
                """\
                id: docs/runbook
                title: Deploy Runbook
                description: Deployment procedure for the pipeline
                tags: [deploy, infrastructure]
                category: runbook
                status: active
                tokens: 200
                priority: 3
                scope: org
                domain: infrastructure
                related: [docs/guide]
                """,
                "Runbook body.\n",
            ),
        )
        _write(
            vault / "docs" / "archived.md",
            _md(
                """\
                id: docs/archived
                title: Old Notes
                description: Deprecated notes
                tags: [experimentation]
                status: deprecated
                tokens: 100
                priority: 5
                """,
                "Archived.\n",
            ),
        )
        build_index(vault, full=True)

    def test_assemble_respects_token_budget(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(root=vault, budget=600)

        assert result["total_tokens"] <= 600
        assert result["remaining"] >= 0
        assert result["selected_count"] > 0

    def test_assemble_excludes_deprecated(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(root=vault, budget=10000)

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/archived" not in selected_ids

    def test_assemble_resolves_dependencies(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context("cuped", root=vault, budget=10000)

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/guide" in selected_ids
        # stats-primer should be pulled in as a dependency
        assert "docs/stats-primer" in selected_ids

    def test_assemble_pins_by_id(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(root=vault, budget=10000, ids=["docs/runbook"])

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/runbook" in selected_ids

    def test_assemble_filters_by_tag(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(root=vault, budget=10000, tags=["deploy"])

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/runbook" in selected_ids
        assert "docs/guide" not in selected_ids

    def test_assemble_filters_by_category(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(root=vault, budget=10000, category="runbook")

        selected_ids = {s["id"] for s in result["selected"]}
        assert selected_ids == {"docs/runbook"}

    def test_assemble_filters_by_domain(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(root=vault, budget=10000, domain="experimentation")

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/guide" in selected_ids
        assert "docs/stats-primer" in selected_ids
        assert "docs/runbook" not in selected_ids

    def test_assemble_includes_related(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(
            root=vault, budget=10000, ids=["docs/runbook"], include_related=True,
        )

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/runbook" in selected_ids
        assert "docs/guide" in selected_ids  # related to runbook

    def test_assemble_no_deps(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        result = assemble_context(
            "cuped", root=vault, budget=600, resolve_deps=False,
        )

        selected_ids = {s["id"] for s in result["selected"]}
        assert "docs/guide" in selected_ids
        # stats-primer should NOT be pulled in when deps disabled
        # (it doesn't match "cuped" query)
        assert "docs/stats-primer" not in selected_ids

    def test_assemble_raises_for_unknown_id(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        with pytest.raises(VaultliError, match="not found"):
            assemble_context(root=vault, ids=["nonexistent"])

    def test_assemble_priority_ordering(self, vault: Path) -> None:
        self._build_multi_doc_vault(vault)
        # Budget only fits 2 of the 3 active docs (500 + 300 = 800, leaves no room for 200)
        result = assemble_context(root=vault, budget=800, domain="experimentation")

        selected_ids = [s["id"] for s in result["selected"]]
        # guide (priority 1) should come before stats-primer (priority 2)
        assert selected_ids.index("docs/guide") < selected_ids.index("docs/stats-primer")

    def test_cli_assemble_json(self, vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
        self._build_multi_doc_vault(vault)
        exit_code = main([
            "--json", "assemble", "--root", str(vault), "--budget", "10000",
        ])
        captured = capsys.readouterr()

        assert exit_code == 0
        payload = json.loads(captured.out)
        assert payload["ok"] is True
        assert payload["result"]["budget"] == 10000

    def test_cli_assemble_with_filters(self, vault: Path, capsys: pytest.CaptureFixture[str]) -> None:
        self._build_multi_doc_vault(vault)
        exit_code = main([
            "--json", "assemble", "cuped", "--root", str(vault),
            "--budget", "5000", "--tag", "experimentation",
        ])
        captured = capsys.readouterr()

        assert exit_code == 0
        payload = json.loads(captured.out)
        assert payload["ok"] is True


# ─── Multi-vault federation tests ────────────────────────────────────


class TestFederation:
    @pytest.fixture
    def two_vaults(self, tmp_path: Path) -> tuple[Path, Path]:
        work = tmp_path / "work"
        personal = tmp_path / "personal"
        init_vault(work)
        init_vault(personal)

        _write(
            work / "docs" / "guide.md",
            _md(
                """\
                id: docs/guide
                title: Work Guide
                description: Work guide document
                category: reference
                tags: [work]
                """,
                "Work guide body.\n",
            ),
        )
        _write(
            work / "queries" / "report.sql",
            "select 1;\n",
        )
        _write(
            work / "queries" / "report.sql.md",
            _md(
                """\
                id: queries/report
                title: Report Query
                description: SQL report for analytics
                category: query
                source: ./report.sql
                tags: [sql, analytics]
                """,
                "Report docs.\n",
            ),
        )

        _write(
            personal / "notes" / "journal.md",
            _md(
                """\
                id: notes/journal
                title: Journal Entry
                description: Personal journal notes
                category: note
                tags: [personal]
                """,
                "Journal body.\n",
            ),
        )

        build_index(work, full=True)
        build_index(personal, full=True)

        return work, personal

    def test_federated_search_merges_results(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        result = federated_search(
            [{"path": str(work), "alias": "work"}, {"path": str(personal), "alias": "personal"}],
            query=None,
        )

        assert result["total"] == 3
        ids = {r["id"] for r in result["results"]}
        assert "work/docs/guide" in ids
        assert "work/queries/report" in ids
        assert "personal/notes/journal" in ids

    def test_federated_search_with_query(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        result = federated_search(
            [{"path": str(work), "alias": "work"}, {"path": str(personal), "alias": "personal"}],
            query="journal",
        )

        assert result["total"] == 1
        assert result["results"][0]["id"] == "personal/notes/journal"

    def test_federated_search_preserves_vault_metadata(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        result = federated_search(
            [{"path": str(work), "alias": "work"}, {"path": str(personal), "alias": "personal"}],
            query=None,
        )

        assert len(result["vaults"]) == 2
        prefixes = {v["prefix"] for v in result["vaults"]}
        assert prefixes == {"work", "personal"}

    def test_federated_load_detects_no_collisions(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        result = federated_load(
            [{"path": str(work), "alias": "work"}, {"path": str(personal), "alias": "personal"}],
        )

        assert result["total"] == 3
        assert result["collisions"] == {}

    def test_federated_records_have_vault_context(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        result = federated_load(
            [{"path": str(work), "alias": "work"}, {"path": str(personal), "alias": "personal"}],
        )

        for record in result["records"]:
            assert "_vault" in record
            assert "_vault_root" in record
            assert "_original_id" in record

    def test_federated_search_auto_prefixes_from_dirname(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        result = federated_search(
            [{"path": str(work)}, {"path": str(personal)}],
            query=None,
        )

        prefixes = {v["prefix"] for v in result["vaults"]}
        assert "work" in prefixes
        assert "personal" in prefixes

    def test_federated_search_handles_missing_vault(self, two_vaults: tuple[Path, Path]) -> None:
        work, _ = two_vaults
        result = federated_search(
            [{"path": str(work), "alias": "work"}, {"path": "/nonexistent"}],
            query=None,
        )

        assert len(result["errors"]) == 1
        assert result["total"] == 2  # only work vault records

    def test_resolve_federated_id(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        vault_specs = [
            {"path": str(work), "alias": "work"},
            {"path": str(personal), "alias": "personal"},
        ]

        result = resolve_federated_id("work/docs/guide", vault_specs)
        assert result["original_id"] == "docs/guide"
        assert result["vault_prefix"] == "work"
        assert result["record"]["title"] == "Work Guide"

    def test_resolve_federated_id_unknown_prefix(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        vault_specs = [
            {"path": str(work), "alias": "work"},
            {"path": str(personal), "alias": "personal"},
        ]

        with pytest.raises(VaultliError, match="No vault matches prefix"):
            resolve_federated_id("unknown/docs/guide", vault_specs)

    def test_resolve_federated_id_unknown_doc(self, two_vaults: tuple[Path, Path]) -> None:
        work, personal = two_vaults
        vault_specs = [
            {"path": str(work), "alias": "work"},
        ]

        with pytest.raises(VaultliError, match="not found"):
            resolve_federated_id("work/nonexistent", vault_specs)

    def test_no_vaults_raises(self) -> None:
        with pytest.raises(VaultliError, match="No vaults specified"):
            federated_search([], query=None)

    def test_cli_fed_search_json(
        self, two_vaults: tuple[Path, Path], capsys: pytest.CaptureFixture[str],
    ) -> None:
        work, personal = two_vaults
        exit_code = main([
            "--json", "fed-search",
            "--vault", f"work:{work}",
            "--vault", f"personal:{personal}",
        ])
        captured = capsys.readouterr()

        assert exit_code == 0
        payload = json.loads(captured.out)
        assert payload["ok"] is True
        assert payload["result"]["total"] == 3

    def test_cli_fed_load_json(
        self, two_vaults: tuple[Path, Path], capsys: pytest.CaptureFixture[str],
    ) -> None:
        work, personal = two_vaults
        exit_code = main([
            "--json", "fed-load",
            "--vault", f"work:{work}",
            "--vault", f"personal:{personal}",
        ])
        captured = capsys.readouterr()

        assert exit_code == 0
        payload = json.loads(captured.out)
        assert payload["ok"] is True

    def test_cli_fed_resolve_json(
        self, two_vaults: tuple[Path, Path], capsys: pytest.CaptureFixture[str],
    ) -> None:
        work, personal = two_vaults
        exit_code = main([
            "--json", "fed-resolve", "work/docs/guide",
            "--vault", f"work:{work}",
            "--vault", f"personal:{personal}",
        ])
        captured = capsys.readouterr()

        assert exit_code == 0
