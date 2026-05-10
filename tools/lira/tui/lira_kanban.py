#!/usr/bin/env python3
#
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "pyyaml>=6.0.2",
#   "textual>=4.0.0",
# ]
# ///
"""Textual kanban board for local lira projects.

Run with:

    uv run tui/lira_kanban.py
"""

from __future__ import annotations

import argparse
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_STATUSES = (
    ("backlog", "Backlog", False),
    ("todo", "To Do", False),
    ("in-progress", "In Progress", False),
    ("blocked", "Blocked", False),
    ("in-review", "In Review", False),
    ("done", "Done", True),
    ("cancelled", "Cancelled", True),
    ("archived", "Archived", True),
)


@dataclass(frozen=True)
class StatusDef:
    id: str
    name: str
    terminal: bool = False


@dataclass(frozen=True)
class TicketCard:
    id: str
    title: str
    status: str
    priority: str
    assignee: str | None
    claimed_by: str | None
    task_total: int
    task_done: int


@dataclass(frozen=True)
class StatusColumn:
    status: StatusDef
    tickets: tuple[TicketCard, ...]


@dataclass(frozen=True)
class ProjectBoard:
    key: str
    name: str
    columns: tuple[StatusColumn, ...]

    @property
    def ticket_count(self) -> int:
        return sum(len(column.tickets) for column in self.columns)


@dataclass(frozen=True)
class BoardSnapshot:
    home: Path
    projects: tuple[ProjectBoard, ...]
    errors: tuple[str, ...] = ()

    @property
    def ticket_count(self) -> int:
        return sum(project.ticket_count for project in self.projects)


def resolve_lira_home(home: str | Path | None = None) -> Path:
    if home is not None:
        return Path(home).expanduser()
    if env_home := os.environ.get("LIRA_HOME"):
        return Path(env_home).expanduser()
    return Path.home() / ".lira"


def load_snapshot(home: str | Path | None = None) -> BoardSnapshot:
    root = resolve_lira_home(home)
    projects_root = root / "projects"
    errors: list[str] = []
    projects: list[ProjectBoard] = []

    if not projects_root.exists():
        return BoardSnapshot(root, (), (f"Missing projects directory: {projects_root}",))

    for project_dir in sorted(path for path in projects_root.iterdir() if path.is_dir()):
        try:
            projects.append(_load_project(project_dir))
        except Exception as exc:  # noqa: BLE001 - TUI should survive one bad project.
            errors.append(f"{project_dir.name}: {exc}")

    return BoardSnapshot(root, tuple(projects), tuple(errors))


def _load_project(project_dir: Path) -> ProjectBoard:
    project_data = _load_yaml(project_dir / "project.yaml")
    key = str(project_data.get("key") or project_dir.name)
    name = str(project_data.get("name") or key)
    statuses = _load_statuses(project_dir / "workflow.yaml")
    tickets = _load_tickets(project_dir / "tickets")

    known_statuses = {status.id for status in statuses}
    extra_statuses = sorted({ticket.status for ticket in tickets} - known_statuses)
    all_statuses = list(statuses) + [
        StatusDef(id=status, name=status.replace("-", " ").title()) for status in extra_statuses
    ]

    columns = []
    for status in all_statuses:
        cards = tuple(ticket for ticket in tickets if ticket.status == status.id)
        columns.append(StatusColumn(status=status, tickets=cards))
    return ProjectBoard(key=key, name=name, columns=tuple(columns))


def _load_statuses(path: Path) -> tuple[StatusDef, ...]:
    if not path.exists():
        return tuple(StatusDef(*status) for status in DEFAULT_STATUSES)
    workflow = _load_yaml(path)
    raw_statuses = workflow.get("statuses") or []
    statuses = []
    for raw in raw_statuses:
        if isinstance(raw, dict) and raw.get("id"):
            statuses.append(
                StatusDef(
                    id=str(raw["id"]),
                    name=str(raw.get("name") or raw["id"]),
                    terminal=bool(raw.get("terminal", False)),
                )
            )
    return tuple(statuses) or tuple(StatusDef(*status) for status in DEFAULT_STATUSES)


def _load_tickets(tickets_root: Path) -> tuple[TicketCard, ...]:
    if not tickets_root.exists():
        return ()

    tickets = []
    for path in sorted(tickets_root.glob("*/*.yaml")):
        data = _load_yaml(path)
        ticket_id = str(data.get("id") or path.stem)
        tasks = data.get("tasks") or []
        task_done = sum(
            1
            for task in tasks
            if isinstance(task, dict) and str(task.get("status")) == "done"
        )
        agent = data.get("agent") if isinstance(data.get("agent"), dict) else {}
        tickets.append(
            TicketCard(
                id=ticket_id,
                title=str(data.get("title") or ticket_id),
                status=str(data.get("status") or path.parent.name),
                priority=str(data.get("priority") or "medium"),
                assignee=_optional_str(data.get("assignee")),
                claimed_by=_optional_str(agent.get("claimed_by")),
                task_total=len(tasks),
                task_done=task_done,
            )
        )
    return tuple(sorted(tickets, key=lambda ticket: ticket.id))


