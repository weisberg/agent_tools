"""Report generation for XLI companion results."""

from __future__ import annotations

import html
import importlib.resources
from pathlib import Path

from jinja2 import Environment, BaseLoader

from xli_companion.models import CompanionResult


def _get_template_string() -> str:
    """Load the Jinja2 template from package resources."""
    templates = importlib.resources.files("xli_companion") / "templates"
    template_path = templates / "report.md.j2"
    return template_path.read_text(encoding="utf-8")


def _get_environment() -> Environment:
    """Create a Jinja2 environment with the report template loaded."""
    return Environment(loader=BaseLoader(), autoescape=False)


def render_markdown(result: CompanionResult) -> str:
    """Render a CompanionResult as a Markdown report.

    Parameters
    ----------
    result:
        The companion result to render.

    Returns
    -------
    str
        The rendered Markdown string.
    """
    env = _get_environment()
    template_source = _get_template_string()
    template = env.from_string(template_source)
    return template.render(result=result)


def render_html(result: CompanionResult) -> str:
    """Render a CompanionResult as a basic HTML report.

    Wraps the Markdown output in a minimal HTML document using ``<pre>``
    tags for simplicity.

    Parameters
    ----------
    result:
        The companion result to render.

    Returns
    -------
    str
        The rendered HTML string.
    """
    md = render_markdown(result)
    escaped = html.escape(md)
    return (
        "<!DOCTYPE html>\n"
        "<html>\n"
        "<head><meta charset=\"utf-8\"><title>XLI Companion Report</title></head>\n"
        "<body>\n"
        f"<pre>{escaped}</pre>\n"
        "</body>\n"
        "</html>"
    )


def write_report(result: CompanionResult, path: Path, fmt: str = "markdown") -> None:
    """Write a report to *path* in the given format.

    Parameters
    ----------
    result:
        The companion result to render.
    path:
        Destination file path.
    fmt:
        ``"markdown"`` (default) or ``"html"``.
    """
    if fmt == "html":
        content = render_html(result)
    else:
        content = render_markdown(result)

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
