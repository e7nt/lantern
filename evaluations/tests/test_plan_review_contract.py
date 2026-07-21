import json
from pathlib import Path

from deepeval.test_case import LLMTestCase

from metrics.contract import PlanReviewContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "plan_review" / "v1.json"


def revised_plan() -> str:
    return """Objective
Answer warm code questions in under three seconds.
Repository evidence
- apps/daemon/src/main.rs
Acceptance criteria
- Measure warm response latency.
Exclusions
- The second task remains outside the first release.
Decisions
- Keep one bounded path.
Tasks
- Measure the first task.
Risks and unknowns
- Provider latency varies.
Verification
- Run the live latency trace.
"""


def test_one_complete_revision_addresses_every_comment() -> None:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "plan_review"
    assert dataset["version"] == 1
    case = dataset["cases"][0]
    metric = PlanReviewContractMetric(case["required_outcomes"])
    assert (
        metric.measure(LLMTestCase(input=str(case["comments"]), actual_output=revised_plan()))
        == 1.0
    )


def test_partial_revision_fails_the_review_contract() -> None:
    metric = PlanReviewContractMetric(["under three seconds", "outside the first release"])
    score = metric.measure(
        LLMTestCase(
            input="Address both comments",
            actual_output=revised_plan().replace("outside the first release", "deferred"),
        )
    )
    assert score == 0.0
    assert "unaddressed comments" in metric.reason


def test_model_wrappers_and_implementation_claims_are_rejected() -> None:
    metric = PlanReviewContractMetric(["under three seconds", "outside the first release"])
    score = metric.measure(
        LLMTestCase(
            input="Revise only", actual_output=f"```markdown\n{revised_plan()}\nImplemented\n```"
        )
    )
    assert score == 0.0
    assert "forbidden wrapper or claim" in metric.reason
