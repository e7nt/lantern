import json
from pathlib import Path

import pytest

from run_retrieval_baseline import comparison, evaluate_result


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "retrieval_baseline" / "v1.json"


@pytest.fixture(scope="module")
def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "retrieval_baseline"
    assert dataset["version"] == 1
    return dataset["cases"]


def result_for(case: dict, *, mode: str) -> dict:
    evidence = []
    tools = [{"tool": "read", "relative_path": path} for path in case["required_paths"]]
    if mode == "lsp":
        evidence = [
            {"source": "selection", "relative_path": case["required_paths"][0]},
            {"source": "definition", "relative_path": case["required_paths"][1]},
        ]
        tools = []
    return {
        "answer": " ".join(case["required_answer_terms"]),
        "tools": tools,
        "evidence": evidence,
        "first_tool_ms": 20 if tools else None,
        "first_text_ms": 100,
        "settled_ms": 150,
        "outcome": "completed",
        "repository_unchanged": True,
    }


def test_dataset_pins_distinct_external_repositories(cases: list[dict]) -> None:
    assert len({case["id"] for case in cases}) == len(cases)
    assert len({case["repository"] for case in cases}) == len(cases)
    for case in cases:
        assert len(case["revision"]) == 40
        assert not case["repository"].startswith("/")
        assert case["required_paths"]


@pytest.mark.parametrize("mode", ["exact", "lsp"])
def test_retrieval_contract_accepts_grounded_read_only_runs(cases: list[dict], mode: str) -> None:
    for case in cases:
        passed, failures = evaluate_result(result_for(case, mode=mode), case, mode)
        assert passed, failures


def test_retrieval_contract_rejects_missing_definition_evidence(cases: list[dict]) -> None:
    case = cases[0]
    result = result_for(case, mode="lsp")
    result["evidence"] = [result["evidence"][0]]
    passed, failures = evaluate_result(result, case, "lsp")
    assert not passed
    assert any("definition evidence" in failure for failure in failures)


def test_retrieval_contract_rejects_repository_mutation(cases: list[dict]) -> None:
    case = cases[0]
    result = result_for(case, mode="exact")
    result["repository_unchanged"] = False
    passed, failures = evaluate_result(result, case, "exact")
    assert not passed
    assert any("changed repository state" in failure for failure in failures)


def test_comparison_reports_lsp_minus_exact() -> None:
    exact = {"tools": [1, 2, 3], "first_tool_ms": 100, "first_text_ms": 500, "settled_ms": 900}
    lsp = {"tools": [1], "first_tool_ms": None, "first_text_ms": 300, "settled_ms": 600}
    assert comparison(exact, lsp) == {
        "tool_call_delta": -2,
        "first_tool_ms_delta": None,
        "first_text_ms_delta": -200,
        "settled_ms_delta": -300,
    }
