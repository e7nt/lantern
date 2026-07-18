from __future__ import annotations

import subprocess
from pathlib import Path

import numpy as np
import pytest

from lantern_semantic_index.index import SemanticIndex, extract_repository_symbols


class FakeEmbedder:
    def __init__(self) -> None:
        self.embedded: list[str] = []

    @staticmethod
    def _vector(text: str) -> np.ndarray:
        folded = text.casefold()
        return np.asarray(
            [float("credential" in folded), float("queue" in folded), 1.0], dtype=np.float32
        )

    def documents(self, texts: list[str]) -> np.ndarray:
        self.embedded.extend(texts)
        return np.asarray([self._vector(text) for text in texts])

    def query(self, text: str) -> np.ndarray:
        return self._vector(text)


def repository(tmp_path: Path) -> Path:
    tmp_path.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=tmp_path, check=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=tmp_path, check=True)
    (tmp_path / "session.py").write_text(
        "def remove_secret_on_redirect(url):\n    return url.replace('credential', '')\n",
        encoding="utf-8",
    )
    (tmp_path / "queue.ts").write_text(
        "function resumeNext() {\n  queue.dequeue().run();\n}\n", encoding="utf-8"
    )
    subprocess.run(["git", "add", "."], cwd=tmp_path, check=True)
    subprocess.run(["git", "commit", "-qm", "fixture"], cwd=tmp_path, check=True)
    return tmp_path


def test_extracts_language_symbols_with_bounded_source(tmp_path: Path) -> None:
    symbols = extract_repository_symbols(repository(tmp_path))
    assert {(symbol.relative_path, symbol.name) for symbol in symbols} == {
        ("queue.ts", "resumeNext"),
        ("session.py", "remove_secret_on_redirect"),
    }
    assert all(len(symbol.text.splitlines()) <= 16 for symbol in symbols)


def test_build_reuses_unchanged_vectors_and_query_requires_current_revision(tmp_path: Path) -> None:
    repo = repository(tmp_path / "repo")
    embedder = FakeEmbedder()
    index = SemanticIndex(tmp_path / "index", embedder)
    first = index.build(repo)
    assert first.embedded == 2
    assert first.reused == 0
    assert len(embedder.embedded) == 2

    second = index.build(repo)
    assert second.embedded == 0
    assert second.reused == 2
    assert len(embedder.embedded) == 2
    assert index.status(repo) == "ready"
    assert index.query(repo, "credential leakage", limit=1)[0].name == "remove_secret_on_redirect"

    (repo / "queue.ts").write_text(
        "function resumeNext() {\n  queue.dequeue().run();\n  queue.dequeue().run();\n}\n",
        encoding="utf-8",
    )
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-qm", "change queue"], cwd=repo, check=True)
    with pytest.raises(RuntimeError, match="stale"):
        index.query(repo, "queue capacity")
    assert index.status(repo) == "stale"
    updated = index.build(repo)
    assert updated.embedded == 1
    assert updated.reused == 1


def test_uncommitted_tracked_edit_changes_revision_and_reuses_other_vectors(
    tmp_path: Path,
) -> None:
    repo = repository(tmp_path / "repo")
    embedder = FakeEmbedder()
    index = SemanticIndex(tmp_path / "index", embedder)
    index.build(repo)

    (repo / "queue.ts").write_text(
        "function resumeNext() {\n  queue.clear();\n}\n", encoding="utf-8"
    )

    assert index.status(repo) == "stale"
    with pytest.raises(RuntimeError, match="stale"):
        index.query(repo, "queue capacity")
    refreshed = index.build(repo)
    assert refreshed.embedded == 1
    assert refreshed.reused == 1
    assert index.status(repo) == "ready"


def test_rejects_corrupt_vector_manifest_pair(tmp_path: Path) -> None:
    repo = repository(tmp_path / "repo")
    index = SemanticIndex(tmp_path / "index", FakeEmbedder())
    result = index.build(repo)
    np.save(
        tmp_path / "index" / "revisions" / result.revision / "vectors.npy",
        np.zeros((1, 3), dtype=np.float32),
    )
    with pytest.raises(RuntimeError, match="manifest and vectors differ"):
        index.query(repo, "credential")
