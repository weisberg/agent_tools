"""Git integration for vaultli — extract created/updated/author from local git history."""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Any

from .core import (
    VaultliError,
    _resolve_root_hint,
    iter_markdown_files,
    load_index_records,
    parse_markdown_file,
    ordered_metadata,
    render_document,
    write_index_records,
)


@dataclass(frozen=True)
class GitMeta:
    """Metadata extracted from git history for a single file."""

    created: date | None = None
    updated: date | None = None
    author: str | None = None

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        if self.created is not None:
            result["created"] = self.created.isoformat()
        if self.updated is not None:
            result["updated"] = self.updated.isoformat()
        if self.author is not None:
            result["author"] = self.author
        return result


def _is_git_repo(path: Path) -> bool:
    """Check if the given path is inside a git repository."""
    try:
        result = subprocess.run(
            ["git", "-C", str(path), "rev-parse", "--git-dir"],
            capture_output=True,
            text=True,
            check=False,
        )
        return result.returncode == 0
    except FileNotFoundError:
        return False


def _git_log_dates(path: Path, repo_dir: Path) -> tuple[date | None, date | None]:
    """Return (first_commit_date, last_commit_date) for a file using git log."""
    result = subprocess.run(
        [
            "git", "-C", str(repo_dir),
            "log", "--follow", "--format=%aI", "--", str(path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None, None

    lines = [line.strip() for line in result.stdout.strip().splitlines() if line.strip()]
    if not lines:
        return None, None

    # git log outputs newest first; last line is the oldest commit
    newest = date.fromisoformat(lines[0][:10])
    oldest = date.fromisoformat(lines[-1][:10])
    return oldest, newest


def _git_first_author(path: Path, repo_dir: Path) -> str | None:
    """Return the author of the first commit that introduced the file."""
    result = subprocess.run(
        [
            "git", "-C", str(repo_dir),
            "log", "--follow", "--diff-filter=A", "--format=%aN", "--", str(path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0 or not result.stdout.strip():
        # Fallback: use the oldest commit author
        result = subprocess.run(
            [
                "git", "-C", str(repo_dir),
                "log", "--follow", "--reverse", "--format=%aN", "--", str(path),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0 or not result.stdout.strip():
            return None

    lines = [line.strip() for line in result.stdout.strip().splitlines() if line.strip()]
    return lines[0] if lines else None


def git_meta_for_file(path: Path, repo_dir: Path) -> GitMeta:
    """Extract git metadata for a single file."""
    abs_path = path.resolve()
    created, updated = _git_log_dates(abs_path, repo_dir)
    author = _git_first_author(abs_path, repo_dir)
    return GitMeta(created=created, updated=updated, author=author)


def git_meta_for_vault(
    *,
    root: Path | str | None = None,
) -> dict[str, dict[str, Any]]:
    """Extract git metadata for all markdown files in the vault.

    Returns a dict keyed by relative file path with git metadata values.
    """
    root_path = _resolve_root_hint(root)

    if not _is_git_repo(root_path):
        raise VaultliError(
            f"Vault root is not inside a git repository: {root_path}",
            code="NOT_A_GIT_REPO",
        )

    results: dict[str, dict[str, Any]] = {}
    for md_path in iter_markdown_files(root_path):
        rel = md_path.resolve().relative_to(root_path.resolve()).as_posix()
        meta = git_meta_for_file(md_path, root_path)
        if meta.created or meta.updated or meta.author:
            results[rel] = meta.to_dict()

    return results


def apply_git_meta(
    *,
    root: Path | str | None = None,
    overwrite: bool = False,
    dry_run: bool = False,
) -> dict[str, Any]:
    """Apply git-derived created/updated/author to vault files and re-index.

    By default, only fills in fields that are missing from the frontmatter.
    With overwrite=True, replaces existing values with git-derived ones.
    """
    root_path = _resolve_root_hint(root)

    if not _is_git_repo(root_path):
        raise VaultliError(
            f"Vault root is not inside a git repository: {root_path}",
            code="NOT_A_GIT_REPO",
        )

    updated_files: list[dict[str, Any]] = []
    skipped_files: list[str] = []

    for md_path in iter_markdown_files(root_path):
        try:
            doc = parse_markdown_file(md_path, root_path)
        except VaultliError:
            continue

        if not doc.has_frontmatter:
            skipped_files.append(doc.relative_path)
            continue

        meta = git_meta_for_file(md_path, root_path)
        changes: dict[str, str] = {}

        if meta.created and (overwrite or not doc.metadata.get("created")):
            new_val = meta.created.isoformat()
            old_val = doc.metadata.get("created")
            if str(old_val) != new_val:
                changes["created"] = new_val

        if meta.updated and (overwrite or not doc.metadata.get("updated")):
            new_val = meta.updated.isoformat()
            old_val = doc.metadata.get("updated")
            if str(old_val) != new_val:
                changes["updated"] = new_val

        if meta.author and (overwrite or not doc.metadata.get("author")):
            if doc.metadata.get("author") != meta.author:
                changes["author"] = meta.author

        if not changes:
            skipped_files.append(doc.relative_path)
            continue

        if not dry_run:
            new_metadata = dict(doc.metadata)
            new_metadata.update(changes)
            md_path.write_text(
                render_document(new_metadata, doc.body),
                encoding="utf-8",
            )

        updated_files.append({
            "file": doc.relative_path,
            "changes": changes,
        })

    return {
        "root": str(root_path),
        "overwrite": overwrite,
        "dry_run": dry_run,
        "updated": len(updated_files),
        "skipped": len(skipped_files),
        "files": updated_files,
    }
