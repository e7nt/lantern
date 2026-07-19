from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path

os.environ.setdefault("DEEPEVAL_DISABLE_DOTENV", "1")

from deepeval.test_case import LLMTestCase

from metrics.contract import ToolJourneyContractMetric
from run_live_trace import (
    MODEL,
    PI_VERSION,
    PROJECT_ROOT,
    PROTOCOL_VERSION,
    EventReader,
    git_revision,
    require_executable,
    require_pi,
    run_operation,
    send,
    wait_for,
    write_fixture,
)


ROOT = Path(__file__).parent
DATASET_PATH = ROOT / "datasets" / "external_edit" / "v1.json"


def git(repository: Path, *arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        timeout=20,
    ).stdout


def initialize_repository(repository: Path, fixture: dict[str, str]) -> None:
    write_fixture(repository, fixture)
    git(repository, "init", "-q")
    git(repository, "add", ".")
    git(
        repository,
        "-c",
        "user.name=Lantern Evaluation",
        "-c",
        "user.email=lantern@example.invalid",
        "commit",
        "-qm",
        "baseline",
    )


def start_daemon(
    daemon: str, environment: dict[str, str], repository: Path
) -> tuple[subprocess.Popen[str], EventReader]:
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
    deadline = time.monotonic() + 10
    send(process, {"method": "initialize", "protocol_version": PROTOCOL_VERSION})
    wait_for(reader, "initialized", deadline)
    send(process, {"method": "open_workbench", "repository": str(repository)})
    wait_for(reader, "workbench_opened", deadline)
    return process, reader


def stop_daemon(process: subprocess.Popen[str]) -> None:
    send(process, {"method": "shutdown"})
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def symbol_request(repository: Path, operation_id: int, question: str, context: dict) -> dict:
    return {
        "method": "ask_agent_symbol",
        "id": operation_id,
        "repository": str(repository),
        "query": question,
        "context": context,
        "intent": "implement",
    }


def run_edit(daemon: str, environment: dict[str, str], dataset: dict) -> tuple[dict, dict]:
    with tempfile.TemporaryDirectory(prefix="lantern-external-edit.") as workdir:
        repository = Path(workdir)
        initialize_repository(repository, dataset["fixture"])
        process, reader = start_daemon(daemon, environment, repository)
        try:
            result = run_operation(
                process,
                reader,
                repository,
                1,
                dataset["question"],
                request=symbol_request(repository, 1, dataset["question"], dataset["context"]),
            )
        finally:
            stop_daemon(process)

        focused_test = subprocess.run(
            ["node", "status.test.mjs"],
            cwd=repository,
            capture_output=True,
            text=True,
            timeout=20,
        )
        actual_files = {
            path: (repository / path).read_text(encoding="utf-8")
            for path in dataset["expected_files"]
        }
        state = {
            "focused_test_passed": focused_test.returncode == 0,
            "changed_paths": sorted(
                line[3:] for line in git(repository, "status", "--porcelain=v1").splitlines()
            ),
            "files_match": actual_files == dataset["expected_files"],
            "staged_paths": git(repository, "diff", "--cached", "--name-only").splitlines(),
        }
        return result, state


def run_interruption(daemon: str, environment: dict[str, str], dataset: dict) -> tuple[dict, bool]:
    with tempfile.TemporaryDirectory(prefix="lantern-external-interrupt.") as workdir:
        repository = Path(workdir)
        initialize_repository(repository, dataset["fixture"])
        process, reader = start_daemon(daemon, environment, repository)
        try:
            question = dataset["interruption_question"]
            result = run_operation(
                process,
                reader,
                repository,
                2,
                question,
                cancel_after="tool_started",
                request=symbol_request(repository, 2, question, dataset["context"]),
            )
        finally:
            stop_daemon(process)
        unchanged = not git(repository, "status", "--porcelain=v1").strip()
        return result, unchanged


