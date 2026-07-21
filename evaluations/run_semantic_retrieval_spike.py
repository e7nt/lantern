from __future__ import annotations

import hashlib
import json
import os
import subprocess
import time
from datetime import UTC, datetime
from pathlib import Path

os.environ.setdefault("DEEPEVAL_DISABLE_DOTENV", "1")

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
)


ROOT = Path(__file__).parent
DATASET_PATH = ROOT / "datasets" / "semantic_retrieval" / "v1.json"


def repository_status(repository: Path) -> str:
    return subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        timeout=20,
    ).stdout


def require_repository(case: dict) -> Path:
    repository = (PROJECT_ROOT / case["repository"]).resolve()
    if not repository.is_dir():
        raise RuntimeError(
            f"{case['id']} requires {case['repository']} at {case['revision']}; "
            f"clone {case['remote']} there and check out the pinned revision"
        )
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        timeout=5,
    ).stdout.strip()
    if revision != case["revision"]:
        raise RuntimeError(f"{case['id']} requires revision {case['revision']}; found {revision}")
    return repository


def evaluate(result: dict, case: dict) -> tuple[bool, list[str]]:
    failures = []
    folded_answer = result["answer"].casefold()
    missing_terms = [
        term for term in case["required_answer_terms"] if term.casefold() not in folded_answer
    ]
    if missing_terms:
        failures.append(f"answer omitted required grounded terms: {missing_terms}")
    observed_paths = {
        item["relative_path"]
        for item in [*result["tools"], *result["evidence"]]
        if item["relative_path"] is not None
    }
    missing_paths = [path for path in case["required_paths"] if path not in observed_paths]
    if missing_paths:
        failures.append(f"required paths were not observed: {missing_paths}")
    if len(result["tools"]) > case["max_exact_tool_calls"]:
        failures.append(
            f"used {len(result['tools'])} tools; maximum is {case['max_exact_tool_calls']}"
        )
    activity = [
        value for value in (result["first_tool_ms"], result["first_text_ms"]) if value is not None
    ]
    first_activity_ms = min(activity) if activity else None
    if first_activity_ms is None or first_activity_ms > case["max_first_activity_ms"]:
        failures.append(
            f"first activity arrived in {first_activity_ms!r} ms; maximum is "
            f"{case['max_first_activity_ms']} ms"
        )
    if result["outcome"] != "completed":
        failures.append(f"operation outcome was {result['outcome']!r}")
    if result["settled_ms"] > case["max_settled_ms"]:
        failures.append(
            f"settled in {result['settled_ms']} ms; maximum is {case['max_settled_ms']} ms"
        )
    if not result["repository_unchanged"]:
        failures.append("read-only question changed repository state")
    return not failures, failures


def run_case(daemon: str, environment: dict[str, str], repository: Path, case: dict) -> dict:
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
        result = run_operation(process, reader, repository, 1, case["question"])
        send(process, {"method": "shutdown"})
        return result
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def main() -> int:
    pi = require_pi()
    daemon = require_executable(
        "LANTERN_DAEMON_BIN", PROJECT_ROOT / "target" / "debug" / "lantern-daemon"
    )
    dataset_bytes = DATASET_PATH.read_bytes()
    dataset = json.loads(dataset_bytes)
    environment = os.environ.copy()
    environment["LANTERN_PI_BIN"] = pi
    environment["LANTERN_PI_MODEL"] = MODEL
    results = []
    passed = True
    selected_case = os.environ.get("LANTERN_EVAL_CASE")
    cases = [
        case for case in dataset["cases"] if selected_case is None or case["id"] == selected_case
    ]
    if selected_case is not None and not cases:
        raise RuntimeError(f"unknown LANTERN_EVAL_CASE: {selected_case}")
    for case in cases:
        print(f"running {case['id']}", flush=True)
        repository = require_repository(case)
        status = repository_status(repository)
        try:
            result = run_case(daemon, environment, repository, case)
        except RuntimeError as cause:
            if str(cause) != "Lantern live trace timed out":
                raise
            result = {
                "answer": "",
                "tools": [],
                "evidence": [],
                "first_evidence_ms": None,
                "first_grounding_state_ms": None,
                "first_tool_ms": None,
                "first_text_ms": None,
                "provider_wait_after_evidence_ms": None,
                "settled_ms": case["max_settled_ms"],
                "outcome": "timed_out",
                "cancellation_latency_ms": None,
            }
        result["repository_unchanged"] = repository_status(repository) == status
        case_passed, failures = evaluate(result, case)
        result.update({"passed": case_passed, "failures": failures})
        results.append(
            {
                "case_id": case["id"],
                "repository": case["repository"],
                "revision": case["revision"],
                "run": result,
            }
        )
        passed = passed and case_passed

    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    report = {
        "dataset": dataset["dataset"],
        "dataset_version": dataset["version"],
        "dataset_sha256": hashlib.sha256(dataset_bytes).hexdigest(),
        "lantern_revision": git_revision(),
        "driver": "lantern-protocol-v15-hybrid-retrieval",
        "pi_version": PI_VERSION,
        "provider": "openai-codex",
        "model": MODEL,
        "generated_at": timestamp,
        "passed": passed,
        "results": results,
    }
    report_dir = ROOT / "reports"
    report_dir.mkdir(exist_ok=True)
    report_path = report_dir / f"semantic-retrieval-{timestamp}.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(report_path)
    for item in results:
        run = item["run"]
        status = "PASS" if run["passed"] else "FAIL"
        detail = "; ".join(run["failures"]) or f"settled in {run['settled_ms']} ms"
        print(f"{item['case_id']}: {status} — {detail}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
