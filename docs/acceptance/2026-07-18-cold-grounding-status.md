# Cold grounding status — 2026-07-18

Status: accepted.

Protocol v9 adds one bounded `grounding_state` event with two valid states:
`preparing_index` and `repository_search_only`. The terminal renders them as
`Preparing code understanding…` and `Searching the repository…`. Both are
transient activity-line text; neither enters the conversation transcript or
claims a percentage.

The daemon emits the state after its semantic query returns an explicit
non-ready state and before Pi initialization. Questions remain admitted and do
not wait for index construction. A ready index emits no redundant grounding
state because its verified evidence already drives `Found relevant code ·
thinking…`.

The reproducible `evaluations/run_cold_grounding_status.py` journey cloned
pinned Requests revision `f361ead047be5cb873174218582f7d8b9fcd9f49` into a
disposable directory, opened it with the real local embedding worker and a
fresh path-keyed index, then submitted a repository question.

| Measurement | Result |
| --- | ---: |
| Typed state | `preparing_index` |
| Submission to visible state | 1 ms |
| Visibility ceiling | 1,000 ms |
| Provider credential required | no |

The evaluation stops after the lifecycle signal because answer quality and
provider latency are covered by the semantic-retrieval and live-trace suites.
The worker and disposable repository are terminated and removed afterward.
