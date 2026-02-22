"""Markdown Search Tool — search markdown content for headers, links, and code blocks."""

import re
import sys
from pathlib import Path
from typing import Annotated

from tooli import Tooli, Argument, Option
from tooli.annotations import ReadOnly
from tooli.errors import InputError, StateError, Suggestion

app = Tooli(
    name="md-search",
    description="Search markdown content for structured elements like headers, links, and code blocks.",
    version="0.1.0",
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _read_file(path: str) -> tuple[str, Path]:
    p = Path(path)
    if not p.exists():
        raise StateError(
            message=f"File not found: {path}",
            code="E3001",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Check that the file path is correct and the file exists.",
                example="md-search headers README.md",
            ),
        )
    if not p.is_file():
        raise InputError(
            message=f"Path is not a file: {path}",
            code="E1001",
        )
    try:
        return p.read_text(encoding="utf-8"), p
    except UnicodeDecodeError as exc:
        raise InputError(
            message=f"File is not valid UTF-8: {path}",
            code="E1002",
            details={"error": str(exc)},
        ) from exc


def _extract_headers(content: str, level: int | None) -> list[dict]:
    results = []
    for i, line in enumerate(content.splitlines(), start=1):
        m = re.match(r'^(#{1,6})\s+(.+)$', line)
        if m:
            lvl = len(m.group(1))
            if level is None or lvl == level:
                results.append({
                    "level": lvl,
                    "text": m.group(2).strip(),
                    "line": i,
                })
    return results


def _extract_links(content: str, external_only: bool) -> list[dict]:
    results = []
    lines = content.splitlines()

    # Collect reference definitions: [label]: url "optional title"
    ref_defs: dict[str, str] = {}
    for line in lines:
        m = re.match(r'^\s*\[([^\]]+)\]:\s*(\S+)', line)
        if m:
            ref_defs[m.group(1).lower()] = m.group(2)

    for i, line in enumerate(lines, start=1):
        # Inline links: [text](url)
        for m in re.finditer(r'\[([^\]]*)\]\(([^)]+)\)', line):
            url = m.group(2).split()[0].strip('<>')  # strip optional title
            if external_only and not url.startswith(('http://', 'https://')):
                continue
            results.append({"text": m.group(1), "url": url, "line": i, "type": "inline"})

        # Reference-style links: [text][label] or [text][]
        for m in re.finditer(r'\[([^\]]+)\]\[([^\]]*)\]', line):
            label = (m.group(2) or m.group(1)).lower()
            url = ref_defs.get(label, "")
            if external_only and not url.startswith(('http://', 'https://')):
                continue
            results.append({"text": m.group(1), "url": url, "line": i, "type": "reference"})

        # Autolinks: <https://example.com>
        for m in re.finditer(r'<(https?://[^>]+)>', line):
            url = m.group(1)
            results.append({"text": url, "url": url, "line": i, "type": "autolink"})

    return results


def _extract_code_blocks(content: str, language: str | None) -> list[dict]:
    results = []
    lines = content.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        # Fenced code block opening: ``` or ~~~
        m = re.match(r'^(`{3,}|~{3,})(\w*).*$', line)
        if m:
            fence = m.group(1)
            lang = m.group(2).lower() or ""
            start_line = i + 1
            i += 1
            code_lines = []
            while i < len(lines):
                if lines[i].startswith(fence):
                    break
                code_lines.append(lines[i])
                i += 1
            end_line = i + 1
            if language is None or lang == language.lower():
                results.append({
                    "language": lang,
                    "content": "\n".join(code_lines),
                    "start_line": start_line,
                    "end_line": end_line,
                })
        i += 1
    return results


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

@app.command(
    annotations=ReadOnly,
    task_group="Query",
    when_to_use="Extract all ATX headings from a markdown file",
    examples=[
        {"args": ["README.md"], "description": "All headers in README"},
        {"args": ["README.md", "--level", "2"], "description": "Only H2 headers"},
    ],
    error_codes={"E1001": "Path is not a file", "E1002": "File is not valid UTF-8", "E3001": "File not found"},
    output_example=[{"level": 2, "text": "Installation", "line": 12}],
    paginated=True,
)
def headers(
    file: Annotated[str, Argument(help="Path to the markdown file")],
    level: Annotated[int | None, Option(help="Filter to only this heading level (1-6)")] = None,
) -> list[dict]:
    """Extract all ATX headings from a markdown file.

    Returns each heading with its nesting level (1-6), text, and line number.
    """
    if level is not None and not (1 <= level <= 6):
        raise InputError(
            message=f"Heading level must be between 1 and 6, got {level}",
            code="E1003",
            field="level",
        )
    content, _ = _read_file(file)
    results = _extract_headers(content, level)
    if not results:
        raise StateError(
            message=f"No headers found in {file}" + (f" at level {level}" if level else ""),
            code="E3002",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Try without --level to see all headers.",
                example=f"md-search headers {file}",
            ),
        )
    return results


@app.command(
    annotations=ReadOnly,
    task_group="Query",
    when_to_use="Discover all links (inline, reference, autolink) in a markdown file",
    examples=[
        {"args": ["README.md"], "description": "All links in README"},
        {"args": ["README.md", "--external-only"], "description": "Only external HTTP links"},
    ],
    error_codes={"E3001": "File not found", "E3002": "No links found"},
    output_example=[{"text": "GitHub", "url": "https://github.com", "line": 5, "type": "inline"}],
    paginated=True,
)
def links(
    file: Annotated[str, Argument(help="Path to the markdown file")],
    external_only: Annotated[bool, Option(help="Return only external HTTP/HTTPS links")] = False,
) -> list[dict]:
    """Extract all links from a markdown file.

    Supports inline links `[text](url)`, reference-style `[text][ref]`, and autolinks `<url>`.
    """
    content, _ = _read_file(file)
    results = _extract_links(content, external_only)
    if not results:
        raise StateError(
            message=f"No{'external ' if external_only else ' '}links found in {file}",
            code="E3002",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Try without --external-only to see all links.",
                example=f"md-search links {file}",
            ),
        )
    return results


@app.command(
    name="code-blocks",
    annotations=ReadOnly,
    task_group="Query",
    when_to_use="Extract fenced code blocks from a markdown file, optionally filtered by language",
    examples=[
        {"args": ["README.md"], "description": "All code blocks"},
        {"args": ["README.md", "--language", "python"], "description": "Only Python blocks"},
    ],
    error_codes={"E3001": "File not found", "E3002": "No matching code blocks found"},
    output_example=[{"language": "python", "content": "print('hello')", "start_line": 15, "end_line": 17}],
    paginated=True,
)
def code_blocks(
    file: Annotated[str, Argument(help="Path to the markdown file")],
    language: Annotated[str | None, Option(help="Filter to only this language (e.g. python, bash)")] = None,
) -> list[dict]:
    """Extract fenced code blocks from a markdown file.

    Returns each block's language tag, content, and line range.
    """
    content, _ = _read_file(file)
    results = _extract_code_blocks(content, language)
    if not results:
        raise StateError(
            message=f"No{f' {language}' if language else ''} code blocks found in {file}",
            code="E3002",
            suggestion=Suggestion(
                action="retry_with_modified_input",
                fix="Try without --language to see all code blocks.",
                example=f"md-search code-blocks {file}",
            ),
        )
    return results


if __name__ == "__main__":
    app()
