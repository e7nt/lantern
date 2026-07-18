from __future__ import annotations

import json
import os
import selectors
import subprocess
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path

from run_live_trace import PROJECT_ROOT, git_revision


ROOT = Path(__file__).parent
DATASET_PATH = ROOT / "datasets" / "semantic_retrieval" / "v1.json"
PYTHON = PROJECT_ROOT / ".lantern" / "toolchains" / "semantic-index" / "bin" / "python"
MODEL_CACHE = PROJECT_ROOT / ".lantern" / "toolchains" / "semantic-models"
SERVICE = PROJECT_ROOT / "services" / "semantic-index"
MAX_INITIAL_BUILD_SECONDS = 60
MAX_REFRESH_SECONDS = 3


def send(process: subprocess.Popen[str], request: dict) -> None:
    assert process.stdin is not None
    process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
    process.stdin.flush()


def receive(
    process: subprocess.Popen[str], selector: selectors.BaseSelector, deadline: float
) -> dict:
    remaining = deadline - time.monotonic()
    if remaining <= 0 or not selector.select(remaining):
        raise RuntimeError("semantic worker event timed out")
    assert process.stdout is not None
    line = process.stdout.readline()
    if not line:
        raise RuntimeError("semantic worker stopped before the expected event")
    return json.loads(line)


def wait_for(
    process: subprocess.Popen[str],
    selector: selectors.BaseSelector,
    event_type: str,
    deadline: float,
) -> dict:
    while True:
        event = receive(process, selector, deadline)
        if event.get("type") == "index_failed":
            raise RuntimeError(f"semantic index failed: {event.get('message', 'unknown cause')}")
        if event.get("type") == event_type:
            return event


def main() -> int:
    if not PYTHON.is_file():
        raise RuntimeError(
            f"semantic worker is not prepared; run {PROJECT_ROOT}/frontend/helix/prepare.sh"
        )
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    case = next(item for item in dataset["cases"] if item["id"].startswith("p-limit-"))
    source = (PROJECT_ROOT / case["repository"]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="lantern-semantic-refresh-") as temporary:
        temporary_path = Path(temporary)
        repository = temporary_path / "repository"
        subprocess.run(
            ["git", "clone", "--quiet", "--no-hardlinks", str(source), str(repository)],
            check=True,
            timeout=20,
        )
        subprocess.run(
            ["git", "checkout", "--quiet", case["revision"]],
            cwd=repository,
            check=True,
            timeout=10,
        )
        process = subprocess.Popen(
            [
                str(PYTHON),
                "-m",
                "lantern_semantic_index.worker",
                "--storage",
                str(temporary_path / "index"),
                "--model-cache",
                str(MODEL_CACHE),
            ],
            cwd=SERVICE,
            env={**os.environ, "PYTHONPATH": str(SERVICE)},
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        assert process.stdout is not None
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        try:
            send(process, {"method": "initialize", "protocol_version": 1})
            wait_for(process, selector, "initialized", time.monotonic() + 10)
            send(process, {"method": "open_workbench", "repository": str(repository)})
            wait_for(process, selector, "workbench_opened", time.monotonic() + 10)
            initial = wait_for(
                process,
                selector,
                "index_ready",
                time.monotonic() + MAX_INITIAL_BUILD_SECONDS,
            )

            source_path = repository / "index.js"
            original = source_path.read_text(encoding="utf-8")
            changed = original.replace(
                "Expected `rejectOnClear` to be a boolean",
                "Expected the reject-on-clear option to be boolean",
                1,
            )
            if changed == original:
                raise RuntimeError("p-limit refresh fixture no longer contains the expected symbol")
            started = time.monotonic()
            source_path.write_text(changed, encoding="utf-8")
            wait_for(
                process,
                selector,
                "index_refreshing",
                started + MAX_REFRESH_SECONDS,
            )
            refreshed = wait_for(
                process,
                selector,
                "index_ready",
                started + MAX_REFRESH_SECONDS,
            )
            refresh_ms = round((time.monotonic() - started) * 1000)
            send(
                process,
                {"method": "query", "id": 1, "query": "wake queued work when capacity returns"},
            )
            query = wait_for(process, selector, "query_result", time.monotonic() + 5)
            matched_paths = [match["relative_path"] for match in query["matches"]]
            passed = (
                refreshed["embedded"] == 1
                and refreshed["reused"] > 0
                and query["state"] == "ready"
                and "index.js" in matched_paths
                and refresh_ms <= MAX_REFRESH_SECONDS * 1000
            )
            report = {
                "generated_at": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "lantern_revision": git_revision(),
                "fixture_revision": case["revision"],
                "initial": initial,
                "refresh": {**refreshed, "elapsed_ms": refresh_ms},
                "query_state": query["state"],
                "matched_paths": matched_paths,
                "repository_dirty": bool(
                    subprocess.run(
                        ["git", "status", "--porcelain"],
                        cwd=repository,
                        check=True,
                        capture_output=True,
                        text=True,
                    ).stdout
                ),
                "passed": passed,
            }
            reports = ROOT / "reports"
            reports.mkdir(exist_ok=True)
            path = reports / f"semantic-refresh-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}.json"
            path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(path)
            print(
                f"semantic refresh: {'PASS' if passed else 'FAIL'} — {refresh_ms} ms, "
                f"{refreshed['embedded']} embedded, {refreshed['reused']} reused"
            )
            return 0 if passed else 1
        finally:
            if process.poll() is None:
                send(process, {"method": "shutdown"})
                process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
