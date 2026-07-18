# Incremental semantic index — 2026-07-18

Status: retained. All three vocabulary-mismatch cases pass the end-to-end gate.

Lantern now prepares a pinned local FastEmbed worker and model. The worker
extracts bounded language-aware symbols from tracked Python, JavaScript,
TypeScript, Go, and Rust source; builds outside the question path; stores
immutable revision directories behind an atomic `CURRENT` pointer; and reuses
unchanged vectors by content hash.

Protocol v8 adds `semantic` evidence provenance. The daemon queries only a
ready current-revision index, reopens every returned range through its existing
repository-bound source reader, and supplies those verified excerpts to Pi.
Building, unavailable, stale, and failed states never produce semantic
evidence. The terminal labels matches as `Related code` and opens them directly
in Helix.

| Repository | Tools | First text | Settled | Prior exact first activity |
| --- | ---: | ---: | ---: | ---: |
| Requests | 0 | 2,272 ms | 8,218 ms | 3,750 ms |
| p-limit | 0 | 2,234 ms | 4,726 ms | 4,272 ms |
| Pi | 0 | 2,209 ms | 5,652 ms | >45,000 ms timeout |

Every answer used verified semantic provenance and left its repository
unchanged. A repeated Pi run after aligning the deterministic contract with the
recovery behavior began text under three seconds and passed. The earlier
p-limit acceptance run began in 2,561 ms and also passed.

Downloaded model data, virtual environments, index artifacts, raw model output,
source bodies, credentials, provider diagnostics, and machine-specific paths
remain uncommitted.
