# Semantic retrieval spike — 2026-07-18

Status: measured need confirmed; fixed-window local embedding implementation
rejected. Embeddings are justified as a retrieval signal, but the tested index
shape is too expensive to ship.

## Vocabulary-mismatch baseline

Dataset v1 asks three natural questions without naming their implementation
symbols:

- redirect credential leakage in Requests;
- waking queued work when p-limit regains capacity;
- recovering unfinished Pi tool calls after an output limit.

The unchanged repository-only Pi path produced:

| Repository | Tools | First activity | First text | Settled | Gate |
| --- | ---: | ---: | ---: | ---: | --- |
| Requests | 7 | 3,750 ms | 19,317 ms | 23,301 ms | fail |
| p-limit | 3 | 4,272 ms | 8,479 ms | 11,807 ms | fail |
| Pi | unknown | >45,000 ms | >45,000 ms | timeout | fail |

The completed answers were grounded and correct, but neither met the strict
three-second first-activity requirement. The timeout is recorded as a terminal
evaluation outcome rather than weakening the ceiling.

## Local embedding probe

The disposable probe used FastEmbed `0.8.0` with
`BAAI/bge-small-en-v1.5`, 32-line source windows, eight-line overlap, tracked
source files, and repository-relative paths in the embedded text.

Warm semantic queries were fast and relevant:

- p-limit returned the queue-control regions first and second in 11 ms;
- Requests returned the credential-rebuild region first in 14 ms.

The indexing cost failed:

| Repository | Chunks | Model load | Index build | Query |
| --- | ---: | ---: | ---: | ---: |
| p-limit | 49 | 10,295 ms cold | 2,932 ms | 11 ms |
| Requests | 518 | 228 ms warm | 35,892 ms | 14 ms |
| Pi | not completed | warm | >120,000 ms | not reached |

The Pi build was actively interrupted after two minutes. No question should
wait for indexing, and a dependency stack plus whole-repository fixed windows
would violate Lantern's lightweight product direction.

## Decision

Retain the versioned vocabulary-mismatch dataset and its exact baseline runner.
Do not retain FastEmbed, the downloaded model, fixed-window chunking, or any
runtime/protocol changes from this probe.

The next candidate must use language-aware symbol-sized chunks, build and
update outside the question path, store disposable artifacts keyed by
repository revision, verify retrieved candidates against current source, and
beat this dataset end to end before entering Lantern.

Raw model output, source bodies, downloaded models, credentials, and
machine-specific paths remain uncommitted.
