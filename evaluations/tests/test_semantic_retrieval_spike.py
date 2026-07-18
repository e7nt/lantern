import json
import os
from pathlib import Path
from unittest.mock import patch

import pytest

from run_semantic_retrieval_spike import evaluate, main


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "semantic_retrieval" / "v1.json"


def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "semantic_retrieval"
    assert dataset["version"] == 1
    return dataset["cases"]


def passing_result(case: dict) -> dict:
    return {
        "answer": " ".join(case["required_answer_terms"]),
        "tools": [{"tool": "read", "relative_path": path} for path in case["required_paths"]],
        "evidence": [],
        "first_tool_ms": 100,
        "first_text_ms": 500,
        "settled_ms": 900,
        "outcome": "completed",
        "repository_unchanged": True,
    }


def test_dataset_uses_vocabulary_mismatches_across_three_languages() -> None:
    dataset_cases = cases()
    assert len(dataset_cases) == 3
    assert len({case["repository"] for case in dataset_cases}) == 3
    for case in dataset_cases:
        assert case["remote"].startswith("https://github.com/")
        assert len(case["revision"]) == 40
        for term in case["required_answer_terms"]:
            assert term.casefold() not in case["question"].casefold()


def test_contract_accepts_grounded_fast_read_only_result() -> None:
    for case in cases():
        passed, failures = evaluate(passing_result(case), case)
        assert passed, failures


def test_contract_rejects_slow_unbounded_or_ungrounded_result() -> None:
    case = cases()[0]
    result = passing_result(case)
    result["answer"] = "A plausible conceptual answer"
    result["tools"] = result["tools"] * 9
    result["first_tool_ms"] = 3100
    result["first_text_ms"] = 4000
    result["repository_unchanged"] = False
    passed, failures = evaluate(result, case)
    assert not passed
    assert len(failures) == 4


def test_runner_rejects_an_unknown_explicit_case() -> None:
    with (
        patch.dict(os.environ, {"LANTERN_EVAL_CASE": "missing-case"}),
        patch("run_semantic_retrieval_spike.require_pi", return_value="pi"),
        patch("run_semantic_retrieval_spike.require_executable", return_value="daemon"),
        pytest.raises(RuntimeError, match="unknown LANTERN_EVAL_CASE"),
    ):
        main()
