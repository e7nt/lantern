import json
from pathlib import Path

from deepeval.test_case import LLMTestCase

from metrics.contract import CodeReviewContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "code_review" / "v1.json"


def test_one_result_addresses_the_complete_review() -> None:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "code_review"
    assert dataset["version"] == 1
    case = dataset["cases"][0]
    output = (
        "Updated the implementation to return three while preserving the existing signature. "
        "The focused test passed."
    )
    metric = CodeReviewContractMetric(case["required"], case["forbidden"])
    assert metric.measure(LLMTestCase(input=str(case["comments"]), actual_output=output)) == 1.0


def test_partial_correction_fails_the_review_contract() -> None:
    metric = CodeReviewContractMetric(
        ["return three", "signature", "focused test"],
        ["all tests passed"],
    )
    score = metric.measure(
        LLMTestCase(
            input="Address both comments",
            actual_output="Changed the code to return three. The focused test passed.",
        )
    )
    assert score == 0.0
    assert "unaddressed review outcomes" in metric.reason


def test_correction_cannot_invent_broader_verification() -> None:
    metric = CodeReviewContractMetric(
        ["return three", "signature", "focused test"],
        ["all tests passed"],
    )
    score = metric.measure(
        LLMTestCase(
            input="Focused verification only",
            actual_output=(
                "Updated the code to return three, preserved the signature, and ran the focused "
                "test; all tests passed."
            ),
        )
    )
    assert score == 0.0
