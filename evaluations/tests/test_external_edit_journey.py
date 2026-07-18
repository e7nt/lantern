import json
from pathlib import Path

from run_external_edit_journey import evaluate_edit, evaluate_interruption


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "external_edit" / "v1.json"


def contract() -> dict:
    return json.loads(DATASET_PATH.read_text(encoding="utf-8"))


def edit_result() -> dict:
    return {
        "tools": [
            {"tool": "read", "relative_path": "src/status.js"},
            {"tool": "edit", "relative_path": "src/status.js"},
            {"tool": "edit", "relative_path": "status.test.mjs"},
            {"tool": "bash", "relative_path": None},
        ],
        "evidence": [{"source": "call", "relative_path": "src/status.js"}],
        "outcome": "completed",
        "first_tool_ms": 100,
        "settled_ms": 500,
    }


def edit_state() -> dict:
    return {
        "files_match": True,
        "focused_test_passed": True,
        "changed_paths": ["src/status.js", "status.test.mjs"],
        "staged_paths": [],
    }


def test_external_edit_contract_accepts_focused_reviewable_change() -> None:
    passed, failures = evaluate_edit(edit_result(), edit_state(), contract())
    assert passed, failures


def test_external_edit_contract_rejects_missing_verification_and_call_evidence() -> None:
    result = edit_result()
    result["tools"] = result["tools"][:-1]
    result["evidence"] = []
    passed, failures = evaluate_edit(result, edit_state(), contract())
    assert not passed
    assert any("bash" in failure for failure in failures)
    assert any("call evidence" in failure for failure in failures)


def test_interruption_contract_requires_fast_clean_settlement() -> None:
    result = {
        "outcome": "cancelled",
        "cancellation_latency_ms": 20,
        "post_cancel_settled_ms": 40,
    }
    passed, failures = evaluate_interruption(result, True, contract())
    assert passed, failures
    result["post_cancel_settled_ms"] = 1001
    passed, failures = evaluate_interruption(result, False, contract())
    assert not passed
    assert len(failures) == 2
