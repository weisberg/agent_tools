"""xlwings engine adapter — requires xlwings and Excel to be installed."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from xli_companion.engines.base import SpreadsheetEngine


class XlwingsEngine(SpreadsheetEngine):
    """Engine adapter using xlwings to automate real Excel."""

    def __init__(self) -> None:
        self._app: Any = None
        self._book: Any = None

    @classmethod
    def available(cls) -> bool:
        try:
            import xlwings  # noqa: F401
            return True
        except ImportError:
            return False

    def open(self, path: Path) -> None:
        import xlwings as xw
        self._app = xw.App(visible=False)
        self._book = self._app.books.open(str(path))

    def read_cell(self, sheet: str, address: str) -> Any:
        if self._book is None:
            raise RuntimeError("No workbook is open")
        return self._book.sheets[sheet].range(address).value

    def recalculate(self) -> None:
        if self._app is None:
            raise RuntimeError("No workbook is open")
        self._app.calculate()

    def close(self) -> None:
        if self._book is not None:
            self._book.close()
            self._book = None
        if self._app is not None:
            self._app.quit()
            self._app = None
