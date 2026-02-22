"""Markdown Cleaner Tool — unwrap paragraphs, strip span/div tags, normalize EPUB links."""

import sys
from pathlib import Path
from typing import Annotated

from tooli import Tooli, Argument, Option
from tooli.annotations import Destructive, Idempotent
from tooli.errors import InputError, StateError, ToolRuntimeError, Suggestion

# Import core cleaning logic from the existing standalone script.
sys.path.insert(0, str(Path(__file__).parent.parent))
from markdown_cleaner import clean_markdown  # noqa: E402

app = Tooli(
    name="md-clean",
    description="Clean markdown files: unwrap paragraphs, strip span/div tags, and normalize EPUB index links.",
    version="0.1.0",
)


def _read_file(path: str) -> tuple[str, Path]:
    p = Path(path)
    if not p.exists():
        raise StateError(
            message=f"File not found: {path}",
            code="E3001",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Check that the file path is correct.",
                example="md-clean clean document.md",
            ),
        )
    if not p.is_file():
        raise InputError(message=f"Path is not a file: {path}", code="E1001")
    try:
        return p.read_text(encoding="utf-8"), p
    except UnicodeDecodeError as exc:
        raise InputError(
            message=f"File is not valid UTF-8: {path}",
            code="E1002",
            details={"error": str(exc)},
        ) from exc


@app.command(
    annotations=Destructive | Idempotent,
    task_group="Clean",
    when_to_use=(
        "Clean a markdown file: unwrap hard-wrapped paragraphs, remove unreferenced "
        "<span>/<div> tags, and optionally convert EPUB index links to GFM anchors"
    ),
    supports_dry_run=True,
    examples=[
        {"args": ["document.md"], "description": "Clean and write to document_cleaned.md"},
        {"args": ["document.md", "--in-place"], "description": "Clean in place (overwrites original)"},
        {"args": ["book.md", "--convert-index", "--out-file", "book_clean.md"],
         "description": "Clean EPUB export with index link conversion"},
    ],
    error_codes={
        "E1001": "Path is not a file",
        "E1002": "File is not valid UTF-8",
        "E1003": "--in-place and --out-file cannot both be specified",
        "E3001": "File not found",
        "E4001": "Failed to write output file",
    },
    output_example={
        "output": "document_cleaned.md",
        "chars_before": 4200,
        "chars_after": 3850,
        "in_place": False,
    },
)
def clean(
    file: Annotated[str, Argument(help="Path to the markdown file to clean")],
    out_file: Annotated[str | None, Option(help="Output file path (default: <name>_cleaned.md)")] = None,
    in_place: Annotated[bool, Option(help="Overwrite the input file in place")] = False,
    keep_divs: Annotated[bool, Option(help="Preserve <div> tags instead of stripping them")] = False,
    convert_index: Annotated[bool, Option(help="Convert EPUB index links to GFM heading anchors")] = False,
) -> dict:
    """Clean a markdown file by unwrapping paragraphs and stripping HTML artifacts.

    Paragraph unwrapping joins hard-wrapped lines into single-line paragraphs
    while preserving headings, code blocks, lists, blockquotes, and tables.
    Unreferenced <span> and <div> tags are removed; span IDs that are linked
    elsewhere in the document are kept.
    """
    if in_place and out_file:
        raise InputError(
            message="--in-place and --out-file cannot both be specified.",
            code="E1003",
        )

    content, src = _read_file(file)
    chars_before = len(content)

    cleaned = clean_markdown(content, keep_divs=keep_divs, convert_index=convert_index)
    chars_after = len(cleaned)

    if in_place:
        out_path = src
    elif out_file:
        out_path = Path(out_file)
    else:
        out_path = src.parent / f"{src.stem}_cleaned{src.suffix}"

    try:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(cleaned, encoding="utf-8")
    except OSError as exc:
        raise ToolRuntimeError(
            message=f"Failed to write output file: {out_path} — {exc}",
            code="E4001",
            details={"output": str(out_path)},
        ) from exc

    return {
        "output": str(out_path),
        "chars_before": chars_before,
        "chars_after": chars_after,
        "in_place": in_place,
    }


if __name__ == "__main__":
    app()
