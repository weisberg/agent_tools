from __future__ import annotations

import os
import tempfile
import textwrap
import unittest
from pathlib import Path

from tui.lira_kanban import load_snapshot, resolve_lira_home


class LiraKanbanTests(unittest.TestCase):
    def test_load_snapshot_groups_tickets_by_workflow_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / ".lira"
            project = _project(root, "LIRA", "lira")
            _ticket(
                project,
                "backlog",
                "LIRA-1",
                "Build a kanban TUI",
                priority="high",
                tasks=[("T1", "Sketch board", "done"), ("T2", "Render board", "todo")],
            )
            _ticket(
                project,
                "in-progress",
                "LIRA-2",
                "Wire refresh loop",
                assignee="codex",
                claimed_by="runner",
                tasks=[("T1", "Refresh regularly", "todo")],
            )

            snapshot = load_snapshot(root)

            self.assertEqual(snapshot.errors, ())
            self.assertEqual(snapshot.ticket_count, 2)
            self.assertEqual(snapshot.projects[0].key, "LIRA")
            self.assertEqual([column.status.id for column in snapshot.projects[0].columns[:3]], [
                "backlog",
                "todo",
                "in-progress",
            ])
            backlog_ticket = snapshot.projects[0].columns[0].tickets[0]
            self.assertEqual(backlog_ticket.id, "LIRA-1")
            self.assertEqual(backlog_ticket.task_done, 1)
            self.assertEqual(backlog_ticket.task_total, 2)
            in_progress_ticket = snapshot.projects[0].columns[2].tickets[0]
            self.assertEqual(in_progress_ticket.assignee, "codex")
            self.assertEqual(in_progress_ticket.claimed_by, "runner")

    def test_load_snapshot_adds_unknown_status_columns(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / ".lira"
            project = _project(root, "LIRA", "lira")
            _ticket(project, "paused", "LIRA-9", "Paused work")

            snapshot = load_snapshot(root)

            self.assertIn(
                "paused",
                [column.status.id for column in snapshot.projects[0].columns],
            )
            paused = snapshot.projects[0].columns[-1]
            self.assertEqual(paused.status.name, "Paused")
            self.assertEqual(paused.tickets[0].id, "LIRA-9")

    def test_resolve_lira_home_prefers_argument_then_environment(self) -> None:
        previous = os.environ.get("LIRA_HOME")
        try:
            os.environ["LIRA_HOME"] = "/tmp/from-env"
            self.assertEqual(resolve_lira_home("/tmp/from-arg"), Path("/tmp/from-arg"))
            self.assertEqual(resolve_lira_home(), Path("/tmp/from-env"))
        finally:
            if previous is None:
                os.environ.pop("LIRA_HOME", None)
            else:
                os.environ["LIRA_HOME"] = previous


def _project(root: Path, key: str, name: str) -> Path:
    project = root / "projects" / key
    project.mkdir(parents=True)
    (project / "project.yaml").write_text(
        textwrap.dedent(
            f"""
            schema_version: 3
            key: {key}
            name: {name}
            default_status: backlog
            """
        ),
        encoding="utf-8",
    )
    (project / "workflow.yaml").write_text(
        textwrap.dedent(
            f"""
            schema_version: 3
            project: {key}
            default_status: backlog
            statuses:
            - id: backlog
              name: Backlog
              terminal: false
            - id: todo
              name: To Do
              terminal: false
            - id: in-progress
              name: In Progress
              terminal: false
            - id: done
              name: Done
              terminal: true
            """
        ),
        encoding="utf-8",
    )
    return project


def _ticket(
    project: Path,
    status: str,
    ticket_id: str,
    title: str,
    *,
    priority: str = "medium",
    assignee: str | None = None,
    claimed_by: str | None = None,
    tasks: list[tuple[str, str, str]] | None = None,
) -> None:
    tasks = tasks or [("T1", "Do the work", "todo")]
    ticket_dir = project / "tickets" / status
    ticket_dir.mkdir(parents=True, exist_ok=True)
    lines = [
        "schema_version: 3",
        f"id: {ticket_id}",
        f"project: {project.name}",
        f"title: {title}",
        "type: task",
        f"status: {status}",
        f"priority: {priority}",
    ]
    if assignee:
        lines.append(f"assignee: {assignee}")
    lines.append("tasks:")
    for task_id, task_title, task_status in tasks:
        lines.extend(
            [
                f"  - id: {task_id}",
                f"    title: {task_title}",
                f"    status: {task_status}",
                "    tags: []",
            ]
        )
    lines.append("agent:")
    if claimed_by:
        lines.append(f"  claimed_by: {claimed_by}")
    (ticket_dir / f"{ticket_id}.yaml").write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
