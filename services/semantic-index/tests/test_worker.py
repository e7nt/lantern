from __future__ import annotations

from pathlib import Path

from test_index import FakeEmbedder, repository

from lantern_semantic_index.index import SemanticIndex
from lantern_semantic_index.worker import Worker


def test_refresh_once_rebuilds_a_changed_worktree_without_blocking_query(
    tmp_path: Path,
) -> None:
    repo = repository(tmp_path / "repo").resolve()
    index = SemanticIndex(tmp_path / "index", FakeEmbedder())
    index.build(repo)
    worker = Worker(tmp_path / "storage", tmp_path / "models")
    events: list[dict] = []
    worker.emit = events.append
    worker.repository = repo
    worker.index = index
    worker.state = "ready"

    assert worker.refresh_once(repo, index) is False
    (repo / "queue.ts").write_text(
        "function resumeNext() {\n  queue.clear();\n}\n", encoding="utf-8"
    )

    worker.query(7, "queue capacity")
    assert events[-1] == {
        "type": "query_result",
        "id": 7,
        "state": "stale",
        "matches": [],
    }
    assert worker.refresh_once(repo, index) is True
    assert worker.state == "ready"
    assert index.status(repo) == "ready"
    assert [event["type"] for event in events] == [
        "query_result",
        "index_refreshing",
        "index_ready",
    ]
    assert events[-1]["embedded"] == 1
    assert events[-1]["reused"] == 1
