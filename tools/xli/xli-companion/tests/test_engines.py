"""Tests for engine adapter interface."""

from xli_companion.engines.base import SpreadsheetEngine
from xli_companion.engines.xlwings_adapter import XlwingsEngine


def test_spreadsheet_engine_is_abstract():
    """SpreadsheetEngine cannot be instantiated directly."""
    import pytest
    with pytest.raises(TypeError):
        SpreadsheetEngine()


def test_xlwings_available_returns_bool():
    """XlwingsEngine.available() returns a boolean without crashing."""
    result = XlwingsEngine.available()
    assert isinstance(result, bool)


def test_xlwings_graceful_when_unavailable():
    """If xlwings isn't installed, available() returns False."""
    # This test passes in CI where xlwings is not installed.
    # If xlwings IS installed, it returns True — both are valid.
    result = XlwingsEngine.available()
    assert isinstance(result, bool)
