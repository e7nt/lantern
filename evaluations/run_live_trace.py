from __future__ import annotations

import hashlib
import json
import os
import queue
import shutil
import subprocess
import tempfile
import threading
import time
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath

os.environ.setdefault("DEEPEVAL_DISABLE_DOTENV", "1")

from deepeval.test_case import LLMTestCase

from metrics.contract import ToolJourneyContractMetric


ROOT = Path(__file__).parent
PROJECT_ROOT = ROOT.parent
DATASET_PATH = ROOT / "datasets" / "live_trace" / "v1.json"
PI_VERSION = "0.80.6"
MODEL = "gpt-5.4"
PROTOCOL_VERSION = 6
TURN_TIMEOUT_SECONDS = 45


def require_executable(environment_name: str, default: Path) -> str:
    configured = os.environ.get(environment_name)
    candidate = configured or str(default)
    resolved = shutil.which(candidate)
    if resolved is None:
        hint = f" Verify {environment_name}." if configured is not None else " Run `cargo build`."
        raise RuntimeError(f"required executable for {environment_name} was not found.{hint}")
    return resolved


def require_pi() -> str:
    configured = os.environ.get("LANTERN_PI_BIN", "pi")
    pi = shutil.which(configured)
    if pi is None:
        raise RuntimeError(
            "Pi was not found on PATH. Install the pinned version or set LANTERN_PI_BIN."
        )
    version = subprocess.run(
        [pi, "-v"], check=True, capture_output=True, text=True, timeout=5
    ).stdout.strip()
    if version != PI_VERSION:
        raise RuntimeError(f"Pi {PI_VERSION} is required; found {version or 'unknown'}")
    return pi


def write_fixture(repository: Path, files: dict[str, str]) -> None:
    for relative, content in files.items():
        path = PurePosixPath(relative)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError(f"fixture path is not repository-relative: {relative}")
        destination = repository.joinpath(*path.parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content, encoding="utf-8")


def snapshot_repository(repository: Path) -> dict[str, str]:
    snapshot = {}
    for path in sorted(repository.rglob("*")):
        relative = path.relative_to(repository).as_posix()
        if path.is_symlink():
            snapshot[relative] = "symlink"
        elif path.is_file():
            snapshot[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    return snapshot


def send(process: subprocess.Popen[str], request: dict) -> None:
    if process.stdin is None:
        raise RuntimeError("Lantern daemon stdin is unavailable")
    process.stdin.write(json.dumps(request, separators=(",", ":")) + "\n")
    process.stdin.flush()


class EventReader:
    def __init__(self, stream) -> None:
        self.events: queue.Queue[str | None] = queue.Queue()
        threading.Thread(target=self._read, args=(stream,), daemon=True).start()

    def _read(self, stream) -> None:
        for line in stream:
            self.events.put(line)
        self.events.put(None)

    def next(self, deadline: float) -> dict:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("Lantern live trace timed out")
        try:
            line = self.events.get(timeout=remaining)
        except queue.Empty as cause:
            raise RuntimeError("Lantern live trace timed out") from cause
        if line is None:
            raise RuntimeError("Lantern daemon closed before the trace settled")
        event = json.loads(line)
        if event.get("type") == "error":
            raise RuntimeError(
                f"Lantern rejected the live trace: {event.get('message', 'unknown error')}"
            )
        return event


def wait_for(
    reader: EventReader,
    event_type: str,
    deadline: float,
) -> dict:
    while True:
        event = reader.next(deadline)
        if event.get("type") == event_type:
            return event


def run_operation(
    process: subprocess.Popen[str],
    reader: EventReader,
    repository: Path,
    operation_id: int,
    question: str,
    cancel_after: str | None = None,
) -> dict:
    started = time.monotonic()
    send(
        process,
        {
            "method": "ask_agent",
            "id": operation_id,
            "repository": str(repository),
            "query": question,
        },
    )
    deadline = started + TURN_TIMEOUT_SECONDS
    answer: list[str] = []
    tools: list[dict] = []
    first_tool_ms = None
    first_text_ms = None
    outcome = None
    cancellation_latency_ms = None
    cancel_sent = False
    cancel_sent_at = None
    while True:
        event = reader.next(deadline)
        if event.get("id") != operation_id:
            continue
        elapsed_ms = round((time.monotonic() - started) * 1000)
        event_type = event["type"]
        if event_type == "tool_started":
            first_tool_ms = first_tool_ms or elapsed_ms
            tools.append({"tool": event["tool"], "relative_path": event.get("relative_path")})
        elif event_type == "text_delta":
            first_text_ms = first_text_ms or elapsed_ms
            answer.append(event["delta"])
        elif event_type == "completed":
            outcome = "completed"
        elif event_type == "cancelled":
            outcome = "cancelled"
            cancellation_latency_ms = event["cancellation_latency_ms"]
        if cancel_after == event_type and not cancel_sent:
            send(process, {"method": "cancel", "id": operation_id})
            cancel_sent = True
            cancel_sent_at = time.monotonic()
        if event_type == "settled":
            return {
                "answer": "".join(answer).strip(),
                "tools": tools,
                "first_tool_ms": first_tool_ms,
                "first_text_ms": first_text_ms,
                "settled_ms": elapsed_ms,
                "outcome": outcome,
                "cancellation_latency_ms": cancellation_latency_ms,
                "post_cancel_settled_ms": (
                    round((time.monotonic() - cancel_sent_at) * 1000)
                    if cancel_sent_at is not None
                    else None
                ),
            }


def evaluate_explanation(result: dict, contract: dict) -> tuple[bool, list[str]]:
    failures = []
    answer = result["answer"].casefold()
    missing_terms = [
        term for term in contract["required_answer_terms"] if term.casefold() not in answer
    ]
    if missing_terms:
        failures.append(f"answer omitted required grounded terms: {missing_terms}")
    trace = [tool["tool"] for tool in result["tools"]]
    journey_metric = ToolJourneyContractMetric(
        required_order=[],
        forbidden=contract["forbidden_tools"],
        max_calls=contract["max_tool_calls"],
    )
    journey_metric.measure(LLMTestCase(input=contract["question"], actual_output=json.dumps(trace)))
    if not journey_metric.is_successful():
        failures.append(journey_metric.reason)
    paths = {tool["relative_path"] for tool in result["tools"]}
    if contract["required_path"] not in paths:
        failures.append(f"required evidence path was not inspected: {contract['required_path']}")
    if result["outcome"] != "completed":
        failures.append(f"operation outcome was {result['outcome']!r}, expected 'completed'")
    if not result["fixture_unchanged"]:
        failures.append("explanation operation modified the repository fixture")
    if result["settled_ms"] > contract["max_settled_ms"]:
        failures.append(
            f"operation settled in {result['settled_ms']} ms; maximum is "
            f"{contract['max_settled_ms']} ms"
        )
    return not failures, failures


def evaluate_interruption(result: dict, contract: dict) -> tuple[bool, list[str]]:
    failures = []
    if result["outcome"] != "cancelled":
        failures.append(f"operation outcome was {result['outcome']!r}, expected 'cancelled'")
    if not result["fixture_unchanged"]:
        failures.append("interrupted inspection modified the repository fixture")
    latency = result["cancellation_latency_ms"]
    if latency is None or latency > contract["max_cancellation_ms"]:
        failures.append(
            f"cancellation latency was {latency!r} ms; maximum is "
            f"{contract['max_cancellation_ms']} ms"
        )
    post_cancel_settled_ms = result["post_cancel_settled_ms"]
    if (
        post_cancel_settled_ms is None
        or post_cancel_settled_ms > contract["max_post_cancel_settled_ms"]
    ):
        failures.append(
            f"interrupted operation settled {post_cancel_settled_ms!r} ms after cancellation; "
            f"maximum is {contract['max_post_cancel_settled_ms']} ms"
        )
    return not failures, failures


def git_revision() -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=5,
    ).stdout.strip()


