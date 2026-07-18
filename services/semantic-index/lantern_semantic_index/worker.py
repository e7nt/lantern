from __future__ import annotations

import argparse
import hashlib
import json
import sys
import threading
import time
from pathlib import Path

import numpy as np

from .index import MODEL_NAME, RepositoryChangedError, SemanticIndex, StaleIndexError

PROTOCOL_VERSION = 1
MAX_REQUEST_BYTES = 64 * 1024
MAX_QUERY_CHARS = 4_096
REFRESH_INTERVAL_SECONDS = 0.5


class FastEmbedder:
    def __init__(self, cache_dir: Path):
        from fastembed import TextEmbedding

        self.model = TextEmbedding(model_name=MODEL_NAME, cache_dir=str(cache_dir))

    def documents(self, texts: list[str]) -> np.ndarray:
        return np.asarray(list(self.model.embed(texts, batch_size=64)), dtype=np.float32)

    def query(self, text: str) -> np.ndarray:
        return np.asarray(list(self.model.query_embed(text))[0], dtype=np.float32)


class Worker:
    def __init__(self, storage: Path, model_cache: Path):
        self.storage = storage
        self.model_cache = model_cache
        self.output_lock = threading.Lock()
        self.state_lock = threading.Lock()
        self.repository: Path | None = None
        self.index: SemanticIndex | None = None
        self.state = "closed"
        self.failure: str | None = None
        self.monitor_generation = 0

    def emit(self, value: dict) -> None:
        encoded = json.dumps(value, separators=(",", ":"))
        with self.output_lock:
            print(encoded, flush=True)

    def _repository_storage(self, repository: Path) -> Path:
        key = hashlib.sha256(str(repository).encode()).hexdigest()
        return self.storage / key

    def _build(self, repository: Path, index: SemanticIndex) -> None:
        try:
            result = index.build(repository)
        except RepositoryChangedError as cause:
            with self.state_lock:
                if self.repository != repository or self.index is not index:
                    return
                self.state = "stale"
                self.failure = str(cause)
            self.emit({"type": "index_refresh_deferred", "message": str(cause)})
            return
        except Exception as cause:  # worker boundary converts failures to typed JSON
            with self.state_lock:
                if self.repository != repository or self.index is not index:
                    return
                self.state = "failed"
                self.failure = str(cause)
            self.emit({"type": "index_failed", "message": str(cause)})
            return
        with self.state_lock:
            if self.repository != repository or self.index is not index:
                return
            self.state = "ready"
            self.failure = None
        self.emit(
            {
                "type": "index_ready",
                "revision": result.revision,
                "symbols": result.symbols,
                "embedded": result.embedded,
                "reused": result.reused,
            }
        )

    def refresh_once(self, repository: Path, index: SemanticIndex) -> bool:
        with self.state_lock:
            if self.repository != repository or self.index is not index:
                return False
            if self.state == "building":
                return False
        if index.status(repository) == "ready":
            return False
        with self.state_lock:
            if self.repository != repository or self.index is not index or self.state == "building":
                return False
            self.state = "building"
            self.failure = None
        self.emit({"type": "index_refreshing"})
        self._build(repository, index)
        return True

    def _monitor(self, repository: Path, index: SemanticIndex, generation: int) -> None:
        while True:
            time.sleep(REFRESH_INTERVAL_SECONDS)
            with self.state_lock:
                if self.monitor_generation != generation or self.repository != repository:
                    return
                state = self.state
            if state in {"ready", "stale"}:
                try:
                    self.refresh_once(repository, index)
                except Exception as cause:  # monitor boundary reports refresh failures
                    with self.state_lock:
                        if self.monitor_generation != generation:
                            return
                        self.state = "failed"
                        self.failure = str(cause)
                    self.emit({"type": "index_failed", "message": str(cause)})

    def open_workbench(self, repository_value: str) -> None:
        repository = Path(repository_value).resolve(strict=True)
        embedder = FastEmbedder(self.model_cache)
        index = SemanticIndex(self._repository_storage(repository), embedder)
        status = index.status(repository)
        with self.state_lock:
            self.monitor_generation += 1
            generation = self.monitor_generation
            self.repository = repository
            self.index = index
            self.failure = None
            self.state = status
            if status != "ready":
                self.state = "building"
        self.emit({"type": "workbench_opened", "state": self.state})
        if status != "ready":
            threading.Thread(target=self._build, args=(repository, index), daemon=True).start()
        threading.Thread(
            target=self._monitor,
            args=(repository, index, generation),
            daemon=True,
        ).start()

    def query(self, request_id: int, question: str) -> None:
        if not question.strip() or len(question) > MAX_QUERY_CHARS:
            raise ValueError("query must contain 1 to 4096 characters")
        with self.state_lock:
            state = self.state
            repository = self.repository
            index = self.index
            failure = self.failure
        if state != "ready" or repository is None or index is None:
            self.emit(
                {
                    "type": "query_result",
                    "id": request_id,
                    "state": state,
                    **({"message": failure} if failure else {}),
                    "matches": [],
                }
            )
            return
        if index.status(repository) != "ready":
            with self.state_lock:
                self.state = "stale"
            self.emit(
                {
                    "type": "query_result",
                    "id": request_id,
                    "state": "stale",
                    "matches": [],
                }
            )
            return
        try:
            matches = index.query(repository, question)
        except StaleIndexError:
            with self.state_lock:
                self.state = "stale"
            self.emit(
                {
                    "type": "query_result",
                    "id": request_id,
                    "state": "stale",
                    "matches": [],
                }
            )
            return
        self.emit(
            {
                "type": "query_result",
                "id": request_id,
                "state": "ready",
                "matches": [match.__dict__ for match in matches],
            }
        )

    def run(self) -> int:
        for raw_line in sys.stdin.buffer:
            if len(raw_line) > MAX_REQUEST_BYTES:
                self.emit({"type": "error", "message": "request exceeds 64 KiB"})
                continue
            try:
                request = json.loads(raw_line)
                method = request["method"]
                if method == "initialize":
                    if request.get("protocol_version") != PROTOCOL_VERSION:
                        raise ValueError(
                            f"unsupported protocol version; expected {PROTOCOL_VERSION}"
                        )
                    self.emit({"type": "initialized", "protocol_version": PROTOCOL_VERSION})
                elif method == "open_workbench":
                    self.open_workbench(request["repository"])
                elif method == "status":
                    with self.state_lock:
                        state = self.state
                    self.emit({"type": "status", "state": state})
                elif method == "query":
                    self.query(request["id"], request["query"])
                elif method == "shutdown":
                    with self.state_lock:
                        self.monitor_generation += 1
                    self.emit({"type": "shutdown"})
                    return 0
                else:
                    raise ValueError(f"unknown method: {method}")
            except (KeyError, TypeError, ValueError, json.JSONDecodeError) as cause:
                self.emit({"type": "error", "message": str(cause)})
        return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--storage", required=True, type=Path)
    parser.add_argument("--model-cache", required=True, type=Path)
    arguments = parser.parse_args()
    return Worker(arguments.storage, arguments.model_cache).run()


if __name__ == "__main__":
    raise SystemExit(main())
