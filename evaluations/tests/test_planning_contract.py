import json
from pathlib import Path

from deepeval.test_case import LLMTestCase

from metrics.contract import PlanningArtifactContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "planning" / "v1.json"


def test_complete_grounded_plan_passes_the_persistence_gate() -> None:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "planning"
    assert dataset["version"] == 1
    case = dataset["cases"][0]
    evidence = "\n".join(case["required_evidence"])
    output = f"""Objective
{case["objective"]}
Repository evidence
{evidence}
Acceptance criteria
- One active plan opens in Helix.
Exclusions
- No task dashboard.
Decisions
- Create new; never overwrite.
Tasks
- Validate and serialize the plan.
Risks and unknowns
- Existing active plan.
Verification
- Prove duplicate rejection.
"""
    metric = PlanningArtifactContractMetric(case["required_evidence"])
    assert metric.measure(LLMTestCase(input=case["objective"], actual_output=output)) == 1.0


def test_incomplete_plan_is_not_persistable() -> None:
    metric = PlanningArtifactContractMetric(["apps/daemon/src/main.rs"])
    score = metric.measure(
        LLMTestCase(input="Persist this", actual_output="Objective\nSave the plan")
    )
    assert score == 0.0
    assert "missing headings" in metric.reason
