"""CLI for vaultli."""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any

from .core import (
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
    load_index_records,
    make_id,
    refresh_metadata,
    resolve_record,
    scaffold_file,
    search_index,
    set_metadata_field,
    show_record,
    unset_metadata_field,
    validate_vault,
)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="vaultli",
        description="Manage a file-based knowledge vault with YAML frontmatter and JSONL indexing.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    init_parser = subparsers.add_parser("init", help="Initialize a new vault")
    init_parser.add_argument("path", nargs="?", default=".")

    index_parser = subparsers.add_parser("index", help="Build or rebuild INDEX.jsonl")
    index_parser.add_argument("--root", default=".")
    index_parser.add_argument("--full", action="store_true", help="Force a full rebuild")

    search_parser = subparsers.add_parser("search", help="Search the JSONL index")
    search_parser.add_argument("query", nargs="?", default=None)
    search_parser.add_argument("--root", default=".")
    search_parser.add_argument("--jq", dest="jq_filter", default=None, help="jq filter expression")
    search_parser.add_argument("--category", default=None, help="Filter by exact category")
    search_parser.add_argument("--status", default=None, help="Filter by exact status")
    search_parser.add_argument("--domain", default=None, help="Filter by exact domain")
    search_parser.add_argument("--scope", default=None, help="Filter by exact scope")
    search_parser.add_argument("--tag", action="append", default=[], help="Require a tag; repeat for AND filtering")
    search_parser.add_argument("--limit", type=int, default=None, help="Limit the number of returned records")
    search_parser.add_argument("--sort", default=None, help="Sort by id, title, updated, priority, tokens, category, status, or score")
    search_parser.add_argument("--order", choices=["asc", "desc"], default="asc")
    search_parser.add_argument("--explain", action="store_true", help="Include match explanation metadata")
    search_parser.add_argument("--semantic", action="store_true", help="Use experimental token-overlap matching")

    federated_parser = subparsers.add_parser("federated-search", help="Search multiple vault roots")
    federated_parser.add_argument("query", nargs="?", default=None)
    federated_parser.add_argument("--vault", action="append", required=True, help="Vault root to search; repeat for multiple")
    federated_parser.add_argument("--limit", type=int, default=None, help="Limit total returned records")
    federated_parser.add_argument("--per-vault-limit", type=int, default=None, help="Limit records per vault")
    federated_parser.add_argument("--sort", default=None, help="Sort within each vault")
    federated_parser.add_argument("--order", choices=["asc", "desc"], default="asc")
    federated_parser.add_argument("--semantic", action="store_true", help="Use experimental token-overlap matching")
    federated_parser.add_argument("--explain", action="store_true", help="Include per-record match explanations")

    add_parser = subparsers.add_parser("add", help="Add metadata to a file and re-index")
    add_parser.add_argument("file")
    add_parser.add_argument("--root", default=".")

    show_parser = subparsers.add_parser("show", help="Show an indexed record by id")
    show_parser.add_argument("id")
    show_parser.add_argument("--root", default=".")

    resolve_parser = subparsers.add_parser("resolve", help="Resolve an indexed id to files and optional content")
    resolve_parser.add_argument("id")
    resolve_parser.add_argument("--root", default=".")
    resolve_parser.add_argument("--body", action="store_true", help="Include markdown body content")
    resolve_parser.add_argument("--source", action="store_true", help="Include source asset content when present")

    cat_parser = subparsers.add_parser("cat", help="Print indexed markdown body or sidecar source content")
    cat_parser.add_argument("id")
    cat_parser.add_argument("--root", default=".")
    cat_parser.add_argument("--source", action="store_true", help="Print source asset content instead of markdown body")

    context_parser = subparsers.add_parser("context", help="Assemble a deterministic context bundle")
    context_parser.add_argument("query", nargs="?", default=None)
    context_parser.add_argument("--root", default=".")
    context_parser.add_argument("--id", action="append", dest="ids", default=[], help="Seed with an id; repeat to include multiple")
    context_parser.add_argument("--token-budget", type=int, default=None)
    context_parser.add_argument("--related", action="store_true", help="Include related references")
    context_parser.add_argument("--no-dependencies", action="store_true", help="Do not include depends_on references")
    context_parser.add_argument("--limit", type=int, default=None, help="Limit search seeds when using a query")

    git_parser = subparsers.add_parser("git-info", help="Return git state for a vault or indexed item")
    git_parser.add_argument("target", nargs="?", default=None)
    git_parser.add_argument("--root", default=".")

    validate_parser = subparsers.add_parser("validate", help="Validate vault integrity")
    validate_parser.add_argument("--root", default=".")

    scaffold_parser = subparsers.add_parser("scaffold", help="Create a frontmatter or sidecar stub")
    scaffold_parser.add_argument("file")
    scaffold_parser.add_argument("--root", default=".")

    ingest_parser = subparsers.add_parser("ingest", help="Bulk scaffold missing metadata under a file or directory")
    ingest_parser.add_argument("path")
    ingest_parser.add_argument("--root", default=".")
    ingest_parser.add_argument("--index", action="store_true", help="Rebuild INDEX.jsonl after scaffolding")
    ingest_parser.add_argument("--dry-run", action="store_true", help="Preview writes without changing files")
    ingest_parser.add_argument("--include", action="append", default=[], help="Glob of relative paths to include; repeatable")
    ingest_parser.add_argument("--exclude", action="append", default=[], help="Glob of relative paths to exclude; repeatable")

    set_parser = subparsers.add_parser("set", help="Set one frontmatter field")
    set_parser.add_argument("target")
    set_parser.add_argument("field")
    set_parser.add_argument("value")
    set_parser.add_argument("--root", default=".")
    set_parser.add_argument("--index", action="store_true")

    unset_parser = subparsers.add_parser("unset", help="Remove one frontmatter field")
    unset_parser.add_argument("target")
    unset_parser.add_argument("field")
    unset_parser.add_argument("--root", default=".")
    unset_parser.add_argument("--index", action="store_true")

    refresh_parser = subparsers.add_parser("refresh", help="Refresh inferred metadata fields")
    refresh_parser.add_argument("target")
    refresh_parser.add_argument("--root", default=".")
    refresh_parser.add_argument("--field", action="append", default=[])
    refresh_parser.add_argument("--index", action="store_true")

    root_parser = subparsers.add_parser("root", help="Locate the nearest vault root")
    root_parser.add_argument("path", nargs="?", default=".")

    make_id_parser = subparsers.add_parser("make-id", help="Derive a vault id from a file path")
    make_id_parser.add_argument("file")
    make_id_parser.add_argument("--root", default=".")

    infer_parser = subparsers.add_parser("infer", help="Preview inferred scaffold metadata")
    infer_parser.add_argument("file")
    infer_parser.add_argument("--root", default=".")

    dump_index_parser = subparsers.add_parser("dump-index", help="Dump all current index records")
    dump_index_parser.add_argument("--root", default=".")

    return parser


