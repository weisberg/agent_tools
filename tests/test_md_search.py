"""Tests for tools/md_search.py"""

import pytest
from pathlib import Path

# Import the app and private helpers directly for unit testing
import sys
sys.path.insert(0, str(Path(__file__).parent.parent))

from tools.md_search import app, _extract_headers, _extract_links, _extract_code_blocks


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

COMPLEX_MD = """\
# Title

Some introductory paragraph that spans
multiple lines in the original source.

## Section One

Content here.

### Sub-section 1.1

More content.

## Section Two

Final section.
"""

LINKS_MD = """\
# Links Document

Inline link: [GitHub](https://github.com)
Internal link: [Section](#section-one)
Reference link: [Python][python-ref]
Autolink: <https://example.com>

[python-ref]: https://python.org

Some text with no links.
"""

CODE_MD = """\
# Code Blocks

Plain text block.

```python
def hello():
    return "world"
```

```bash
echo hello
```

```
no language
```
"""

MIXED_MD = COMPLEX_MD + "\n" + LINKS_MD + "\n" + CODE_MD


@pytest.fixture
def md_file(tmp_path):
    """Return a factory that writes content to a temp .md file."""
    def _write(content: str, name: str = "test.md") -> str:
        p = tmp_path / name
        p.write_text(content, encoding="utf-8")
        return str(p)
    return _write


# ---------------------------------------------------------------------------
# _extract_headers unit tests
# ---------------------------------------------------------------------------

class TestExtractHeaders:
    def test_all_levels(self):
        result = _extract_headers(COMPLEX_MD, level=None)
        assert len(result) == 4
        assert result[0] == {"level": 1, "text": "Title", "line": 1}
        assert result[1] == {"level": 2, "text": "Section One", "line": 6}
        assert result[2] == {"level": 3, "text": "Sub-section 1.1", "line": 10}
        assert result[3] == {"level": 2, "text": "Section Two", "line": 14}

    def test_filter_level_2(self):
        result = _extract_headers(COMPLEX_MD, level=2)
        assert len(result) == 2
        assert all(h["level"] == 2 for h in result)
        assert result[0]["text"] == "Section One"
        assert result[1]["text"] == "Section Two"

    def test_filter_level_1(self):
        result = _extract_headers(COMPLEX_MD, level=1)
        assert len(result) == 1
        assert result[0]["text"] == "Title"

    def test_no_headers(self):
        result = _extract_headers("Just a paragraph.\n", level=None)
        assert result == []

    def test_level_filter_no_match(self):
        result = _extract_headers(COMPLEX_MD, level=6)
        assert result == []

    def test_line_numbers_are_1_indexed(self):
        content = "# First\n\n## Second\n"
        result = _extract_headers(content, level=None)
        assert result[0]["line"] == 1
        assert result[1]["line"] == 3


# ---------------------------------------------------------------------------
# _extract_links unit tests
# ---------------------------------------------------------------------------

class TestExtractLinks:
    def test_inline_links(self):
        result = _extract_links(LINKS_MD, external_only=False)
        inline = [r for r in result if r["type"] == "inline"]
        assert any(r["url"] == "https://github.com" and r["text"] == "GitHub" for r in inline)
        assert any(r["url"] == "#section-one" for r in inline)

    def test_autolinks(self):
        result = _extract_links(LINKS_MD, external_only=False)
        auto = [r for r in result if r["type"] == "autolink"]
        assert any(r["url"] == "https://example.com" for r in auto)

    def test_reference_links(self):
        result = _extract_links(LINKS_MD, external_only=False)
        refs = [r for r in result if r["type"] == "reference"]
        assert any(r["text"] == "Python" and r["url"] == "https://python.org" for r in refs)

    def test_external_only_filters_internal(self):
        result = _extract_links(LINKS_MD, external_only=True)
        assert all(r["url"].startswith("http") for r in result)
        assert not any("#section-one" in r["url"] for r in result)

    def test_no_links(self):
        result = _extract_links("No links here.\n", external_only=False)
        assert result == []


# ---------------------------------------------------------------------------
# _extract_code_blocks unit tests
# ---------------------------------------------------------------------------

class TestExtractCodeBlocks:
    def test_all_blocks(self):
        result = _extract_code_blocks(CODE_MD, language=None)
        assert len(result) == 3

    def test_python_filter(self):
        result = _extract_code_blocks(CODE_MD, language="python")
        assert len(result) == 1
        assert result[0]["language"] == "python"
        assert "def hello" in result[0]["content"]

    def test_bash_filter(self):
        result = _extract_code_blocks(CODE_MD, language="bash")
        assert len(result) == 1
        assert "echo hello" in result[0]["content"]

    def test_no_language_block(self):
        result = _extract_code_blocks(CODE_MD, language=None)
        no_lang = [b for b in result if b["language"] == ""]
        assert len(no_lang) == 1
        assert "no language" in no_lang[0]["content"]

    def test_nonexistent_language(self):
        result = _extract_code_blocks(CODE_MD, language="rust")
        assert result == []

    def test_line_numbers(self):
        result = _extract_code_blocks(CODE_MD, language="python")
        block = result[0]
        assert block["start_line"] < block["end_line"]


