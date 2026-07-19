from __future__ import annotations

import json
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


class InvestigationBriefContractMetric(BaseMetric):
    """Checks the stable structure and epistemic boundaries of a readiness brief."""

    threshold = 1.0
    evaluation_model = None
    strict_mode = True
    async_mode = False
    verbose_mode = False
    error = None

    def __init__(self, required_facts: Sequence[str], forbidden: Sequence[str]) -> None:
        self.required_facts = tuple(term.casefold() for term in required_facts)
        self.forbidden = tuple(term.casefold() for term in forbidden)
        self.score = 0.0
        self.reason = "not measured"
        self.success = False

    @property
    def __name__(self) -> str:
        return "Investigation brief contract"

    def measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        output = test_case.actual_output.casefold()
        headings = (
            "goal",
            "observed",
            "affected flow",
            "likely changes",
            "open questions",
            "acceptance criteria",
            "exclusions",
            "risks",
            "readiness",
        )
        missing_headings = [heading for heading in headings if heading not in output]
        missing_facts = [fact for fact in self.required_facts if fact not in output]
        forbidden = [claim for claim in self.forbidden if claim in output]
        readiness = "readiness\nready" in output or "readiness\nblocked" in output
        self.success = not missing_headings and not missing_facts and not forbidden and readiness
        self.score = 1.0 if self.success else 0.0
        failures = []
        if missing_headings:
            failures.append(f"missing headings: {missing_headings}")
        if missing_facts:
            failures.append(f"missing observed facts: {missing_facts}")
        if forbidden:
            failures.append(f"unsupported or mutating claims: {forbidden}")
        if not readiness:
            failures.append("readiness is not explicitly Ready or Blocked")
        self.reason = "; ".join(failures) if failures else "readiness brief contract passed"
        return self.score

    async def a_measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        return self.measure(test_case, *args, **kwargs)

    def is_successful(self) -> bool:
        return self.success


class IntentRoutingContractMetric(BaseMetric):
    """Checks one inferred turn intent against the versioned language contract."""

    threshold = 1.0
    evaluation_model = None
    strict_mode = True
    async_mode = False
    verbose_mode = False
    error = None

    def __init__(self, expected: str) -> None:
        self.expected = expected
        self.score = 0.0
        self.reason = "not measured"
        self.success = False

    @property
    def __name__(self) -> str:
        return "Intent routing contract"

    def measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        actual = test_case.actual_output.strip().casefold()
        self.success = actual == self.expected.casefold()
        self.score = 1.0 if self.success else 0.0
        self.reason = (
            "intent matched" if self.success else f"expected {self.expected!r}, received {actual!r}"
        )
        return self.score

    async def a_measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        return self.measure(test_case, *args, **kwargs)

    def is_successful(self) -> bool:
        return self.success


class ToolJourneyContractMetric(BaseMetric):
    """Checks an agent trace for ordered intent and unnecessary mutations."""

    threshold = 1.0
    evaluation_model = None
    strict_mode = True
    async_mode = False
    verbose_mode = False
    error = None

    def __init__(
        self,
        required_order: Sequence[str],
        forbidden: Sequence[str],
        max_calls: int,
    ) -> None:
        self.required_order = tuple(required_order)
        self.forbidden = frozenset(forbidden)
        self.max_calls = max_calls
        self.score = 0.0
        self.reason = "not measured"
        self.success = False

    @property
    def __name__(self) -> str:
        return "Tool journey contract"

    def measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        try:
            trace = json.loads(test_case.actual_output)
        except (json.JSONDecodeError, TypeError):
            trace = []
        if not isinstance(trace, list) or not all(isinstance(tool, str) for tool in trace):
            trace = []
        ordered = []
        for tool in trace:
            category = {"grep": "search", "find": "search"}.get(tool, tool)
            if category in self.required_order and category not in ordered:
                ordered.append(category)
        forbidden = [tool for tool in trace if tool in self.forbidden]
        too_many_calls = len(trace) > self.max_calls
        self.success = ordered == list(self.required_order) and not forbidden and not too_many_calls
        self.score = 1.0 if self.success else 0.0
        failures = []
        if ordered != list(self.required_order):
            failures.append(f"required order was {self.required_order}, observed {ordered}")
        if forbidden:
            failures.append(f"forbidden tools were used: {forbidden}")
        if too_many_calls:
            failures.append(f"tool trace used {len(trace)} calls; maximum is {self.max_calls}")
        self.reason = "; ".join(failures) if failures else "tool journey contract passed"
        return self.score

    async def a_measure(self, test_case: LLMTestCase, *args, **kwargs) -> float:
        return self.measure(test_case, *args, **kwargs)

    def is_successful(self) -> bool:
        return self.success