def _print_json(payload: dict[str, Any], *, stderr: bool = False) -> None:
    print(json.dumps(payload, indent=2, sort_keys=False), file=sys.stderr if stderr else sys.stdout)


def _print_error(exc: VaultliError, as_json: bool) -> None:
    if as_json:
        _print_json({"ok": False, "error": exc.to_dict()}, stderr=True)
        return
    print(f"error [{exc.code}]: {exc.message}", file=sys.stderr)


def _print_search_results(records: list[dict[str, Any]], as_json: bool) -> None:
    if as_json:
        _print_json({"ok": True, "result": {"total": len(records), "results": records}})
        return
    if not records:
        print("No matches found.")
        return
    for record in records:
        print(f"{record.get('id', '-')}\t{record.get('title', '-')}\t{record.get('description', '-')}")


def _print_index_result(result: dict[str, Any], as_json: bool) -> None:
    if as_json:
        _print_json({"ok": True, "result": result})
        return
    print(
        f"indexed={result['indexed']} updated={result['updated']} "
        f"pruned={result['pruned']} skipped={result['skipped']}"
    )
    for warning in result.get("warnings", []):
        location = warning.get("file", "-")
        print(f"warning [{warning['code']}] {location}: {warning['message']}")


def _print_record(record: dict[str, Any], as_json: bool) -> None:
    if as_json:
        _print_json({"ok": True, "result": record})
        return
    for key, value in record.items():
        if isinstance(value, list):
            rendered = ", ".join(str(item) for item in value)
        else:
            rendered = value
        print(f"{key}: {rendered}")


def _print_validation(result: dict[str, Any], as_json: bool) -> None:
    if as_json:
        _print_json({"ok": result["valid"], "result": result})
        return
    if result["valid"]:
        print("Vault is valid.")
        return
    print(f"Validation failed with {result['issue_count']} issue(s).")
    for issue in result["issues"]:
        location = issue.get("file", "-")
        print(f"{issue['level']} [{issue['code']}] {location}: {issue['message']}")


