"""Core implementation for the vaultli knowledge-vault CLI."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from dataclasses import dataclass, field
from datetime import date, datetime
from pathlib import Path
from typing import Any

import yaml

VAULT_MARKER = ".kbroot"
INDEX_FILENAME = "INDEX.jsonl"

FRONTMATTER_FIELD_ORDER = [
    "id",
    "title",
    "description",
    "tags",
    "category",
    "aliases",
    "author",
    "status",
    "created",
    "updated",
    "source",
    "depends_on",
    "related",
    "tokens",
    "priority",
    "scope",
    "domain",
    "version",
]
REQUIRED_FIELDS = ("id", "title", "description")
LIST_FIELDS = {"tags", "aliases", "depends_on", "related"}
STRING_FIELDS = {
    "id",
    "title",
    "description",
    "category",
    "author",
    "status",
    "source",
    "scope",
    "domain",
}
INTEGER_FIELDS = {"tokens", "priority"}
DATE_FIELDS = {"created", "updated"}
DOMAIN_CANDIDATES = {
    "experimentation",
    "marketing-analytics",
    "infrastructure",
    "tooling",
    "finance",
    "management",
}
COMMON_TAGS = {
    "md",
    "sql",
    "j2",
    "jinja",
    "jinja2",
    "json",
    "yaml",
    "yml",
    "txt",
    "docs",
    "doc",
    "templates",
    "template",
    "queries",
    "query",
    "skills",
    "runbooks",
}


class VaultliError(Exception):
    """Raised when vaultli cannot complete a requested operation."""

    def __init__(self, message: str, code: str = "VAULTLI_ERROR", details: dict[str, Any] | None = None):
        super().__init__(message)
        self.message = message
        self.code = code
        self.details = details or {}

    def to_dict(self) -> dict[str, Any]:
        payload = {"code": self.code, "message": self.message}
        if self.details:
            payload["details"] = self.details
        return payload


@dataclass(frozen=True)
class ParsedDocument:
    path: Path
    relative_path: str
    metadata: dict[str, Any]
    body: str
    has_frontmatter: bool

    @property
    def doc_id(self) -> str | None:
        value = self.metadata.get("id")
        return value if isinstance(value, str) and value.strip() else None

    @property
    def is_sidecar(self) -> bool:
        return is_sidecar_markdown(self.path)


@dataclass(frozen=True)
class ValidationIssue:
    code: str
    message: str
    file: str | None = None
    doc_id: str | None = None
    level: str = "error"

    def to_dict(self) -> dict[str, Any]:
        payload = {"code": self.code, "message": self.message, "level": self.level}
        if self.file:
            payload["file"] = self.file
        if self.doc_id:
            payload["id"] = self.doc_id
        return payload


@dataclass
class IndexBuildResult:
    root: str
    full: bool
    indexed: int = 0
    updated: int = 0
    pruned: int = 0
    skipped: int = 0
    warnings: list[dict[str, Any]] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "root": self.root,
            "full": self.full,
            "indexed": self.indexed,
            "updated": self.updated,
            "pruned": self.pruned,
            "skipped": self.skipped,
            "warnings": self.warnings,
        }


def find_root(start: Path | str | None = None) -> Path:
    """Walk upward from ``start`` until the vault marker is found."""

    current = Path.cwd() if start is None else Path(start).expanduser()
    current = current.resolve()

    for candidate in (current, *current.parents):
        if (candidate / VAULT_MARKER).exists():
            return candidate

    raise VaultliError(f"No {VAULT_MARKER} found from {current}", code="ROOT_NOT_FOUND")


def is_sidecar_markdown(path: Path | str) -> bool:
    """Return True when the path follows the sidecar ``<file>.<ext>.md`` pattern."""

    candidate = Path(path)
    return candidate.suffix.lower() == ".md" and "." in candidate.stem


def make_id(filepath: Path | str, root: Path | str = ".") -> str:
    """Derive a stable path-based ID from a file path relative to the vault root."""

    root_path = Path(root).expanduser().resolve()
    file_path = Path(filepath).expanduser()
    if not file_path.is_absolute():
        file_path = (Path.cwd() / file_path).resolve()
    else:
        file_path = file_path.resolve()

    try:
        relative_path = file_path.relative_to(root_path)
    except ValueError as exc:
        raise VaultliError(
            f"Cannot derive ID for {file_path} because it is outside vault root {root_path}",
            code="PATH_OUTSIDE_ROOT",
        ) from exc

    stem = relative_path.with_suffix("")
    if stem.suffix:
        stem = stem.with_suffix("")

    return str(stem.as_posix()).replace("_", "-").replace(" ", "-").lower()


def init_vault(target: Path | str = ".") -> dict[str, str]:
    """Create a new vault root with the marker file and empty JSONL index."""

    target_path = Path(target).expanduser().resolve()

    for candidate in (target_path, *target_path.parents):
        if (candidate / VAULT_MARKER).exists():
            raise VaultliError(f"Vault root already exists at {candidate}", code="ROOT_EXISTS")

    target_path.mkdir(parents=True, exist_ok=True)

    marker_path = target_path / VAULT_MARKER
    index_path = target_path / INDEX_FILENAME

    marker_path.touch(exist_ok=False)
    index_path.write_text("", encoding="utf-8")

    return {
        "root": str(target_path),
        "marker": str(marker_path),
        "index": str(index_path),
    }


def parse_markdown_file(path: Path | str, root: Path | str) -> ParsedDocument:
    """Parse a markdown file into frontmatter metadata and body."""

    file_path = Path(path).expanduser().resolve()
    root_path = Path(root).expanduser().resolve()
    if not file_path.exists():
        raise VaultliError(f"File not found: {file_path}", code="FILE_NOT_FOUND")
    if file_path.suffix.lower() != ".md":
        raise VaultliError(f"Expected a markdown file, got {file_path}", code="NOT_MARKDOWN")

    text = file_path.read_text(encoding="utf-8")
    metadata, body, has_frontmatter = _parse_frontmatter_text(text, file_path)
    relative_path = _relative_path(file_path, root_path)
    return ParsedDocument(
        path=file_path,
        relative_path=relative_path,
        metadata=metadata,
        body=body,
        has_frontmatter=has_frontmatter,
    )


def build_index(root: Path | str | None = None, full: bool = False) -> IndexBuildResult:
    """Build or rebuild the JSONL index for the vault."""

    root_path = _resolve_root_hint(root)
    existing_records = load_index_records(root_path)
    existing_by_id = {
        record["id"]: record for record in existing_records if isinstance(record.get("id"), str)
    }
    result = IndexBuildResult(root=str(root_path), full=full)

    emitted_ids: set[str] = set()
    records_to_write: list[dict[str, Any]] = []

    for md_path in iter_markdown_files(root_path):
        try:
            document = parse_markdown_file(md_path, root_path)
        except VaultliError as exc:
            result.warnings.append(
                ValidationIssue(code=exc.code, message=exc.message, file=_relative_path(md_path, root_path)).to_dict()
            )
            continue

        blocking_issues = _index_blocking_issues(document)
        if blocking_issues:
            result.warnings.extend(issue.to_dict() for issue in blocking_issues)
            continue

        assert document.doc_id is not None
        if document.doc_id in emitted_ids:
            result.warnings.append(
                ValidationIssue(
                    code="DUPLICATE_ID",
                    message=f"Duplicate id {document.doc_id!r} encountered during indexing",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                ).to_dict()
            )
            continue

        content_hash = compute_content_hash(document)
        record = build_index_record(document, content_hash)
        previous = existing_by_id.get(document.doc_id)
        emitted_ids.add(document.doc_id)

        if full:
            result.indexed += 1
            records_to_write.append(record)
            continue

        if previous is None:
            result.indexed += 1
            records_to_write.append(record)
            continue

        if previous == record:
            result.skipped += 1
            records_to_write.append(previous)
            continue

        result.updated += 1
        records_to_write.append(record)

    if full:
        result.pruned = 0
    else:
        result.pruned = len(set(existing_by_id) - emitted_ids)

    write_index_records(root_path, records_to_write)
    return result


def load_index_records(root: Path | str | None = None) -> list[dict[str, Any]]:
    """Load all index records from the vault root."""

    root_path = _resolve_root_hint(root)
    index_path = root_path / INDEX_FILENAME
    if not index_path.exists():
        raise VaultliError(f"Missing index file: {index_path}", code="INDEX_MISSING")

    records: list[dict[str, Any]] = []
    for raw_line in index_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line:
            continue
        payload = json.loads(line)
        if not isinstance(payload, dict):
            raise VaultliError("INDEX.jsonl contains a non-object record", code="INDEX_INVALID")
        records.append(payload)
    return records


def search_index(
    query: str | None = None,
    *,
    root: Path | str | None = None,
    jq_filter: str | None = None,
) -> list[dict[str, Any]]:
    """Search the index using a keyword query and optional jq filter."""

    records = load_index_records(root)
    if query:
        needle = query.casefold()
        records = [
            record
            for record in records
            if needle in json.dumps(record, sort_keys=True, ensure_ascii=False).casefold()
        ]

    if jq_filter:
        jq_path = shutil.which("jq")
        if jq_path is None:
            raise VaultliError("The `jq` executable is required for --jq filtering", code="JQ_UNAVAILABLE")

        payload = "".join(json.dumps(record, sort_keys=True) + "\n" for record in records)
        completed = subprocess.run(
            [jq_path, "-c", jq_filter],
            check=False,
            capture_output=True,
            text=True,
            input=payload,
        )
        if completed.returncode != 0:
            raise VaultliError(
                completed.stderr.strip() or "jq filter failed",
                code="JQ_FILTER_FAILED",
            )

        filtered: list[dict[str, Any]] = []
        for line in completed.stdout.splitlines():
            if not line.strip():
                continue
            parsed = json.loads(line)
            if not isinstance(parsed, dict):
                raise VaultliError("jq filter must emit JSON objects", code="JQ_FILTER_INVALID")
            filtered.append(parsed)
        records = filtered

    return records


def show_record(doc_id: str, *, root: Path | str | None = None) -> dict[str, Any]:
    """Show a single indexed document by id."""

    records = load_index_records(root)
    matches = [record for record in records if record.get("id") == doc_id]
    if not matches:
        raise VaultliError(f"No indexed document found for id {doc_id!r}", code="ID_NOT_FOUND")
    if len(matches) > 1:
        raise VaultliError(f"Multiple indexed documents found for id {doc_id!r}", code="DUPLICATE_ID")
    return matches[0]


def scaffold_file(file: Path | str, *, root: Path | str | None = None) -> dict[str, Any]:
    """Generate a frontmatter stub for markdown or a sidecar for non-markdown files."""

    target_path = Path(file).expanduser().resolve()
    if not target_path.exists():
        raise VaultliError(f"File not found: {target_path}", code="FILE_NOT_FOUND")
    if target_path.is_dir():
        raise VaultliError(f"Expected a file, got directory: {target_path}", code="NOT_A_FILE")

    root_path = _resolve_root_hint(root if root is not None else target_path.parent)
    metadata = infer_frontmatter(target_path, root_path)

    if target_path.suffix.lower() == ".md":
        document = parse_markdown_file(target_path, root_path)
        if document.has_frontmatter:
            raise VaultliError(
                f"Markdown file already contains frontmatter: {target_path}",
                code="FRONTMATTER_EXISTS",
            )
        target_path.write_text(render_document(metadata, document.body), encoding="utf-8")
        written_path = target_path
        mode = "frontmatter"
    else:
        sidecar_path = target_path.with_name(f"{target_path.name}.md")
        if sidecar_path.exists():
            raise VaultliError(f"Sidecar already exists: {sidecar_path}", code="SIDECAR_EXISTS")
        sidecar_body = _default_sidecar_body(target_path)
        sidecar_path.write_text(render_document(metadata, sidecar_body), encoding="utf-8")
        written_path = sidecar_path
        mode = "sidecar"

    return {
        "root": str(root_path),
        "mode": mode,
        "file": _relative_path(written_path, root_path),
        "id": metadata["id"],
        "metadata": ordered_metadata(metadata),
    }


def ingest_path(
    path: Path | str,
    *,
    root: Path | str | None = None,
    index: bool = False,
    dry_run: bool = False,
) -> dict[str, Any]:
    """Scaffold missing metadata for one file or every eligible file under a directory."""

    target_path = Path(path).expanduser().resolve()
    if not target_path.exists():
        raise VaultliError(f"File not found: {target_path}", code="FILE_NOT_FOUND")

    root_path = _resolve_root_hint(root if root is not None else (target_path if target_path.is_dir() else target_path.parent))
    candidates = _ingest_candidates(target_path, root_path)
    scaffolded: list[dict[str, Any]] = []
    skipped: list[dict[str, Any]] = []
    errors: list[dict[str, Any]] = []

    for candidate in candidates:
        try:
            planned = _plan_scaffold(candidate, root_path)
            if dry_run:
                scaffolded.append(planned)
            else:
                scaffolded.append(scaffold_file(candidate, root=root_path))
        except VaultliError as exc:
            entry = {
                "file": _relative_path(candidate, root_path),
                "code": exc.code,
                "message": exc.message,
            }
            if exc.code in {"FRONTMATTER_EXISTS", "SIDECAR_EXISTS", "SIDECAR_MARKDOWN"}:
                skipped.append(entry)
            else:
                errors.append(entry)

    result: dict[str, Any] = {
        "root": str(root_path),
        "path": _relative_path(target_path, root_path),
        "dry_run": dry_run,
        "indexed": False,
        "total": len(candidates),
        "scaffolded": scaffolded,
        "skipped": skipped,
        "errors": errors,
    }
    if index and not dry_run:
        result["index"] = build_index(root_path, full=False).to_dict()
        result["indexed"] = True
    return result


def add_file(file: Path | str, *, root: Path | str | None = None) -> dict[str, Any]:
    """Add metadata to a file and rebuild the index."""

    scaffolded = scaffold_file(file, root=root)
    root_path = Path(scaffolded["root"])
    index_result = build_index(root_path, full=False)
    return {
        "root": scaffolded["root"],
        "file": scaffolded["file"],
        "id": scaffolded["id"],
        "mode": scaffolded["mode"],
        "index": index_result.to_dict(),
    }


def validate_vault(root: Path | str | None = None) -> dict[str, Any]:
    """Audit the vault for integrity issues described by the spec."""

    root_path = _resolve_root_hint(root)
    issues: list[ValidationIssue] = []
    parsed_documents: list[ParsedDocument] = []

    for md_path in iter_markdown_files(root_path):
        try:
            document = parse_markdown_file(md_path, root_path)
        except VaultliError as exc:
            issues.append(
                ValidationIssue(code=exc.code, message=exc.message, file=_relative_path(md_path, root_path))
            )
            continue

        parsed_documents.append(document)
        issues.extend(_document_validation_issues(document))

        if document.is_sidecar:
            sibling_source = document.path.with_suffix("")
            if not sibling_source.exists():
                issues.append(
                    ValidationIssue(
                        code="ORPHANED_SIDECAR",
                        message=f"Sidecar has no sibling source asset: {sibling_source.name}",
                        file=document.relative_path,
                        doc_id=document.doc_id,
                    )
                )

    ids_to_files: dict[str, list[ParsedDocument]] = {}
    for document in parsed_documents:
        if document.doc_id:
            ids_to_files.setdefault(document.doc_id, []).append(document)

    for doc_id, documents in ids_to_files.items():
        if len(documents) > 1:
            for document in documents:
                issues.append(
                    ValidationIssue(
                        code="DUPLICATE_ID",
                        message=f"Duplicate id {doc_id!r} declared by multiple files",
                        file=document.relative_path,
                        doc_id=doc_id,
                    )
                )

    referenceable_ids = {
        document.doc_id
        for document in parsed_documents
        if document.doc_id and not _index_blocking_issues(document)
    }

    for document in parsed_documents:
        doc_id = document.doc_id
        depends_on = document.metadata.get("depends_on") or []
        related = document.metadata.get("related") or []
        if isinstance(depends_on, list):
            for target in depends_on:
                if isinstance(target, str) and target not in referenceable_ids:
                    issues.append(
                        ValidationIssue(
                            code="DANGLING_DEPENDENCY",
                            message=f"depends_on reference {target!r} does not resolve",
                            file=document.relative_path,
                            doc_id=doc_id,
                        )
                    )
        if isinstance(related, list):
            for target in related:
                if isinstance(target, str) and target not in referenceable_ids:
                    issues.append(
                        ValidationIssue(
                            code="DANGLING_RELATED",
                            message=f"related reference {target!r} does not resolve",
                            file=document.relative_path,
                            doc_id=doc_id,
                        )
                    )

    issues.extend(_index_staleness_issues(root_path, parsed_documents))
    deduped_issues = _dedupe_issues(issues)
    return {
        "root": str(root_path),
        "valid": not deduped_issues,
        "issue_count": len(deduped_issues),
        "issues": [issue.to_dict() for issue in deduped_issues],
    }


def infer_frontmatter(path: Path | str, root: Path | str) -> dict[str, Any]:
    """Infer scaffold metadata for a file or source asset."""

    target_path = Path(path).expanduser().resolve()
    root_path = Path(root).expanduser().resolve()
    today = date.today().isoformat()
    category = infer_category(target_path)
    title = infer_title(target_path)
    description = infer_description(target_path, category, title)
    tags = infer_tags(target_path, category)
    domain = infer_domain(target_path)
    token_count = estimate_tokens(_sample_text_for_inference(target_path))

    metadata: dict[str, Any] = {
        "id": make_id(target_path, root_path),
        "title": title,
        "description": description,
        "tags": tags,
        "category": category,
        "status": "draft",
        "created": today,
        "updated": today,
        "tokens": token_count,
        "priority": 3,
        "scope": "personal",
    }
    if domain:
        metadata["domain"] = domain
    if target_path.suffix.lower() != ".md":
        metadata["source"] = f"./{target_path.name}"
    return ordered_metadata(metadata)


def infer_category(path: Path | str) -> str:
    """Infer a category from filename and extension."""

    target = Path(path)
    parts = {part.lower() for part in target.parts}
    name = target.name.lower()
    stem = target.stem.lower()
    suffix = target.suffix.lower()

    if suffix == ".md":
        if name == "skill.md" or "skills" in parts:
            return "skill"
        if "runbooks" in parts or "runbook" in stem:
            return "runbook"
        if "tutorial" in stem:
            return "tutorial"
        if "guide" in stem or "reference" in stem or "readme" in stem or "spec" in stem:
            return "reference"
        return "note"

    if suffix == ".sql":
        return "query"
    if suffix in {".j2", ".jinja", ".jinja2"}:
        return "template"
    if suffix in {".py", ".json", ".yaml", ".yml", ".toml"}:
        return "reference"
    return "reference"


def infer_title(path: Path | str) -> str:
    """Infer a title from a filename."""

    target = Path(path)
    base = target.stem
    if target.suffix.lower() == ".md" and "." in base:
        base = Path(base).stem
    return " ".join(token.capitalize() for token in base.replace("-", " ").replace("_", " ").split())


def infer_description(path: Path | str, category: str, title: str | None = None) -> str:
    """Infer a one-sentence description for scaffolding."""

    target = Path(path)
    display_title = title or infer_title(target)
    if category == "query":
        return f"SQL query asset for {display_title.lower()} stored in the vault."
    if category == "template":
        return f"Template asset for {display_title.lower()} stored in the vault."
    if category == "skill":
        return f"Skill definition for {display_title.lower()} stored in the vault."
    if category == "runbook":
        return f"Runbook documenting {display_title.lower()} for the vault."
    return f"Markdown document for {display_title.lower()} stored in the vault."


def infer_tags(path: Path | str, category: str) -> list[str]:
    """Infer a small, deterministic tag list from path components."""

    target = Path(path)
    tokens: list[str] = []
    for part in target.parts[:-1]:
        tokens.extend(_slug_tokens(part))
    tokens.extend(_slug_tokens(target.stem))
    tokens.append(category)

    deduped: list[str] = []
    for token in tokens:
        if not token or token in COMMON_TAGS:
            continue
        if token not in deduped:
            deduped.append(token)
    return deduped[:8]


def infer_domain(path: Path | str) -> str | None:
    """Infer a domain from well-known directory names."""

    target = Path(path)
    for part in target.parts:
        normalized = part.lower().replace("_", "-").replace(" ", "-")
        if normalized in DOMAIN_CANDIDATES:
            return normalized
    if "tools" in {part.lower() for part in target.parts}:
        return "tooling"
    return None


def estimate_tokens(text: str) -> int:
    """Estimate token count from plain text."""

    words = len(text.split())
    if words == 0:
        return 0
    return max(1, int(words * 1.3))


def compute_content_hash(document: ParsedDocument) -> str:
    """Compute the spec-defined content hash for a parsed document."""

    source = document.metadata.get("source")
    if isinstance(source, str) and source.strip():
        target = (document.path.parent / source).resolve()
        if not target.exists():
            raise VaultliError(
                f"Source target does not exist for {document.relative_path}: {source}",
                code="BROKEN_SOURCE",
            )
        return hashlib.sha256(target.read_bytes()).hexdigest()[:12]
    return hashlib.sha256(document.body.encode("utf-8")).hexdigest()[:12]


def build_index_record(document: ParsedDocument, content_hash: str) -> dict[str, Any]:
    """Create a serialized index record for a parsed document."""

    record = ordered_metadata(document.metadata)
    record["file"] = document.relative_path
    record["hash"] = content_hash
    return record


def iter_markdown_files(root: Path | str) -> list[Path]:
    """Return all markdown files in the vault, excluding derived artifacts."""

    root_path = Path(root).expanduser().resolve()
    return sorted(
        path
        for path in root_path.rglob("*.md")
        if path.is_file() and path.name != INDEX_FILENAME
    )


def ordered_metadata(metadata: dict[str, Any]) -> dict[str, Any]:
    """Return metadata in schema order with unknown fields appended."""

    ordered: dict[str, Any] = {}
    for field in FRONTMATTER_FIELD_ORDER:
        if field in metadata and metadata[field] is not None:
            ordered[field] = _normalize_value(metadata[field])
    for key, value in metadata.items():
        if key in {"file", "hash"}:
            continue
        if key not in ordered and value is not None:
            ordered[key] = _normalize_value(value)
    return ordered


def render_document(metadata: dict[str, Any], body: str) -> str:
    """Render a markdown document with ordered YAML frontmatter."""

    frontmatter = yaml.safe_dump(
        ordered_metadata(metadata),
        sort_keys=False,
        allow_unicode=False,
        default_flow_style=False,
    ).strip()
    if body and not body.startswith("\n"):
        body = "\n" + body
    return f"---\n{frontmatter}\n---{body}"


def write_index_records(root: Path | str, records: list[dict[str, Any]]) -> None:
    """Atomically write JSONL records to INDEX.jsonl."""

    root_path = Path(root).expanduser().resolve()
    index_path = root_path / INDEX_FILENAME
    tmp_path = root_path / f"{INDEX_FILENAME}.tmp"
    lines = [json.dumps(record, sort_keys=False, ensure_ascii=False) for record in records]
    with tmp_path.open("w", encoding="utf-8") as handle:
        for line in lines:
            handle.write(line + "\n")
    tmp_path.replace(index_path)


def _parse_frontmatter_text(text: str, path: Path) -> tuple[dict[str, Any], str, bool]:
    lines = text.splitlines(keepends=True)
    if not lines or lines[0].strip() != "---":
        return {}, text, False

    closing_index: int | None = None
    for index in range(1, len(lines)):
        if lines[index].strip() == "---":
            closing_index = index
            break

    if closing_index is None:
        raise VaultliError(f"Malformed frontmatter in {path}", code="MALFORMED_FRONTMATTER")

    raw_metadata = "".join(lines[1:closing_index])
    raw_body = "".join(lines[closing_index + 1 :])
    try:
        parsed = yaml.safe_load(raw_metadata) or {}
    except yaml.YAMLError as exc:
        raise VaultliError(f"Invalid YAML frontmatter in {path}: {exc}", code="INVALID_FRONTMATTER") from exc

    if not isinstance(parsed, dict):
        raise VaultliError(
            f"Frontmatter must deserialize to a mapping in {path}",
            code="INVALID_FRONTMATTER",
        )

    return ordered_metadata(parsed), raw_body, True


def _index_blocking_issues(document: ParsedDocument) -> list[ValidationIssue]:
    issues: list[ValidationIssue] = []
    metadata = document.metadata

    missing_required = [field for field in REQUIRED_FIELDS if not metadata.get(field)]
    if missing_required:
        issues.append(
            ValidationIssue(
                code="MISSING_REQUIRED_FIELDS",
                message=f"Missing required fields: {', '.join(missing_required)}",
                file=document.relative_path,
                doc_id=document.doc_id,
            )
        )

    source = metadata.get("source")
    if document.is_sidecar and not source:
        issues.append(
            ValidationIssue(
                code="MISSING_SOURCE_FIELD",
                message="Sidecar markdown is missing required source field",
                file=document.relative_path,
                doc_id=document.doc_id,
            )
        )

    if isinstance(source, str) and source.strip():
        source_path = (document.path.parent / source).resolve()
        if not source_path.exists():
            issues.append(
                ValidationIssue(
                    code="BROKEN_SOURCE",
                    message=f"source target does not exist: {source}",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                )
            )

    return issues


def _document_validation_issues(document: ParsedDocument) -> list[ValidationIssue]:
    issues = _index_blocking_issues(document)
    metadata = document.metadata

    for field in LIST_FIELDS:
        if field in metadata and not isinstance(metadata[field], list):
            issues.append(
                ValidationIssue(
                    code="INVALID_FIELD_TYPE",
                    message=f"Field {field!r} must be a list",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                )
            )

    for field in STRING_FIELDS:
        if field in metadata and not isinstance(metadata[field], str):
            issues.append(
                ValidationIssue(
                    code="INVALID_FIELD_TYPE",
                    message=f"Field {field!r} must be a string",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                )
            )

    for field in INTEGER_FIELDS:
        if field in metadata and not isinstance(metadata[field], int):
            issues.append(
                ValidationIssue(
                    code="INVALID_FIELD_TYPE",
                    message=f"Field {field!r} must be an integer",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                )
            )

    priority = metadata.get("priority")
    if isinstance(priority, int) and not (1 <= priority <= 5):
        issues.append(
            ValidationIssue(
                code="INVALID_PRIORITY",
                message="priority must be between 1 and 5",
                file=document.relative_path,
                doc_id=document.doc_id,
            )
        )

    for field in DATE_FIELDS:
        if field in metadata and not _is_iso_date_like(metadata[field]):
            issues.append(
                ValidationIssue(
                    code="INVALID_DATE",
                    message=f"Field {field!r} must be an ISO date string (YYYY-MM-DD)",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                )
            )

    return issues


def _index_staleness_issues(root: Path, documents: list[ParsedDocument]) -> list[ValidationIssue]:
    issues: list[ValidationIssue] = []
    index_path = root / INDEX_FILENAME
    if not index_path.exists():
        issues.append(
            ValidationIssue(
                code="MISSING_INDEX",
                message="INDEX.jsonl is missing",
                file=INDEX_FILENAME,
            )
        )
        return issues

    indexed_records = load_index_records(root)
    indexed_by_id = {
        record["id"]: record for record in indexed_records if isinstance(record.get("id"), str)
    }

    valid_documents: list[ParsedDocument] = []
    seen_ids: set[str] = set()
    for document in documents:
        if document.doc_id is None or _index_blocking_issues(document):
            continue
        if document.doc_id in seen_ids:
            continue
        seen_ids.add(document.doc_id)
        valid_documents.append(document)

    valid_ids = {document.doc_id for document in valid_documents if document.doc_id}
    indexed_ids = set(indexed_by_id)

    for document in valid_documents:
        assert document.doc_id is not None
        current_record = build_index_record(document, compute_content_hash(document))
        existing = indexed_by_id.get(document.doc_id)
        if existing != current_record:
            issues.append(
                ValidationIssue(
                    code="STALE_INDEX",
                    message=f"Index entry is stale for {document.doc_id}",
                    file=document.relative_path,
                    doc_id=document.doc_id,
                )
            )

    for stale_id in sorted(indexed_ids - valid_ids):
        stale_record = indexed_by_id[stale_id]
        issues.append(
            ValidationIssue(
                code="STALE_INDEX",
                message=f"Index contains removed or invalid record for {stale_id}",
                file=stale_record.get("file"),
                doc_id=stale_id,
            )
        )

    return issues


def _resolve_root_hint(root: Path | str | None) -> Path:
    if root is None:
        return find_root()
    root_path = Path(root).expanduser().resolve()
    if (root_path / VAULT_MARKER).exists():
        return root_path
    return find_root(root_path)


def _relative_path(path: Path, root: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _ingest_candidates(target_path: Path, root_path: Path) -> list[Path]:
    if target_path.is_file():
        return [target_path]
    if not target_path.is_dir():
        raise VaultliError(f"Expected a file, got directory: {target_path}", code="NOT_A_FILE")

    candidates: list[Path] = []
    for path in sorted(candidate for candidate in target_path.rglob("*") if candidate.is_file()):
        if _should_skip_ingest_file(path, root_path):
            continue
        candidates.append(path)
    return candidates


def _should_skip_ingest_file(path: Path, root_path: Path) -> bool:
    relative = path.resolve().relative_to(root_path.resolve())
    if path.name in {VAULT_MARKER, INDEX_FILENAME, f"{INDEX_FILENAME}.tmp"}:
        return True
    if any(part.startswith(".") for part in relative.parts):
        return True
    if is_sidecar_markdown(path):
        return True
    return False


def _plan_scaffold(candidate: Path, root_path: Path) -> dict[str, Any]:
    if is_sidecar_markdown(candidate):
        raise VaultliError(
            f"Sidecar markdown is not scaffolded directly: {candidate}",
            code="SIDECAR_MARKDOWN",
        )

    metadata = infer_frontmatter(candidate, root_path)
    if candidate.suffix.lower() == ".md":
        document = parse_markdown_file(candidate, root_path)
        if document.has_frontmatter:
            raise VaultliError(
                f"Markdown file already contains frontmatter: {candidate}",
                code="FRONTMATTER_EXISTS",
            )
        mode = "frontmatter"
        written_path = candidate
    else:
        written_path = candidate.with_name(f"{candidate.name}.md")
        if written_path.exists():
            raise VaultliError(f"Sidecar already exists: {written_path}", code="SIDECAR_EXISTS")
        mode = "sidecar"

    return {
        "root": str(root_path),
        "mode": mode,
        "file": _relative_path(written_path, root_path),
        "id": metadata["id"],
        "metadata": ordered_metadata(metadata),
    }


def _normalize_value(value: Any) -> Any:
    if isinstance(value, datetime):
        return value.date().isoformat()
    if isinstance(value, date):
        return value.isoformat()
    if isinstance(value, list):
        return [_normalize_value(item) for item in value]
    if isinstance(value, dict):
        return {key: _normalize_value(inner) for key, inner in value.items()}
    return value


def _slug_tokens(raw: str) -> list[str]:
    cleaned = raw.replace(".", " ").replace("-", " ").replace("_", " ").lower()
    return [token for token in cleaned.split() if token]


def _sample_text_for_inference(target_path: Path) -> str:
    try:
        if target_path.suffix.lower() == ".md":
            return target_path.read_text(encoding="utf-8")
        return target_path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return ""


def _default_sidecar_body(source_path: Path) -> str:
    return (
        "\n## Purpose\n\n"
        f"Describe the purpose and usage of `{source_path.name}`.\n"
    )


def _is_iso_date_like(value: Any) -> bool:
    if isinstance(value, (date, datetime)):
        return True
    if not isinstance(value, str):
        return False
    try:
        date.fromisoformat(value)
    except ValueError:
        return False
    return True


def _dedupe_issues(issues: list[ValidationIssue]) -> list[ValidationIssue]:
    seen: set[tuple[str, str | None, str | None, str]] = set()
    deduped: list[ValidationIssue] = []
    for issue in issues:
        key = (issue.code, issue.file, issue.doc_id, issue.message)
        if key in seen:
            continue
        seen.add(key)
        deduped.append(issue)
    return deduped
