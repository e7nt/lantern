from __future__ import annotations

import hashlib
import json
import os
import subprocess
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
)


ROOT = Path(__file__).parent
DATASET_PATH = ROOT / "datasets" / "retrieval_baseline" / "v2.json"


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
            "prepare the pinned reference repositories first"
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


def evaluate_result(result: dict, case: dict, mode: str) -> tuple[bool, list[str]]:
    failures = []
    answer = result["answer"].casefold()
    missing_terms = [
        term for term in case["required_answer_terms"] if term.casefold() not in answer
    ]
    if missing_terms:
        failures.append(f"answer omitted required grounded terms: {missing_terms}")

    trace = [tool["tool"] for tool in result["tools"]]
    metric = ToolJourneyContractMetric(
        required_order=[],
        forbidden=["edit", "write"],
        max_calls=(case["max_lsp_tool_calls"] if mode == "lsp" else case["max_tool_calls"]),
    )
    metric.measure(LLMTestCase(input=case["question"], actual_output=json.dumps(trace)))
    if not metric.is_successful():
        failures.append(metric.reason)
    if mode == "lsp" and len(result["tools"]) < case["min_lsp_tool_calls"]:
        failures.append(
            f"LSP operation used {len(result['tools'])} tools; minimum is "
            f"{case['min_lsp_tool_calls']}"
        )

    observed_paths = {
        item["relative_path"]
        for item in [*result["tools"], *result["evidence"]]
        if item["relative_path"] is not None
    }
    missing_paths = [path for path in case["required_paths"] if path not in observed_paths]
    if missing_paths:
        failures.append(f"required evidence paths were not observed: {missing_paths}")
    if mode == "lsp" and not any(item["source"] == "definition" for item in result["evidence"]):
        failures.append("LSP-assisted operation emitted no definition evidence")
    if mode == "lsp":
        first_text_ms = result["first_text_ms"]
        if first_text_ms is None or first_text_ms > case["max_lsp_first_text_ms"]:
            failures.append(
                f"LSP first text arrived in {first_text_ms!r} ms; maximum is "
                f"{case['max_lsp_first_text_ms']} ms"
            )
        if "max_lsp_first_activity_ms" in case:
            activity_times = [
                value
                for value in (result["first_tool_ms"], result["first_text_ms"])
                if value is not None
            ]
            first_activity_ms = min(activity_times) if activity_times else None
            if first_activity_ms is None or first_activity_ms > case["max_lsp_first_activity_ms"]:
                failures.append(
                    f"LSP first activity arrived in {first_activity_ms!r} ms; maximum is "
                    f"{case['max_lsp_first_activity_ms']} ms"
                )
    if result["outcome"] != "completed":
        failures.append(f"operation outcome was {result['outcome']!r}, expected 'completed'")
    if result["settled_ms"] > case["max_settled_ms"]:
        failures.append(
            f"operation settled in {result['settled_ms']} ms; maximum is "
            f"{case['max_settled_ms']} ms"
        )
    if not result["repository_unchanged"]:
        failures.append("read-only retrieval operation changed repository state")
    return not failures, failures


def comparison(exact: dict, lsp: dict) -> dict:
    def delta(field: str) -> int | None:
        left = exact[field]
        right = lsp[field]
        return None if left is None or right is None else right - left

    return {
        "tool_call_delta": len(lsp["tools"]) - len(exact["tools"]),
        "first_tool_ms_delta": delta("first_tool_ms"),
        "first_text_ms_delta": delta("first_text_ms"),
        "settled_ms_delta": delta("settled_ms"),
    }


def run_isolated_mode(
    daemon: str,
    environment: dict[str, str],
    repository: Path,
    case: dict,
    mode: str,
) -> dict:
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
        request = {
            "method": "ask_agent" if mode == "exact" else "ask_agent_symbol",
            "id": 1,
            "repository": str(repository),
            "query": case["question"],
        }
        if mode == "lsp":
            request["context"] = case["context"]
        result = run_operation(
            process,
            reader,
            repository,
            1,
            case["question"],
            request=request,
        )
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
    for case in dataset["cases"]:
        repository = require_repository(case)
        initial_status = repository_status(repository)
        modes = []
        for mode in ("exact", "lsp"):
            result = run_isolated_mode(daemon, environment, repository, case, mode)
            result["repository_unchanged"] = repository_status(repository) == initial_status
            mode_passed, failures = evaluate_result(result, case, mode)
            result.update({"mode": mode, "passed": mode_passed, "failures": failures})
            modes.append(result)
            passed = passed and mode_passed
        results.append(
            {
                "case_id": case["id"],
                "repository": case["repository"],
                "revision": case["revision"],
                "runs": modes,
                "comparison": comparison(modes[0], modes[1]),
            }
        )

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
        "results": results,
    }
    report_dir = ROOT / "reports"
    report_dir.mkdir(exist_ok=True)
    report_path = report_dir / f"retrieval-baseline-{timestamp}.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(report_path)
    for case in results:
        print(case["case_id"])
        for run in case["runs"]:
            status = "PASS" if run["passed"] else "FAIL"
            detail = "; ".join(run["failures"]) or f"settled in {run['settled_ms']} ms"
            print(f"  {run['mode']}: {status} — {detail}")
        print(f"  delta (LSP - exact): {case['comparison']}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