def _print_generic(result: Any, as_json: bool) -> None:
    if as_json:
        _print_json({"ok": True, "result": result})
        return
    if isinstance(result, dict):
        for key, value in result.items():
            print(f"{key}: {value}")
        return
    print(result)


def main(argv: list[str] | None = None) -> int:
    raw_argv = list(sys.argv[1:] if argv is None else argv)
    as_json = False
    filtered_argv: list[str] = []
    for token in raw_argv:
        if token == "--json":
            as_json = True
            continue
        filtered_argv.append(token)

    parser = _build_parser()
    args = parser.parse_args(filtered_argv)

    try:
        if args.command == "init":
            _print_generic(init_vault(args.path), as_json)
            return 0

        if args.command == "index":
            result = build_index(root=args.root, full=args.full).to_dict()
            _print_index_result(result, as_json)
            return 0

        if args.command == "search":
            _print_search_results(
                search_index(
                    args.query,
                    root=args.root,
                    jq_filter=args.jq_filter,
                    category=args.category,
                    status=args.status,
                    domain=args.domain,
                    scope=args.scope,
                    tags=args.tag,
                    limit=args.limit,
                    sort=args.sort,
                    order=args.order,
                    explain=args.explain,
                    semantic=args.semantic,
                ),
                as_json,
            )
            return 0

        if args.command == "federated-search":
            _print_generic(
                federated_search(
                    args.vault,
                    args.query,
                    limit=args.limit,
                    per_vault_limit=args.per_vault_limit,
                    semantic=args.semantic,
                    explain=args.explain,
                    sort=args.sort,
                    order=args.order,
                ),
                as_json,
            )
            return 0

        if args.command == "add":
            _print_generic(add_file(args.file, root=args.root), as_json)
            return 0

        if args.command == "show":
            _print_record(show_record(args.id, root=args.root), as_json)
            return 0

        if args.command == "resolve":
            _print_generic(
                resolve_record(args.id, root=args.root, include_body=args.body, include_source=args.source),
                as_json,
            )
            return 0

        if args.command == "cat":
            result = cat_record(args.id, root=args.root, source=args.source)
            if as_json:
                _print_json({"ok": True, "result": result})
            else:
                print(result["content"], end="" if str(result["content"]).endswith("\n") else "\n")
            return 0

        if args.command == "context":
            _print_generic(
                assemble_context(
                    args.query,
                    root=args.root,
                    ids=args.ids or None,
                    token_budget=args.token_budget,
                    include_related=args.related,
                    include_dependencies=not args.no_dependencies,
                    limit=args.limit,
                ),
                as_json,
            )
            return 0

        if args.command == "git-info":
            _print_generic(git_info(args.target, root=args.root), as_json)
            return 0

        if args.command == "validate":
            result = validate_vault(root=args.root)
            _print_validation(result, as_json)
            return 0 if result["valid"] else 1

        if args.command == "scaffold":
            _print_generic(scaffold_file(args.file, root=args.root), as_json)
            return 0

        if args.command == "ingest":
            _print_generic(
                ingest_path(
                    args.path,
                    root=args.root,
                    index=args.index,
                    dry_run=args.dry_run,
                    include=args.include,
                    exclude=args.exclude,
                ),
                as_json,
            )
            return 0

        if args.command == "set":
            _print_generic(set_metadata_field(args.target, args.field, args.value, root=args.root, index=args.index), as_json)
            return 0

        if args.command == "unset":
            _print_generic(unset_metadata_field(args.target, args.field, root=args.root, index=args.index), as_json)
            return 0

        if args.command == "refresh":
            _print_generic(refresh_metadata(args.target, root=args.root, fields=args.field, index=args.index), as_json)
            return 0

        if args.command == "root":
            _print_generic({"root": str(find_root(args.path))}, as_json)
            return 0

        if args.command == "make-id":
            _print_generic({"id": make_id(args.file, args.root)}, as_json)
            return 0

        if args.command == "infer":
            _print_generic({"metadata": infer_frontmatter(args.file, args.root)}, as_json)
            return 0

        if args.command == "dump-index":
            _print_generic({"records": load_index_records(args.root)}, as_json)
            return 0
    except VaultliError as exc:
        _print_error(exc, as_json)
        return 1

    parser.error(f"Unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
