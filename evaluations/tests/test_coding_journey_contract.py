import json
from pathlib import Path

import pytest
from deepeval.test_case import LLMTestCase

from metrics.contract import ToolJourneyContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "coding_journey" / "v1.json"


@pytest.fixture(scope="module")
def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "coding_journey"
    assert dataset["version"] == 1
    return dataset["cases"]


def test_every_journey_has_a_stable_contract(cases: list[dict]) -> None:
    assert len({case["id"] for case in cases}) == len(cases)
    for case in cases:
        assert case["request"].strip()
        assert case["required_order"]


@pytest.mark.parametrize(
    ("case_id", "trace"),
    [
        ("explain-before-editing", ["grep", "read"]),
        ("focused-change", ["grep", "read", "edit", "bash"]),
        ("inspect-git-state", ["bash"]),
    ],
)
def test_metric_accepts_efficient_tool_journeys(
    cases: list[dict], case_id: str, trace: list[str]
) -> None:
    case = next(case for case in cases if case["id"] == case_id)
    metric = ToolJourneyContractMetric(case["required_order"], case["forbidden"])
    score = metric.measure(LLMTestCase(input=case["request"], actual_output=json.dumps(trace)))
    assert score == 1.0, metric.reason


def test_metric_rejects_editing_during_an_explanation(cases: list[dict]) -> None:
    case = next(case for case in cases if case["id"] == "explain-before-editing")
    metric = ToolJourneyContractMetric(case["required_order"], case["forbidden"])
    score = metric.measure(
        LLMTestCase(input=case["request"], actual_output='["grep", "edit", "read"]')
    )
    assert score == 0.0
    assert "forbidden" in metric.reason
