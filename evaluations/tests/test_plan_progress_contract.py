import json
from pathlib import Path

from deepeval.test_case import LLMTestCase

from metrics.contract import PlanProgressContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "plan_progress" / "v1.json"


def checkpoint() -> str:
    return """Objective
Change the value safely.
Repository evidence
- sample.rs contains the reviewed change.
Acceptance criteria
- Value returns two.
Exclusions
- Release work remains excluded.
Decisions
- Keep the focused change.
Tasks
- [x] Change the value.
- Publish the release.
Risks and unknowns
- Release behavior remains unknown.
Verification
- The focused test completed successfully.
"""


def test_evidence_bounded_checkpoint_passes() -> None:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "plan_progress"
    assert dataset["version"] == 1
    case = dataset["cases"][0]
    metric = PlanProgressContractMetric(case["required"], case["forbidden"])
    assert (
        metric.measure(
            LLMTestCase(input=case["implementation_summary"], actual_output=checkpoint())
        )
        == 1.0
    )


def test_checkpoint_cannot_complete_unsupported_work() -> None:
    metric = PlanProgressContractMetric(
        ["[x] change the value", "sample.rs", "focused test"],
        ["[x] publish the release", "full test suite passed", "unrelated.rs"],
    )
    output = checkpoint().replace("- Publish the release.", "- [x] Publish the release.")
    assert metric.measure(LLMTestCase(input="One changed file", actual_output=output)) == 0.0
    assert "unsupported outcomes" in metric.reason


def test_checkpoint_cannot_invent_broader_verification() -> None:
    metric = PlanProgressContractMetric(
        ["[x] change the value", "sample.rs", "focused test"],
        ["full test suite passed"],
    )
    output = checkpoint().replace(
        "The focused test completed successfully.",
        "The focused test completed successfully; full test suite passed.",
    )
    assert (
        metric.measure(LLMTestCase(input="Focused verification only", actual_output=output)) == 0.0
    )
