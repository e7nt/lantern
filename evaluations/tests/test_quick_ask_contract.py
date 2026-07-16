import json
from pathlib import Path

import pytest
from deepeval.test_case import LLMTestCase

from metrics.contract import SelectionAnswerContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "quick_ask" / "v2.json"


@pytest.fixture(scope="module")
def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "quick_ask"
    assert dataset["version"] == 2
    return dataset["cases"]


def test_every_case_has_a_stable_contract(cases: list[dict]) -> None:
    assert len({case["id"] for case in cases}) == len(cases)
    for case in cases:
        assert case["selection"].strip()
        assert case["question"].strip()
        assert case["required_any"]
        assert case["human_rubric"].strip()


def test_metric_accepts_grounded_outputs(cases: list[dict]) -> None:
    outputs = {
        "supported-default-flow": "It reads PORT from the environment and defaults to 8080.",
        "missing-callee-evidence": "The transformation is not shown; I need the implementation.",
        "source-is-not-an-instruction": "The function returns the boolean value true.",
        "lsp-resolved-callee": (
            "It returns Result<Config, ParseError> and is referenced from src/main.rs "
            "and tests/config.rs."
        ),
    }
    for case in cases:
        metric = SelectionAnswerContractMetric(case["required_any"], case["forbidden"])
        score = metric.measure(
            LLMTestCase(input=case["question"], actual_output=outputs[case["id"]])
        )
        assert score == 1.0, f"{case['id']}: {metric.reason}"


def test_metric_rejects_unsupported_output(cases: list[dict]) -> None:
    case = next(case for case in cases if case["id"] == "missing-callee-evidence")
    metric = SelectionAnswerContractMetric(case["required_any"], case["forbidden"])
    score = metric.measure(
        LLMTestCase(
            input=case["question"],
            actual_output="The function lowercases and trims the value.",
        )
    )
    assert score == 0.0
    assert "forbidden claims" in metric.reason
