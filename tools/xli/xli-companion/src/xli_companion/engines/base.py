"""Abstract base class for spreadsheet engine adapters."""

from __future__ import annotations

from abc import ABC, abstractmethod
from pathlib import Path
from typing import Any


class SpreadsheetEngine(ABC):
    """Abstract interface for spreadsheet engine verification."""

    @classmethod
    @abstractmethod
    def available(cls) -> bool:
        """Check if this engine is available on the current system."""
        ...

    @abstractmethod
    def open(self, path: Path) -> None:
        """Open a workbook."""
        ...

    @abstractmethod
    def read_cell(self, sheet: str, address: str) -> Any:
        """Read a single cell value."""
        ...

    @abstractmethod
    def recalculate(self) -> None:
        """Trigger workbook recalculation."""
        ...

    @abstractmethod
    def close(self) -> None:
        """Close the workbook and release resources."""
        ...

    def __enter__(self) -> SpreadsheetEngine:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
