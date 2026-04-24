"""CLI for vaultli."""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any

from .core import (
    VaultliError,
    add_file,
    build_index,
    find_root,
    infer_frontmatter,
    ingest_path,
    init_vault,
    load_index_records,
    make_id,
    scaffold_file,
    search_index,
    show_record,
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

    add_parser = subparsers.add_parser("add", help="Add metadata to a file and re-index")
    add_parser.add_argument("file")
    add_parser.add_argument("--root", default=".")

    show_parser = subparsers.add_parser("show", help="Show an indexed record by id")
    show_parser.add_argument("id")
    show_parser.add_argument("--root", default=".")

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

        if args.command == "validate":
            result = validate_vault(root=args.root)
            _print_validation(result, as_json)
            return 0 if result["valid"] else 1

        if args.command == "scaffold":
            _print_generic(scaffold_file(args.file, root=args.root), as_json)
            return 0

        if args.command == "ingest":
            _print_generic(ingest_path(args.path, root=args.root, index=args.index, dry_run=args.dry_run), as_json)
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