def main() -> int:
    pi = require_pi()
    daemon = require_executable(
        "LANTERN_DAEMON_BIN", PROJECT_ROOT / "target" / "debug" / "lantern-daemon"
    )
    dataset_bytes = DATASET_PATH.read_bytes()
    dataset = json.loads(dataset_bytes)
    with tempfile.TemporaryDirectory(prefix="lantern-live-trace.") as workdir:
        repository = Path(workdir)
        write_fixture(repository, dataset["fixture"])
        fixture_snapshot = snapshot_repository(repository)
        environment = os.environ.copy()
        environment["LANTERN_PI_BIN"] = pi
        environment["LANTERN_PI_MODEL"] = MODEL
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
            deadline = time.monotonic() + 10
            send(process, {"method": "initialize", "protocol_version": PROTOCOL_VERSION})
            wait_for(reader, "initialized", deadline)
            send(process, {"method": "open_workbench", "repository": str(repository)})
            wait_for(reader, "workbench_opened", deadline)
            explanation = run_operation(
                process,
                reader,
                repository,
                1,
                dataset["explanation"]["question"],
            )
            explanation["fixture_unchanged"] = snapshot_repository(repository) == fixture_snapshot
            interruption = run_operation(
                process,
                reader,
                repository,
                2,
                dataset["interruption"]["question"],
                dataset["interruption"]["cancel_after"],
            )
            interruption["fixture_unchanged"] = snapshot_repository(repository) == fixture_snapshot
            send(process, {"method": "shutdown"})
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)

    explanation_passed, explanation_failures = evaluate_explanation(
        explanation, dataset["explanation"]
    )
    interruption_passed, interruption_failures = evaluate_interruption(
        interruption, dataset["interruption"]
    )
    passed = explanation_passed and interruption_passed
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    report = {
        "dataset": dataset["dataset"],
        "dataset_version": dataset["version"],
        "dataset_sha256": hashlib.sha256(dataset_bytes).hexdigest(),
        "lantern_revision": git_revision(),
        "driver": "lantern-protocol-v6",
        "pi_version": PI_VERSION,
        "provider": "openai-codex",
        "model": MODEL,
        "generated_at": timestamp,
        "passed": passed,
        "results": [
            {
                "case_id": dataset["explanation"]["id"],
                "passed": explanation_passed,
                "failures": explanation_failures,
                **explanation,
            },
            {
                "case_id": dataset["interruption"]["id"],
                "passed": interruption_passed,
                "failures": interruption_failures,
                **interruption,
            },
        ],
    }
    report_dir = ROOT / "reports"
    report_dir.mkdir(exist_ok=True)
    report_path = report_dir / f"live-trace-{timestamp}.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(report_path)
    for result in report["results"]:
        status = "PASS" if result["passed"] else "FAIL"
        detail = "; ".join(result["failures"]) or f"settled in {result['settled_ms']} ms"
        print(f"{result['case_id']}: {status} — {detail}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
