"""Multi-vault federation for vaultli — merged search across multiple vaults."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .core import (
    VAULT_MARKER,
    VaultliError,
    load_index_records,
    search_index,
)


def _validate_vault_root(path: Path) -> Path:
    """Validate that a path is a vault root (contains .kbroot)."""
    resolved = path.expanduser().resolve()
    if not resolved.is_dir():
        raise VaultliError(
            f"Vault path is not a directory: {resolved}",
            code="NOT_A_DIRECTORY",
        )
    if not (resolved / VAULT_MARKER).exists():
        raise VaultliError(
            f"No {VAULT_MARKER} found at {resolved}",
            code="ROOT_NOT_FOUND",
        )
    return resolved


def _vault_prefix(vault_path: Path, alias: str | None = None) -> str:
    """Derive a vault prefix from an alias or the directory name."""
    if alias:
        return alias
    return vault_path.name


def _prefix_record(
    record: dict[str, Any],
    prefix: str,
    vault_root: str,
) -> dict[str, Any]:
    """Add vault prefix to a record's id and file path for disambiguation."""
    prefixed = dict(record)
    original_id = record.get("id", "")
    prefixed["id"] = f"{prefix}/{original_id}"
    prefixed["_original_id"] = original_id
    prefixed["_vault"] = prefix
    prefixed["_vault_root"] = vault_root

    # Prefix file path
    original_file = record.get("file", "")
    if original_file:
        prefixed["file"] = f"{prefix}/{original_file}"
        prefixed["_original_file"] = original_file

    # Prefix depends_on and related references
    for ref_field in ("depends_on", "related"):
        refs = record.get(ref_field)
        if isinstance(refs, list):
            prefixed[ref_field] = [
                f"{prefix}/{ref}" if isinstance(ref, str) else ref
                for ref in refs
            ]

    return prefixed


def federated_search(
    vaults: list[dict[str, str]],
    query: str | None = None,
    *,
    jq_filter: str | None = None,
) -> dict[str, Any]:
    """Search across multiple vaults with federated results.

    Args:
        vaults: List of vault specs, each a dict with "path" (required)
                and optional "alias" for the vault prefix.
        query: Keyword search query.
        jq_filter: Optional jq filter expression.

    Returns:
        Federated result dict with merged records and per-vault metadata.
    """
    if not vaults:
        raise VaultliError("No vaults specified for federation", code="NO_VAULTS")

    merged_records: list[dict[str, Any]] = []
    vault_metadata: list[dict[str, Any]] = []
    errors: list[dict[str, Any]] = []

    for vault_spec in vaults:
        vault_path_str = vault_spec.get("path")
        if not vault_path_str:
            errors.append({"error": "Missing 'path' in vault spec", "spec": vault_spec})
            continue

        alias = vault_spec.get("alias")

        try:
            vault_path = _validate_vault_root(Path(vault_path_str))
            prefix = _vault_prefix(vault_path, alias)
            records = search_index(query, root=vault_path, jq_filter=jq_filter)

            prefixed = [
                _prefix_record(r, prefix, str(vault_path))
                for r in records
            ]
            merged_records.extend(prefixed)

            vault_metadata.append({
                "prefix": prefix,
                "path": str(vault_path),
                "record_count": len(prefixed),
            })
        except VaultliError as exc:
            errors.append({
                "path": vault_path_str,
                "error": exc.to_dict(),
            })

    return {
        "total": len(merged_records),
        "vaults": vault_metadata,
        "results": merged_records,
        "errors": errors,
    }


def federated_load(
    vaults: list[dict[str, str]],
) -> dict[str, Any]:
    """Load and merge all index records from multiple vaults.

    Args:
        vaults: List of vault specs with "path" and optional "alias".

    Returns:
        Dict with all merged records, vault metadata, and any errors.
    """
    if not vaults:
        raise VaultliError("No vaults specified for federation", code="NO_VAULTS")

    merged_records: list[dict[str, Any]] = []
    vault_metadata: list[dict[str, Any]] = []
    errors: list[dict[str, Any]] = []

    for vault_spec in vaults:
        vault_path_str = vault_spec.get("path")
        if not vault_path_str:
            errors.append({"error": "Missing 'path' in vault spec", "spec": vault_spec})
            continue

        alias = vault_spec.get("alias")

        try:
            vault_path = _validate_vault_root(Path(vault_path_str))
            prefix = _vault_prefix(vault_path, alias)
            records = load_index_records(vault_path)

            prefixed = [
                _prefix_record(r, prefix, str(vault_path))
                for r in records
            ]
            merged_records.extend(prefixed)

            vault_metadata.append({
                "prefix": prefix,
                "path": str(vault_path),
                "record_count": len(prefixed),
            })
        except VaultliError as exc:
            errors.append({
                "path": vault_path_str,
                "error": exc.to_dict(),
            })

    # Check for ID collisions across vaults
    id_sources: dict[str, list[str]] = {}
    for record in merged_records:
        rid = record.get("id", "")
        vault = record.get("_vault", "")
        id_sources.setdefault(rid, []).append(vault)

    collisions = {
        rid: sources for rid, sources in id_sources.items() if len(sources) > 1
    }

    return {
        "total": len(merged_records),
        "vaults": vault_metadata,
        "records": merged_records,
        "collisions": collisions,
        "errors": errors,
    }


def resolve_federated_id(
    federated_id: str,
    vaults: list[dict[str, str]],
) -> dict[str, Any]:
    """Resolve a prefixed federated ID back to its vault and original record.

    Args:
        federated_id: A prefixed ID like "work/queries/retention-holdout".
        vaults: List of vault specs.

    Returns:
        The original record with vault context.
    """
    parts = federated_id.split("/", 1)
    if len(parts) < 2:
        raise VaultliError(
            f"Federated ID must have a vault prefix: {federated_id!r}",
            code="INVALID_FEDERATED_ID",
        )

    prefix, original_id = parts

    for vault_spec in vaults:
        vault_path_str = vault_spec.get("path")
        if not vault_path_str:
            continue

        alias = vault_spec.get("alias")
        vault_path = _validate_vault_root(Path(vault_path_str))
        vault_prefix = _vault_prefix(vault_path, alias)

        if vault_prefix == prefix:
            records = load_index_records(vault_path)
            for record in records:
                if record.get("id") == original_id:
                    return {
                        "vault_prefix": prefix,
                        "vault_root": str(vault_path),
                        "original_id": original_id,
                        "record": record,
                    }

            raise VaultliError(
                f"Document {original_id!r} not found in vault {prefix!r}",
                code="ID_NOT_FOUND",
            )

    raise VaultliError(
        f"No vault matches prefix {prefix!r}",
        code="VAULT_PREFIX_NOT_FOUND",
    )
