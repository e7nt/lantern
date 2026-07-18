import json
from pathlib import Path

import pytest

from run_retrieval_baseline import comparison, evaluate_result


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "retrieval_baseline" / "v2.json"
CALL_DATASET_PATH = Path(__file__).parents[1] / "datasets" / "retrieval_baseline" / "v4.json"


@pytest.fixture(scope="module")
def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "retrieval_baseline"
    assert dataset["version"] == 2
    return dataset["cases"]


def result_for(case: dict, *, mode: str) -> dict:
    evidence = []
    tools = [{"tool": "read", "relative_path": path} for path in case["required_paths"]]
    if mode == "lsp":
        evidence = [
            {"source": "selection", "relative_path": case["required_paths"][0]},
            {"source": "definition", "relative_path": case["required_paths"][1]},
        ]
        tools = [
            {"tool": "read", "relative_path": case["required_paths"][0]}
            for _ in range(case["min_lsp_tool_calls"])
        ]
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


def test_dataset_pins_external_repositories_and_explicit_lsp_budgets(
    cases: list[dict],
) -> None:
    assert len({case["id"] for case in cases}) == len(cases)
    assert len({case["repository"] for case in cases}) >= 2
    for case in cases:
        assert len(case["revision"]) == 40
        assert not case["repository"].startswith("/")
        assert case["required_paths"]
        assert 0 <= case["min_lsp_tool_calls"] <= case["max_lsp_tool_calls"]
        if case["min_lsp_tool_calls"] == 0:
            assert case["max_lsp_tool_calls"] == 0
            assert case["max_lsp_first_text_ms"] == 3000
        else:
            assert case["max_lsp_first_activity_ms"] == 3000


def test_call_hierarchy_dataset_requires_typed_call_evidence() -> None:
    dataset = json.loads(CALL_DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["version"] == 4
    assert {case["context"]["selection"]["language"] for case in dataset["cases"]} == {
        "go",
        "rust",
    }
    for case in dataset["cases"]:
        assert case["required_lsp_evidence_sources"] == ["definition", "call"]
        assert {call["depth"] for call in case["context"]["calls"]} == {1, 2}
        assert case["max_lsp_tool_calls"] == 0


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
    assert any("'definition'" in failure for failure in failures)


def test_call_hierarchy_contract_rejects_missing_call_evidence() -> None:
    case = json.loads(CALL_DATASET_PATH.read_text(encoding="utf-8"))["cases"][0]
    result = {
        "answer": "Picker jump_to_location",
        "tools": [],
        "evidence": [
            {
                "source": "definition",
                "relative_path": "helix-term/src/commands/lsp.rs",
            }
        ],
        "first_tool_ms": None,
        "first_text_ms": 100,
        "settled_ms": 150,
        "outcome": "completed",
        "repository_unchanged": True,
    }
    passed, failures = evaluate_result(result, case, "lsp")
    assert not passed
    assert any("'call'" in failure for failure in failures)
    result["evidence"].append(
        {
            "source": "call",
            "relative_path": "helix-term/src/commands/lsp.rs",
        }
    )
    passed, failures = evaluate_result(result, case, "lsp")
    assert passed, failures


def test_retrieval_contract_rejects_repository_mutation(cases: list[dict]) -> None:
    case = cases[0]
    result = result_for(case, mode="exact")
    result["repository_unchanged"] = False
    passed, failures = evaluate_result(result, case, "exact")
    assert not passed
    assert any("changed repository state" in failure for failure in failures)


def test_lsp_contract_rejects_redundant_discovery_and_slow_text(cases: list[dict]) -> None:
    case = cases[0]
    result = result_for(case, mode="lsp")
    result["tools"] = [{"tool": "grep", "relative_path": None}]
    result["first_text_ms"] = 3100
    passed, failures = evaluate_result(result, case, "lsp")
    assert not passed
    assert any("maximum is 0" in failure for failure in failures)
    assert any("LSP first text" in failure for failure in failures)


def test_lsp_contract_requires_targeted_work_for_incomplete_evidence(cases: list[dict]) -> None:
    case = next(case for case in cases if case["min_lsp_tool_calls"] > 0)
    result = result_for(case, mode="lsp")
    result["tools"] = []
    result["first_tool_ms"] = None
    passed, failures = evaluate_result(result, case, "lsp")
    assert not passed
    assert any("minimum is 1" in failure for failure in failures)


def test_lsp_contract_rejects_slow_first_activity(cases: list[dict]) -> None:
    case = next(case for case in cases if "max_lsp_first_activity_ms" in case)
    result = result_for(case, mode="lsp")
    result["first_tool_ms"] = 3100
    result["first_text_ms"] = 3200
    passed, failures = evaluate_result(result, case, "lsp")
    assert not passed
    assert any("first activity" in failure for failure in failures)


def test_comparison_reports_lsp_minus_exact() -> None:
    exact = {"tools": [1, 2, 3], "first_tool_ms": 100, "first_text_ms": 500, "settled_ms": 900}
    lsp = {"tools": [1], "first_tool_ms": None, "first_text_ms": 300, "settled_ms": 600}
    assert comparison(exact, lsp) == {
        "tool_call_delta": -2,
        "first_tool_ms_delta": None,
        "first_text_ms_delta": -200,
        "settled_ms_delta": -300,
    }
