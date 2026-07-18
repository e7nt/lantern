import json
import time
from io import StringIO
from pathlib import Path

import pytest

from run_live_trace import (
    EventReader,
    evaluate_explanation,
    evaluate_interruption,
    snapshot_repository,
    write_fixture,
)


DATASET_PATH = Path(__file__).parents[1] / "datasets" / "live_trace" / "v1.json"


@pytest.fixture(scope="module")
def dataset() -> dict:
    return json.loads(DATASET_PATH.read_text(encoding="utf-8"))


def test_live_trace_dataset_has_stable_cases(dataset: dict) -> None:
    assert dataset["dataset"] == "live_trace"
    assert dataset["version"] == 1
    assert dataset["explanation"]["id"] != dataset["interruption"]["id"]
    assert dataset["fixture"]


def test_fixture_rejects_repository_escape(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="repository-relative"):
        write_fixture(tmp_path, {"../private": "no"})


def test_repository_snapshot_detects_added_and_changed_files(tmp_path: Path) -> None:
    write_fixture(tmp_path, {"src/lib.rs": "before"})
    before = snapshot_repository(tmp_path)
    write_fixture(tmp_path, {"src/lib.rs": "after", "src/new.rs": "added"})
    assert snapshot_repository(tmp_path) != before


def test_event_reader_preserves_multiple_buffered_frames() -> None:
    reader = EventReader(StringIO('{"type":"first"}\n{"type":"second"}\n'))
    deadline = time.monotonic() + 1
    assert reader.next(deadline)["type"] == "first"
    assert reader.next(deadline)["type"] == "second"


def test_explanation_contract_checks_grounding_tools_and_latency(dataset: dict) -> None:
    result = {
        "answer": "Request admission is in src/admission.rs and failures become a 401.",
        "tools": [
            {"tool": "grep", "relative_path": None},
            {"tool": "read", "relative_path": "src/admission.rs"},
        ],
        "settled_ms": 1200,
        "outcome": "completed",
        "fixture_unchanged": True,
    }
    passed, failures = evaluate_explanation(result, dataset["explanation"])
    assert passed, failures


def test_explanation_contract_rejects_uninspected_evidence(dataset: dict) -> None:
    result = {
        "answer": "Admission failures become a 401.",
        "tools": [{"tool": "read", "relative_path": "src/server.rs"}],
        "settled_ms": 1200,
        "outcome": "completed",
        "fixture_unchanged": True,
    }
    passed, failures = evaluate_explanation(result, dataset["explanation"])
    assert not passed
    assert any("evidence path" in failure for failure in failures)


def test_explanation_contract_requires_every_grounded_term(dataset: dict) -> None:
    result = {
        "answer": "Request admission is implemented in src/admission.rs.",
        "tools": [{"tool": "read", "relative_path": "src/admission.rs"}],
        "settled_ms": 1200,
        "outcome": "completed",
        "fixture_unchanged": True,
    }
    passed, failures = evaluate_explanation(result, dataset["explanation"])
    assert not passed
    assert any("required grounded terms" in failure for failure in failures)


def test_interruption_contract_requires_bounded_cancel_and_settle(dataset: dict) -> None:
    passed, failures = evaluate_interruption(
        {
            "outcome": "cancelled",
            "cancellation_latency_ms": 35,
            "settled_ms": 180,
            "post_cancel_settled_ms": 40,
            "fixture_unchanged": True,
        },
        dataset["interruption"],
    )
    assert passed, failures


def test_interruption_contract_rejects_slow_cancellation(dataset: dict) -> None:
    passed, failures = evaluate_interruption(
        {
            "outcome": "cancelled",
            "cancellation_latency_ms": 700,
            "settled_ms": 800,
            "post_cancel_settled_ms": 710,
            "fixture_unchanged": True,
        },
        dataset["interruption"],
    )
    assert not passed
    assert any("cancellation latency" in failure for failure in failures)
