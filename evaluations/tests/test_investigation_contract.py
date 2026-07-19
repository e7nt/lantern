import json
from pathlib import Path

import pytest
from deepeval.test_case import LLMTestCase

from metrics.contract import InvestigationBriefContractMetric


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "investigation" / "v1.json"


@pytest.fixture(scope="module")
def cases() -> list[dict]:
    dataset = json.loads(DATASET_PATH.read_text(encoding="utf-8"))
    assert dataset["dataset"] == "investigation"
    assert dataset["version"] == 1
    return dataset["cases"]


def complete_brief(case: dict) -> str:
    facts = "\n".join(f"- {fact}" for fact in case["required_facts"])
    return f"""Goal
{case["objective"]}
Observed
{facts}
Affected flow
request to runtime
Likely changes
configuration boundary
Open questions
- Open question: configuration authority
Acceptance criteria
- behavior is explicit
Exclusions
- implementation
Risks
- stale configuration
Readiness
Blocked
"""


def test_contract_accepts_a_grounded_explicit_brief(cases: list[dict]) -> None:
    for case in cases:
        metric = InvestigationBriefContractMetric(case["required_facts"], case["forbidden"])
        score = metric.measure(
            LLMTestCase(input=case["objective"], actual_output=complete_brief(case))
        )
        assert score == 1.0, metric.reason


def test_contract_rejects_missing_structure_and_false_completion(cases: list[dict]) -> None:
    case = cases[0]
    metric = InvestigationBriefContractMetric(case["required_facts"], case["forbidden"])
    score = metric.measure(
        LLMTestCase(
            input=case["objective"],
            actual_output="Implementation is ready. Tests passed. No risks.",
        )
    )
    assert score == 0.0
    assert "missing headings" in metric.reason
