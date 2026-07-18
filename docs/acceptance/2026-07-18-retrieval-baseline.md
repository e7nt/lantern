# Exact and LSP retrieval baseline — 2026-07-18

Status: completed one authenticated comparison on each of two pinned external
repositories through Lantern's real Protocol v6 daemon.

## Repositories

- Helix at revision `14d6bc0febed9c692048271a8ae2362ac969c6e0`.
- Lazygit at revision `080da5cacfcff63a89ea23493bb91b11b0612876`.

These are non-Lantern Rust and Go codebases with different repository shapes.
Both repository-only and LSP-assisted turns answered the same curated question
using the same Pi version, provider, model, daemon, and tool allowlist.

## Results

| Case | Mode | Tools | First tool | First text | Settled | Contract |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Helix definition flow | exact | 4 | 3,941 ms | 11,728 ms | 14,607 ms | pass |
| Helix definition flow | LSP | 2 | 4,578 ms | 6,873 ms | 7,965 ms | pass |
| Lazygit GUI construction | exact | 15 | 6,948 ms | 31,979 ms | 36,631 ms | fail |
| Lazygit GUI construction | LSP | 7 | 3,954 ms | 15,802 ms | 19,330 ms | pass |

Every answer contained the curated facts and observed the expected source
files. Both repositories retained their exact pre-run Git status. The Lazygit
exact run failed only because 15 calls exceeded the eight-call efficiency
ceiling; the ceiling was not changed after observing the result.

## Measured effect

- Helix LSP context removed two tool calls, produced useful text 4,855 ms
  sooner, and settled 6,642 ms sooner.
- Lazygit LSP context removed eight tool calls, produced useful text 16,177 ms
  sooner, and settled 17,301 ms sooner.
- LSP evidence improved both cases without a semantic index. It did not remove
  all discovery: Pi still used two tools for Helix and seven for Lazygit.

## Decision

Prioritize making typed editor/LSP evidence consistently available and reducing
discovery after sufficient evidence is already present. Do not start a
semantic/vector index from these results: neither grounded answer required one.
Add a semantic retrieval experiment only after a curated question remains
incorrect or materially slow with both exact and LSP evidence.

This is an initial baseline, not a latency distribution. Repeat it before using
small timing differences as a release gate. The large observed deltas and the
exact-only efficiency failure are hypotheses for the next expanded dataset.

No raw answers, prompts, source dumps, credentials, provider diagnostics, or
machine-specific paths are committed.
