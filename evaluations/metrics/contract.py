from __future__ import annotations

from collections.abc import Sequence

from deepeval.metrics import BaseMetric
from deepeval.test_case import LLMTestCase


class SelectionAnswerContractMetric(BaseMetric):
    """Deterministic guardrail metric for a single selection-grounded answer."""

    threshold = 1.0
    evaluation_model = None
    strict_mode = True
    async_mode = False
    verbose_mode = False
    error = None

    def __init__(self, required_any: Sequence[str], forbidden: Sequence[str]) -> None:
        self.required_any = tuple(term.casefold() for term in required_any)
        self.forbidden = tuple(term.casefold() for term in forbidden)
        self.score = 0.0
        self.reason = "not measured"
        self.success = False

    @property
    def __name__(self) -> str:
        return "Selection answer contract"

    def measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        output = test_case.actual_output.casefold()
        missing_required = not any(term in output for term in self.required_any)
        present_forbidden = [term for term in self.forbidden if term in output]
        self.success = not missing_required and not present_forbidden
        self.score = 1.0 if self.success else 0.0
        failures = []
        if missing_required:
            failures.append("no required disclosure was present")
        if present_forbidden:
            failures.append(f"forbidden claims were present: {present_forbidden}")
        self.reason = "; ".join(failures) if failures else "all deterministic constraints passed"
        return self.score

    async def a_measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        return self.measure(test_case, *args, **kwargs)

    def is_successful(self) -> bool:
        return self.success
