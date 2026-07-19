from __future__ import annotations

import json
import os
import subprocess
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path

from run_live_trace import (
    PROJECT_ROOT,
    PROTOCOL_VERSION,
    EventReader,
    git_revision,
    require_executable,
    send,
    wait_for,
)


ROOT = Path(__file__).parent
DATASET_PATH = ROOT / "datasets" / "semantic_retrieval" / "v1.json"
SEMANTIC_WORKER = PROJECT_ROOT / "scripts" / "run-semantic-worker.sh"
MAX_VISIBLE_STATE_MS = 1_000


def main() -> int:
    daemon = require_executable(
        "LANTERN_DAEMON_BIN", PROJECT_ROOT / "target" / "debug" / "lantern-daemon"
    )
    if not SEMANTIC_WORKER.is_file():
        raise RuntimeError(f"semantic worker launcher is missing: {SEMANTIC_WORKER}")
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    case = next(item for item in dataset["cases"] if item["id"].startswith("requests-"))
    source = (PROJECT_ROOT / case["repository"]).resolve(strict=True)

    with tempfile.TemporaryDirectory(prefix="lantern-cold-grounding-") as temporary:
        repository = Path(temporary) / "repository"
        subprocess.run(
            ["git", "clone", "--quiet", "--no-hardlinks", str(source), str(repository)],
            check=True,
            timeout=30,
        )
        subprocess.run(
            ["git", "checkout", "--quiet", case["revision"]],
            cwd=repository,
            check=True,
            timeout=10,
        )
        environment = os.environ.copy()
        environment["LANTERN_SEMANTIC_WORKER"] = str(SEMANTIC_WORKER)
        environment["LANTERN_PI_BIN"] = "/bin/false"
        process = subprocess.Popen(
            [daemon],
            cwd=PROJECT_ROOT,
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        assert process.stdout is not None
        reader = EventReader(process.stdout)
        try:
            send(process, {"method": "initialize", "protocol_version": PROTOCOL_VERSION})
            wait_for(reader, "initialized", time.monotonic() + 10)
            send(process, {"method": "open_workbench", "repository": str(repository)})
            wait_for(reader, "workbench_opened", time.monotonic() + 10)
            started = time.monotonic()
            send(
                process,
                {
                    "method": "ask_agent",
                    "id": 1,
                    "repository": str(repository),
                    "query": case["question"],
                    "intent": "understand",
                },
            )
            deadline = started + MAX_VISIBLE_STATE_MS / 1000
            state_event = wait_for(reader, "grounding_state", deadline)
            elapsed_ms = round((time.monotonic() - started) * 1000)
            passed = (
                state_event
                == {
                    "type": "grounding_state",
                    "id": 1,
                    "state": "preparing_index",
                }
                and elapsed_ms <= MAX_VISIBLE_STATE_MS
            )
            report = {
                "generated_at": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "lantern_revision": git_revision(),
                "fixture_revision": case["revision"],
                "protocol_version": PROTOCOL_VERSION,
                "grounding_state": state_event["state"],
                "visible_state_ms": elapsed_ms,
                "passed": passed,
            }
            reports = ROOT / "reports"
            reports.mkdir(exist_ok=True)
            path = reports / f"cold-grounding-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}.json"
            path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(path)
            print(
                f"cold grounding state: {'PASS' if passed else 'FAIL'} — "
                f"{state_event['state']} in {elapsed_ms} ms"
            )
            return 0 if passed else 1
        finally:
            if process.poll() is None:
                send(process, {"method": "shutdown"})
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
