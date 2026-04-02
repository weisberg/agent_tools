"""Context assembly for vaultli — knapsack algorithm for token-budget packing."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .core import (
    VaultliError,
    _resolve_root_hint,
    load_index_records,
)


def assemble_context(
    query: str | None = None,
    *,
    root: Path | str | None = None,
    budget: int = 4000,
    ids: list[str] | None = None,
    tags: list[str] | None = None,
    category: str | None = None,
    domain: str | None = None,
    scope: str | None = None,
    resolve_deps: bool = True,
    include_related: bool = False,
) -> dict[str, Any]:
    """Select documents that fit within a token budget.

    Uses a greedy knapsack approach: filter candidates, resolve dependencies,
    score by priority and relevance, then pack greedily until the budget is
    exhausted.

    Args:
        query: Optional keyword to filter candidates (case-insensitive substring).
        root: Vault root hint.
        budget: Maximum token count for the assembled context.
        ids: Explicitly include these document IDs (bypass filtering).
        tags: Only consider documents matching at least one of these tags.
        category: Only consider documents with this category.
        domain: Only consider documents with this domain.
        scope: Only consider documents with this scope or broader.
        resolve_deps: Automatically include depends_on targets.
        include_related: Also pull in related documents if budget allows.

    Returns:
        Dict with selected documents, total tokens, and budget info.
    """
    root_path = _resolve_root_hint(root)
    all_records = load_index_records(root_path)
    records_by_id: dict[str, dict[str, Any]] = {}
    for record in all_records:
        rid = record.get("id")
        if isinstance(rid, str):
            records_by_id[rid] = record

    # Phase 1: Collect explicitly requested IDs
    pinned_ids: set[str] = set()
    if ids:
        for doc_id in ids:
            if doc_id not in records_by_id:
                raise VaultliError(
                    f"Requested document not found in index: {doc_id!r}",
                    code="ID_NOT_FOUND",
                )
            pinned_ids.add(doc_id)

    # Phase 2: Filter candidates
    candidates = _filter_candidates(
        all_records,
        query=query,
        tags=tags,
        category=category,
        domain=domain,
        scope=scope,
    )
    candidate_ids = {r["id"] for r in candidates if isinstance(r.get("id"), str)}
    candidate_ids |= pinned_ids

    # Phase 3: Resolve dependencies
    if resolve_deps:
        dep_ids = _resolve_dependencies(candidate_ids, records_by_id)
        candidate_ids |= dep_ids

    if include_related:
        rel_ids = _resolve_related(candidate_ids, records_by_id)
        candidate_ids |= rel_ids

    # Phase 4: Score and sort
    scored = []
    for doc_id in candidate_ids:
        record = records_by_id.get(doc_id)
        if record is None:
            continue
        tokens = record.get("tokens", 0)
        if not isinstance(tokens, int) or tokens <= 0:
            tokens = 100  # default estimate for documents without token count
        score = _score_record(record, query=query, pinned=doc_id in pinned_ids)
        scored.append((score, tokens, doc_id, record))

    # Sort by score descending (higher is better), then by tokens ascending (prefer smaller)
    scored.sort(key=lambda x: (-x[0], x[1]))

    # Phase 5: Greedy knapsack packing
    selected: list[dict[str, Any]] = []
    total_tokens = 0
    selected_ids: set[str] = set()

    # First pass: pack pinned/dependency items (must-haves)
    must_have_ids = set(pinned_ids)
    if resolve_deps:
        must_have_ids |= _resolve_dependencies(pinned_ids, records_by_id)

    for _score, tokens, doc_id, record in scored:
        if doc_id in must_have_ids and total_tokens + tokens <= budget:
            selected.append(_selection_entry(record, tokens, "pinned"))
            total_tokens += tokens
            selected_ids.add(doc_id)

    # Second pass: pack remaining candidates greedily
    for _score, tokens, doc_id, record in scored:
        if doc_id in selected_ids:
            continue
        if total_tokens + tokens > budget:
            continue
        reason = "dependency" if doc_id not in candidate_ids else "match"
        selected.append(_selection_entry(record, tokens, reason))
        total_tokens += tokens
        selected_ids.add(doc_id)

    return {
        "root": str(root_path),
        "budget": budget,
        "total_tokens": total_tokens,
        "remaining": budget - total_tokens,
        "selected_count": len(selected),
        "candidate_count": len(candidate_ids),
        "selected": selected,
    }


def _filter_candidates(
    records: list[dict[str, Any]],
    *,
    query: str | None = None,
    tags: list[str] | None = None,
    category: str | None = None,
    domain: str | None = None,
    scope: str | None = None,
) -> list[dict[str, Any]]:
    """Filter records by query, tags, category, domain, and scope."""
    SCOPE_ORDER = ["personal", "team", "org", "public"]

    result = list(records)

    # Exclude deprecated/archived by default
    result = [
        r for r in result
        if r.get("status") not in ("deprecated", "archived")
    ]

    if query:
        needle = query.casefold()
        result = [
            r for r in result
            if (
                needle in str(r.get("title", "")).casefold()
                or needle in str(r.get("description", "")).casefold()
                or needle in str(r.get("id", "")).casefold()
                or any(needle in str(a).casefold() for a in (r.get("aliases") or []))
                or any(needle in str(t).casefold() for t in (r.get("tags") or []))
            )
        ]

    if tags:
        tag_set = {t.casefold() for t in tags}
        result = [
            r for r in result
            if tag_set & {str(t).casefold() for t in (r.get("tags") or [])}
        ]

    if category:
        result = [r for r in result if r.get("category") == category]

    if domain:
        result = [r for r in result if r.get("domain") == domain]

    if scope:
        try:
            max_idx = SCOPE_ORDER.index(scope)
        except ValueError:
            max_idx = len(SCOPE_ORDER)
        result = [
            r for r in result
            if SCOPE_ORDER.index(r.get("scope", "personal"))
            <= max_idx
            if r.get("scope", "personal") in SCOPE_ORDER
        ]

    return result


def _resolve_dependencies(
    ids: set[str],
    records_by_id: dict[str, dict[str, Any]],
    _visited: set[str] | None = None,
) -> set[str]:
    """Transitively resolve depends_on references."""
    if _visited is None:
        _visited = set()

    new_deps: set[str] = set()
    for doc_id in ids:
        if doc_id in _visited:
            continue
        _visited.add(doc_id)
        record = records_by_id.get(doc_id)
        if record is None:
            continue
        deps = record.get("depends_on") or []
        if isinstance(deps, list):
            for dep in deps:
                if isinstance(dep, str) and dep in records_by_id and dep not in ids:
                    new_deps.add(dep)

    if new_deps:
        new_deps |= _resolve_dependencies(new_deps, records_by_id, _visited)

    return new_deps


def _resolve_related(
    ids: set[str],
    records_by_id: dict[str, dict[str, Any]],
) -> set[str]:
    """Collect related references (one level, no transitive resolution)."""
    related: set[str] = set()
    for doc_id in ids:
        record = records_by_id.get(doc_id)
        if record is None:
            continue
        rels = record.get("related") or []
        if isinstance(rels, list):
            for rel in rels:
                if isinstance(rel, str) and rel in records_by_id and rel not in ids:
                    related.add(rel)
    return related


def _score_record(
    record: dict[str, Any],
    *,
    query: str | None = None,
    pinned: bool = False,
) -> float:
    """Score a record for selection priority. Higher is better."""
    score = 0.0

    # Pinned items get highest priority
    if pinned:
        score += 1000.0

    # Priority field: 1 (highest) -> +50, 5 (lowest) -> +10
    priority = record.get("priority")
    if isinstance(priority, int) and 1 <= priority <= 5:
        score += (6 - priority) * 10.0
    else:
        score += 20.0  # default: treat as priority 3

    # Active status gets a boost
    status = record.get("status")
    if status == "active":
        score += 15.0
    elif status == "draft":
        score += 5.0

    # Query match quality
    if query:
        needle = query.casefold()
        title = str(record.get("title", "")).casefold()
        desc = str(record.get("description", "")).casefold()
        doc_id = str(record.get("id", "")).casefold()

        if needle in title:
            score += 30.0
        if needle in desc:
            score += 20.0
        if needle in doc_id:
            score += 10.0

    return score


def _selection_entry(
    record: dict[str, Any],
    tokens: int,
    reason: str,
) -> dict[str, Any]:
    """Build a selection entry for the assembly result."""
    return {
        "id": record.get("id"),
        "title": record.get("title"),
        "file": record.get("file"),
        "tokens": tokens,
        "reason": reason,
    }
