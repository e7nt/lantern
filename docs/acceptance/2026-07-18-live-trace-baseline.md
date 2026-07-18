# Live trace baseline — 2026-07-18

Status: runner passed its offline contracts and completed three authenticated
Protocol v6 repetitions. The repository explanation remained grounded; the
efficiency gate exposed stochastic tool churn on one repetition.

## Grounded explanation

All repetitions inspected the curated admission implementation and returned
the correct missing-token-to-401 flow without modifying the fixture.

| Repetition | Tools | First tool | First text | Settled | Contract |
| --- | ---: | ---: | ---: | ---: | --- |
| 1 | 6 | 5,443 ms | 9,567 ms | 14,206 ms | pass |
| 2 | 8 | 7,594 ms | 15,476 ms | 20,406 ms | fail |
| 3 | 6 | 5,801 ms | 10,390 ms | 14,950 ms | pass |

The second trace repeated discovery after reading all three relevant files. Its
answer was still accurate, but the six-call efficiency ceiling correctly
failed. The ceiling was not relaxed to normalize the result.

## Interruption

Cancellation was sent when the first tool became visible. The three repetitions
reported 74 ms, 30 ms, and 75 ms cancellation latency. After the runner sent
cancel, the operations settled in 75 ms, 31 ms, and 76 ms respectively. Each
run confirmed that the repository fixture remained unchanged.

## Interpretation

The live product path is grounded and promptly interruptible on this fixture.
Time to first useful text and repeated discovery remain measurable experience
problems. The next retrieval baseline must compare against these values instead
of assuming an index will improve them.

The committed record contains no raw prompt, answer, source dump, credential,
provider diagnostic, or machine-specific repository path. Full local reports
remain ignored and disposable.