def evaluate_edit(result: dict, state: dict, contract: dict) -> tuple[bool, list[str]]:
    failures = []
    trace = [tool["tool"] for tool in result["tools"]]
    metric = ToolJourneyContractMetric(
        required_order=contract["required_tool_order"],
        forbidden=[],
        max_calls=contract["max_tool_calls"],
    )
    metric.measure(LLMTestCase(input=contract["question"], actual_output=json.dumps(trace)))
    if not metric.is_successful():
        failures.append(metric.reason)
    if result["outcome"] != "completed":
        failures.append(f"edit outcome was {result['outcome']!r}, expected 'completed'")
    if not state["files_match"]:
        failures.append("implementation or focused test did not match the requested result")
    if not state["focused_test_passed"]:
        failures.append("focused repository test failed")
    if state["changed_paths"] != sorted(contract["expected_files"]):
        failures.append(f"changed paths were {state['changed_paths']!r}")
    if state["staged_paths"]:
        failures.append("edit journey staged repository changes")
    first_tool_ms = result["first_tool_ms"]
    if first_tool_ms is None or first_tool_ms > contract["max_first_tool_ms"]:
        failures.append(
            f"first tool started in {first_tool_ms!r} ms; maximum is "
            f"{contract['max_first_tool_ms']} ms"
        )
    if result["settled_ms"] > contract["max_settled_ms"]:
        failures.append(
            f"edit settled in {result['settled_ms']} ms; maximum is {contract['max_settled_ms']} ms"
        )
    if "call" not in {evidence["source"] for evidence in result["evidence"]}:
        failures.append("edit journey observed no typed call evidence")
    return not failures, failures


def evaluate_interruption(
    result: dict, repository_unchanged: bool, contract: dict
) -> tuple[bool, list[str]]:
    failures = []
    if result["outcome"] != "cancelled":
        failures.append(f"interruption outcome was {result['outcome']!r}")
    if not repository_unchanged:
        failures.append("interrupted inspection changed repository state")
    if (
        result["cancellation_latency_ms"] is None
        or result["cancellation_latency_ms"] > contract["max_cancellation_ms"]
    ):
        failures.append("cancellation acknowledgement exceeded its budget")
    if (
        result["post_cancel_settled_ms"] is None
        or result["post_cancel_settled_ms"] > contract["max_post_cancel_settled_ms"]
    ):
        failures.append("post-cancellation settlement exceeded its budget")
    return not failures, failures


def main() -> int:
    dataset_bytes = DATASET_PATH.read_bytes()
    dataset = json.loads(dataset_bytes)
    daemon = require_executable(
        "LANTERN_DAEMON_BIN", PROJECT_ROOT / "target" / "debug" / "lantern-daemon"
    )
    environment = os.environ.copy()
    environment["LANTERN_PI_BIN"] = require_pi()
    environment["LANTERN_PI_MODEL"] = MODEL
    edit, edit_state = run_edit(daemon, environment, dataset)
    interruption, repository_unchanged = run_interruption(daemon, environment, dataset)
    edit_passed, edit_failures = evaluate_edit(edit, edit_state, dataset)
    interruption_passed, interruption_failures = evaluate_interruption(
        interruption, repository_unchanged, dataset
    )
    passed = edit_passed and interruption_passed
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    report = {
        "dataset": dataset["dataset"],
        "dataset_version": dataset["version"],
        "dataset_sha256": hashlib.sha256(dataset_bytes).hexdigest(),
        "lantern_revision": git_revision(),
        "driver": "lantern-protocol-v7",
        "pi_version": PI_VERSION,
        "provider": "openai-codex",
        "model": MODEL,
        "generated_at": timestamp,
        "passed": passed,
        "edit": {
            "passed": edit_passed,
            "failures": edit_failures,
            "tools": edit["tools"],
            "evidence_sources": [item["source"] for item in edit["evidence"]],
            "first_tool_ms": edit["first_tool_ms"],
            "first_text_ms": edit["first_text_ms"],
            "settled_ms": edit["settled_ms"],
            **edit_state,
        },
        "interruption": {
            "passed": interruption_passed,
            "failures": interruption_failures,
            "outcome": interruption["outcome"],
            "cancellation_latency_ms": interruption["cancellation_latency_ms"],
            "post_cancel_settled_ms": interruption["post_cancel_settled_ms"],
            "repository_unchanged": repository_unchanged,
        },
    }
    report_dir = ROOT / "reports"
    report_dir.mkdir(exist_ok=True)
    report_path = report_dir / f"external-edit-{timestamp}.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(report_path)
    print(f"edit: {'PASS' if edit_passed else 'FAIL'} — {edit_failures or edit_state}")
    print(
        f"interruption: {'PASS' if interruption_passed else 'FAIL'} — "
        f"{interruption_failures or interruption['cancellation_latency_ms']}"
    )
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
