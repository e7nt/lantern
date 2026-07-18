# LSP call-hierarchy spike — 2026-07-18

Status: implemented and retained. The typed LSP signal resolves the measured
incomplete-evidence miss. A prompt-only optimization did not change tool
behavior and was rejected.

## Measured miss

The version 2 retrieval baseline asks how Helix handles multiple definition
locations. Selection, definition, and reference evidence omit the local control
flow. Three live runs therefore used `grep`, `read`, and `read`, with a 3,223 ms
median first activity time.

Adding an instruction to prefer contextual grep did not change the sequence. A
live run still used `grep`, `read`, and `read`, with first activity at 3,168 ms.
The instruction was reverted; Lantern does not retain ineffective prompt text.

## Typed-structure result

A read-only rust-analyzer call-hierarchy probe at the pinned Helix revision
returned:

1. selected `goto_definition` → outgoing `goto_single_impl` at line 946;
2. `goto_single_impl` → outgoing `goto_impl` at line 914.

Those locations lead directly to the `Picker` and `jump_to_location` behavior
required by the case. Helix's LSP client already implements prepare and outgoing
call-hierarchy requests, so Lantern can extend its narrow Helix export instead
of creating a daemon-owned parser or vector index for this miss.

## Implementation boundary

Protocol v7 exports only bounded, repository-local outgoing-call locations:

- one hop from the selected enclosing symbol;
- a second hop only through the directly invoked local symbol;
- deterministic deduplication and a strict location ceiling;
- explicit typed provenance in the Lantern protocol;
- bounded source excerpts assembled by the daemon;
- no durable index, embedding dependency, generated summary, or prompt
  heuristic.

The Helix patch obtains an owned handle to the active language server, prepares
call hierarchy at the selection, follows at most four direct calls, and follows
only the first same-document callee for at most four second-hop calls. The
daemon reads at most 32 lines per call location and marks names as untrusted.
The terminal opens the deepest call before the definition and labels the typed
provenance as a call path.

## Retention result

| Run | Tools | First text | Settled | Three-second gate |
| --- | ---: | ---: | ---: | --- |
| 1 | 0 | 2,343 ms | 4,389 ms | pass |
| 2 | 0 | 2,080 ms | 3,986 ms | pass |
| 3 | 0 | 2,132 ms | 3,883 ms | pass |

Every answer contained `Picker` and `jump_to_location`, observed selection,
two call records, and definition evidence, and left the pinned Helix repository
unchanged. Median first text was 2,132 ms. The prior incomplete-evidence runs
used `grep`, `read`, and `read`, with a 3,223 ms median first activity and
7.57–10.66 second first text. The component therefore earns retention.

No raw model output, prompts, repository source, credentials, provider
diagnostics, or machine-specific paths are committed.
