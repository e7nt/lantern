import json
from pathlib import Path

import pytest
from deepeval.test_case import LLMTestCase

from metrics.contract import IntentRoutingContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "intent_routing" / "v1.json"


@pytest.fixture(scope="module")
def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "intent_routing"
    assert dataset["version"] == 1
    return dataset["cases"]


def test_contract_accepts_the_expected_typed_intents(cases: list[dict]) -> None:
    for case in cases:
        metric = IntentRoutingContractMetric(case["expected"])
        score = metric.measure(LLMTestCase(input=case["query"], actual_output=case["expected"]))
        assert score == 1.0, metric.reason


def test_contract_rejects_mutation_for_an_ambiguous_request(cases: list[dict]) -> None:
    case = next(item for item in cases if item["id"] == "ambiguous-default")
    metric = IntentRoutingContractMetric(case["expected"])
    score = metric.measure(LLMTestCase(input=case["query"], actual_output="implement"))
    assert score == 0.0
    assert "expected" in metric.reason


def test_repository_text_is_never_an_intent_input(cases: list[dict]) -> None:
    assert all("repository_text" in case for case in cases)
    assert any("edit" in case["repository_text"].casefold() for case in cases)