# ---------------------------------------------------------------------------
# Tooli app integration tests (Python API)
# ---------------------------------------------------------------------------

class TestHeadersCommand:
    def test_returns_list(self, md_file):
        f = md_file(COMPLEX_MD)
        result = app.call("headers", file=f)
        assert result.ok
        assert isinstance(result.result, list)
        assert len(result.result) == 4

    def test_level_filter(self, md_file):
        f = md_file(COMPLEX_MD)
        result = app.call("headers", file=f, level=2)
        assert result.ok
        assert all(h["level"] == 2 for h in result.result)

    def test_invalid_level(self, md_file):
        f = md_file(COMPLEX_MD)
        result = app.call("headers", file=f, level=0)
        assert not result.ok
        assert result.error.code == "E1003"

    def test_missing_file(self):
        result = app.call("headers", file="/no/such/file.md")
        assert not result.ok
        assert result.error.code == "E3001"

    def test_no_headers_raises_state_error(self, md_file):
        f = md_file("Just plain text, no headers.\n")
        result = app.call("headers", file=f)
        assert not result.ok
        assert result.error.code == "E3002"


class TestLinksCommand:
    def test_returns_links(self, md_file):
        f = md_file(LINKS_MD)
        result = app.call("links", file=f)
        assert result.ok
        assert len(result.result) >= 3

    def test_external_only(self, md_file):
        f = md_file(LINKS_MD)
        result = app.call("links", file=f, external_only=True)
        assert result.ok
        assert all(r["url"].startswith("http") for r in result.result)

    def test_no_links(self, md_file):
        f = md_file("No links here.\n")
        result = app.call("links", file=f)
        assert not result.ok
        assert result.error.code == "E3002"

    def test_missing_file(self):
        result = app.call("links", file="/nonexistent.md")
        assert not result.ok
        assert result.error.code == "E3001"


class TestCodeBlocksCommand:
    def test_all_blocks(self, md_file):
        f = md_file(CODE_MD)
        result = app.call("code-blocks", file=f)
        assert result.ok
        assert len(result.result) == 3

    def test_language_filter(self, md_file):
        f = md_file(CODE_MD)
        result = app.call("code-blocks", file=f, language="python")
        assert result.ok
        assert len(result.result) == 1

    def test_no_match(self, md_file):
        f = md_file(CODE_MD)
        result = app.call("code-blocks", file=f, language="ruby")
        assert not result.ok
        assert result.error.code == "E3002"

    def test_missing_file(self):
        result = app.call("code-blocks", file="/nonexistent.md")
        assert not result.ok
        assert result.error.code == "E3001"


# ---------------------------------------------------------------------------
# Agent interaction scenario tests
# ---------------------------------------------------------------------------

class TestAgentScenarios:
    def test_header_extraction_multi_level(self, md_file):
        """Agent requests all headers from a complex, multi-level markdown document."""
        f = md_file(COMPLEX_MD)
        result = app.call("headers", file=f)
        assert result.ok
        levels = [h["level"] for h in result.result]
        assert 1 in levels
        assert 2 in levels
        assert 3 in levels
        # All have text and line number
        for h in result.result:
            assert isinstance(h["text"], str) and h["text"]
            assert isinstance(h["line"], int) and h["line"] > 0

    def test_link_discovery_various_formats(self, md_file):
        """Agent asks to extract all external links — various link formats."""
        f = md_file(LINKS_MD)
        result = app.call("links", file=f, external_only=True)
        assert result.ok
        urls = {r["url"] for r in result.result}
        assert "https://github.com" in urls
        assert "https://python.org" in urls
        assert "https://example.com" in urls

    def test_code_block_language_filter(self, md_file):
        """Agent searches for all python code blocks."""
        f = md_file(CODE_MD)
        result = app.call("code-blocks", file=f, language="python")
        assert result.ok
        assert all(b["language"] == "python" for b in result.result)
        # Content is present
        assert all(b["content"].strip() for b in result.result)

    def test_malformed_input(self, md_file):
        """Agent passes invalid level parameter — tool returns structured error."""
        f = md_file(COMPLEX_MD)
        result = app.call("headers", file=f, level=99)
        assert not result.ok
        assert result.error.code == "E1003"
        assert result.error.message  # actionable message present