def _load_yaml(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    try:
        import yaml
    except ImportError as exc:
        raise RuntimeError(
            "PyYAML is required. Run with uv so script dependencies are installed: "
            "uv run tui/lira_kanban.py"
        ) from exc

    with path.open("r", encoding="utf-8") as handle:
        data = yaml.safe_load(handle) or {}
    if not isinstance(data, dict):
        return {}
    return data


def _optional_str(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def create_tui_app(
    home: str | Path | None = None, interval: float = 5.0, project: str | None = None
) -> Any:
    try:
        from textual.app import App, ComposeResult
        from textual.containers import Horizontal, HorizontalScroll, Vertical, VerticalScroll
        from textual.widgets import Static
    except ImportError as exc:
        raise RuntimeError(
            "Textual and Rich are required. Run with uv so script dependencies are installed: "
            "uv run tui/lira_kanban.py"
        ) from exc

    class LiraKanbanApp(App[None]):
        CSS = """
        Screen {
            background: #0f1318;
            color: #d8dee9;
        }

        #topbar {
            height: 3;
            padding: 0 2;
            background: #151b24;
            color: #e5e9f0;
            border-bottom: solid #2f3b4f;
        }

        #shell {
            height: 1fr;
        }

        #sidebar {
            width: 34;
            min-width: 28;
            height: 1fr;
            background: #111821;
            border-right: solid #2f3b4f;
            padding: 1;
        }

        #sidebar-title {
            height: 1;
            color: #88c0d0;
            text-style: bold;
        }

        #home {
            height: 2;
            color: #6f7d91;
        }

        #projects {
            width: 1fr;
            height: 1fr;
        }

        .project-row {
            height: 4;
            margin-bottom: 1;
            padding: 0 1;
            background: #151d28;
            border-left: solid #344258;
            color: #b8c1d1;
        }

        .project-row.selected {
            background: #223049;
            border-left: solid #88c0d0;
            color: #eceff4;
            text-style: bold;
        }

        #main {
            width: 1fr;
            height: 1fr;
            background: #0f1318;
        }

        #project-header {
            height: 4;
            padding: 0 2;
            background: #111821;
            border-bottom: solid #2f3b4f;
        }

        #board {
            height: 1fr;
            padding: 1 2;
        }

        .kanban-column {
            width: 34;
            min-width: 28;
            height: 1fr;
            margin-right: 1;
            background: #151d28;
            border: round #344258;
        }

        .kanban-column.terminal {
            border: round #4f6f3a;
        }

        .column-header {
            height: 3;
            padding: 0 1;
            background: #1b2636;
            border-bottom: solid #344258;
            color: #eceff4;
            text-style: bold;
        }

        .column-header.terminal {
            color: #a3be8c;
        }

        .column-body {
            height: 1fr;
            padding: 1;
        }

        .ticket-card {
            width: 100%;
            min-height: 5;
            margin-bottom: 1;
            padding: 1;
            background: #202a3a;
            border-left: solid #5e81ac;
            color: #e5e9f0;
        }

        .ticket-card.high {
            border-left: solid #bf616a;
        }

        .ticket-card.medium {
            border-left: solid #ebcb8b;
        }

        .ticket-card.low {
            border-left: solid #a3be8c;
        }

        .empty-column {
            height: 3;
            padding: 1;
            color: #6f7d91;
        }
        """
        BINDINGS = [
            ("q", "quit", "Quit"),
            ("r", "refresh", "Refresh"),
            ("j", "next_project", "Next project"),
            ("k", "previous_project", "Previous project"),
        ]

        def __init__(self) -> None:
            super().__init__()
            self.home = resolve_lira_home(home)
            self.interval = interval
            self.selected_project = project
            self.selected_index = 0
            self.snapshot = BoardSnapshot(self.home, ())

        def compose(self) -> ComposeResult:
            yield Static(id="topbar")
            with Horizontal(id="shell"):
                with Vertical(id="sidebar"):
                    yield Static("lira projects", id="sidebar-title", markup=False)
                    yield Static(id="home")
                    yield VerticalScroll(id="projects")
                with Vertical(id="main"):
                    yield Static(id="project-header")
                    yield HorizontalScroll(id="board")

        def on_mount(self) -> None:
            self.set_interval(self.interval, self.refresh_snapshot)
            self.call_later(self.refresh_snapshot)

        def refresh_snapshot(self) -> None:
            previous_key = self.current_project_key
            self.snapshot = load_snapshot(self.home)
            self._select_project(previous_key or self.selected_project)
            self.run_worker(self._render(), group="render", exclusive=True)

        def action_refresh(self) -> None:
            self.refresh_snapshot()

        def action_next_project(self) -> None:
            if self.snapshot.projects:
                self.selected_index = (self.selected_index + 1) % len(self.snapshot.projects)
                self.selected_project = self.current_project_key
                self.run_worker(self._render(), group="render", exclusive=True)

        def action_previous_project(self) -> None:
            if self.snapshot.projects:
                self.selected_index = (self.selected_index - 1) % len(self.snapshot.projects)
                self.selected_project = self.current_project_key
                self.run_worker(self._render(), group="render", exclusive=True)

        @property
        def current_project_key(self) -> str | None:
            if not self.snapshot.projects:
                return None
            return self.snapshot.projects[self.selected_index].key

        def _select_project(self, preferred_key: str | None) -> None:
            if not self.snapshot.projects:
                self.selected_index = 0
                return
            if preferred_key:
                for idx, board in enumerate(self.snapshot.projects):
                    if board.key == preferred_key:
                        self.selected_index = idx
                        return
            self.selected_index = min(self.selected_index, len(self.snapshot.projects) - 1)

        async def _render(self) -> None:
            self.query_one("#topbar", Static).update(self._topbar_text(), layout=True)
            self.query_one("#home", Static).update(str(self.snapshot.home), layout=True)
            self.query_one("#project-header", Static).update(
                self._project_header_text(), layout=True
            )
            await self._render_projects()
            await self._render_board()

        async def _render_projects(self) -> None:
            projects = self.query_one("#projects", VerticalScroll)
            await projects.remove_children()
            rows = []
            if not self.snapshot.projects:
                rows.append(Static("No projects found", classes="project-row", markup=False))
            for idx, board in enumerate(self.snapshot.projects):
                classes = "project-row selected" if idx == self.selected_index else "project-row"
                rows.append(
                    Static(
                        f"{board.key}\n{board.name}\n{board.ticket_count} tickets",
                        classes=classes,
                        markup=False,
                    )
                )
            if self.snapshot.errors:
                for error in self.snapshot.errors[:6]:
                    rows.append(Static(f"Load issue\n{error}", classes="project-row", markup=False))
            if rows:
                await projects.mount(*rows)

        async def _render_board(self) -> None:
            board_widget = self.query_one("#board", HorizontalScroll)
            await board_widget.remove_children()
            if not self.snapshot.projects:
                await board_widget.mount(
                    Static(
                        "No lira projects found. Create one with `lira project create`.",
                        classes="empty-column",
                        markup=False,
                    )
                )
                return
            board = self.snapshot.projects[self.selected_index]
            columns = [self._column_widget(column) for column in board.columns]
            await board_widget.mount(*columns)

        def _column_widget(self, column: StatusColumn) -> Vertical:
            terminal_class = " terminal" if column.status.terminal else ""
            header = Static(
                f"{column.status.name}\n{len(column.tickets)} tickets",
                classes=f"column-header{terminal_class}",
                markup=False,
            )
            cards = [_ticket_widget(ticket) for ticket in column.tickets]
            if not cards:
                cards = [Static("No tickets", classes="empty-column", markup=False)]
            body = VerticalScroll(*cards, classes="column-body")
            return Vertical(header, body, classes=f"kanban-column{terminal_class}")

        def _topbar_text(self) -> str:
            return (
                "lira kanban\n"
                f"auto-refresh {self.interval:g}s | j/k project | r refresh | q quit"
            )

        def _project_header_text(self) -> str:
            if not self.snapshot.projects:
                return "No projects\nCreate a project with lira project create"
            board = self.snapshot.projects[self.selected_index]
            active = sum(
                len(column.tickets)
                for column in board.columns
                if not column.status.terminal and column.status.id != "backlog"
            )
            done = sum(
                len(column.tickets) for column in board.columns if column.status.terminal
            )
            return (
                f"{board.key} - {board.name}\n"
                f"{board.ticket_count} tickets | {active} active | {done} terminal"
            )

    def _ticket_widget(ticket: TicketCard) -> Static:
        bits = [ticket.priority, f"tasks {ticket.task_done}/{ticket.task_total}"]
        if ticket.assignee:
            bits.append(f"@{ticket.assignee}")
        if ticket.claimed_by:
            bits.append(f"claimed:{ticket.claimed_by}")
        return Static(
            f"{ticket.id}\n{ticket.title}\n{' | '.join(bits)}",
            classes=f"ticket-card {ticket.priority.lower()}",
            markup=False,
        )

    return LiraKanbanApp()


def run_tui(home: str | Path | None = None, interval: float = 5.0, project: str | None = None) -> None:
    create_tui_app(home=home, interval=interval, project=project).run()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Live Textual kanban board for lira.")
    parser.add_argument(
        "--home",
        default=None,
        help="Path to lira home. Defaults to LIRA_HOME or ~/.lira.",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=5.0,
        help="Refresh interval in seconds. Default: 5.",
    )
    parser.add_argument(
        "--project",
        default=None,
        help="Project key to select initially.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    run_tui(home=args.home, interval=args.interval, project=args.project)


if __name__ == "__main__":
    main()
