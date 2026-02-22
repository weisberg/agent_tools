"""Tests for tools/md_clean.py"""

import pytest
from pathlib import Path

import sys
sys.path.insert(0, str(Path(__file__).parent.parent))

from tools.md_clean import app
from markdown_cleaner import clean_markdown


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

HARD_WRAPPED = """\
# Introduction

This is a paragraph that has been
hard-wrapped at 80 characters so
each line is artificially short.

## A List

- Item one
- Item two
- Item three

## A Code Block

```python
x = 1
```

Another hard-wrapped
paragraph here.
"""

SPAN_CONTENT = """\
# Section <span id="sec-1">One</span>

Some text with an [anchor](#sec-1) to the heading.

An <span id="unreferenced">unreferenced</span> span here.
"""

DIV_CONTENT = """\
# Title

<div class="box">
Content inside a div.
</div>

Plain paragraph.
"""


@pytest.fixture
def md_file(tmp_path):
    def _write(content: str, name: str = "input.md") -> str:
        p = tmp_path / name
        p.write_text(content, encoding="utf-8")
        return str(p)
    return _write


# ---------------------------------------------------------------------------
# clean_markdown unit tests (core logic)
# ---------------------------------------------------------------------------

class TestCleanMarkdown:
    def test_unwraps_paragraphs(self):
        result = clean_markdown(HARD_WRAPPED)
        # The intro paragraph should be joined onto a single line
        assert "hard-wrapped at 80 characters so each line is artificially short." in result
        # That joined text should appear as a single line (no newline mid-sentence)
        for line in result.splitlines():
            if "hard-wrapped" in line:
                assert "artificially short." in line
                break

    def test_preserves_lists(self):
        result = clean_markdown(HARD_WRAPPED)
        assert "- Item one" in result
        assert "- Item two" in result
        assert "- Item three" in result

    def test_preserves_code_blocks(self):
        result = clean_markdown(HARD_WRAPPED)
        assert "```python" in result
        assert "x = 1" in result

    def test_preserves_headings(self):
        result = clean_markdown(HARD_WRAPPED)
        assert "# Introduction" in result
        assert "## A List" in result

    def test_removes_unreferenced_spans(self):
        result = clean_markdown(SPAN_CONTENT)
        # Referenced span is kept
        assert 'id="sec-1"' in result
        # Unreferenced span tags are stripped (content kept, tags gone)
        assert 'id="unreferenced"' not in result
        assert "unreferenced" in result  # content preserved

    def test_removes_divs_by_default(self):
        result = clean_markdown(DIV_CONTENT)
        assert "<div" not in result
        assert "</div>" not in result
        assert "Content inside a div" in result

    def test_keep_divs_flag(self):
        result = clean_markdown(DIV_CONTENT, keep_divs=True)
        assert "<div" in result
        assert "</div>" in result

    def test_trailing_newline(self):
        result = clean_markdown("Hello world.\n")
        assert result.endswith("\n")

    def test_empty_input(self):
        result = clean_markdown("\n\n\n")
        assert result.strip() == ""


# ---------------------------------------------------------------------------
# Tooli app integration tests (Python API)
# ---------------------------------------------------------------------------

class TestCleanCommand:
    def test_writes_cleaned_file(self, md_file, tmp_path):
        f = md_file(HARD_WRAPPED)
        out = str(tmp_path / "clean.md")
        result = app.call("clean", file=f, out_file=out)
        assert result.ok
        assert Path(out).exists()
        content = Path(out).read_text()
        assert "hard-wrapped at 80 characters" in content

    def test_default_output_name(self, md_file):
        f = md_file(HARD_WRAPPED, "myfile.md")
        result = app.call("clean", file=f)
        assert result.ok
        assert result.result["output"].endswith("myfile_cleaned.md")
        assert Path(result.result["output"]).exists()

    def test_in_place(self, md_file):
        f = md_file(HARD_WRAPPED)
        result = app.call("clean", file=f, in_place=True)
        assert result.ok
        assert result.result["in_place"] is True
        assert result.result["output"] == f

    def test_chars_reported(self, md_file):
        f = md_file(HARD_WRAPPED)
        result = app.call("clean", file=f)
        assert result.ok
        assert result.result["chars_before"] > 0
        assert result.result["chars_after"] > 0

    def test_in_place_and_output_conflict(self, md_file):
        f = md_file(HARD_WRAPPED)
        result = app.call("clean", file=f, in_place=True, out_file="/tmp/x.md")
        assert not result.ok
        assert result.error.code == "E1003"

    def test_missing_file(self):
        result = app.call("clean", file="/no/such/file.md")
        assert not result.ok
        assert result.error.code == "E3001"

    def test_removes_divs(self, md_file, tmp_path):
        f = md_file(DIV_CONTENT)
        out = str(tmp_path / "clean.md")
        result = app.call("clean", file=f, out_file=out)
        assert result.ok
        cleaned = Path(out).read_text()
        assert "<div" not in cleaned

    def test_keep_divs(self, md_file, tmp_path):
        f = md_file(DIV_CONTENT)
        out = str(tmp_path / "clean.md")
        result = app.call("clean", file=f, out_file=out, keep_divs=True)
        assert result.ok
        cleaned = Path(out).read_text()
        assert "<div" in cleaned

    def test_removes_unreferenced_spans(self, md_file, tmp_path):
        f = md_file(SPAN_CONTENT)
        out = str(tmp_path / "clean.md")
        result = app.call("clean", file=f, out_file=out)
        assert result.ok
        cleaned = Path(out).read_text()
        assert 'id="unreferenced"' not in cleaned
        assert "unreferenced" in cleaned  # content kept


# ---------------------------------------------------------------------------
# Agent interaction scenario tests
# ---------------------------------------------------------------------------

class TestAgentScenarios:
    def test_epub_cleanup(self, md_file, tmp_path):
        """Agent processes markdown from EPUB with span tags and hard-wrapped lines."""
        epub_md = """\
# Chapter <span id="ch1">One</span>

This paragraph was exported from
an EPUB with hard line breaks every
few words.

See also: [Chapter One](#ch1)

<span id="orphan">Orphan span</span> here.
"""
        f = md_file(epub_md)
        out = str(tmp_path / "clean.md")
        result = app.call("clean", file=f, out_file=out)
        assert result.ok
        cleaned = Path(out).read_text()
        # Paragraph unwrapped
        assert "EPUB with hard line breaks every few words." in cleaned
        # Referenced span kept
        assert 'id="ch1"' in cleaned
        # Orphan span stripped, content kept
        assert 'id="orphan"' not in cleaned
        assert "Orphan span" in cleaned

    def test_paragraph_unwrapping_preserves_structure(self, md_file, tmp_path):
        """Agent unwraps 80-char hard-wraps; lists and blockquotes remain intact."""
        f = md_file(HARD_WRAPPED)
        out = str(tmp_path / "clean.md")
        result = app.call("clean", file=f, out_file=out)
        assert result.ok
        cleaned = Path(out).read_text()
        assert "- Item one" in cleaned
        assert "- Item two" in cleaned
        assert "```python" in cleaned

    def test_input_validation_structured_error(self):
        """Agent passes a non-existent file — receives structured, parseable error."""
        result = app.call("clean", file="/does/not/exist.md")
        assert not result.ok
        assert result.error.code == "E3001"
        assert result.error.message
        assert result.error.suggestion is not None
        assert result.error.suggestion["fix"]
