# Pi SDK tool-control spike — 2026-07-18

Status: first ADR 005 agent-runtime gate passed; production promotion remains
pending streaming, interruption, latency comparison, and DeepEval parity.

The reproducible `scripts/spike-pi-sdk-control.mjs` locates the installed pinned
Pi 0.80.6 package, imports its public SDK, and creates a session with only
`read` and `edit`. A Lantern-owned inline `tool_call` hook blocks mutation until
an in-memory confirmation gate is opened.

The live subscription-backed run used a disposable Git repository containing
`sample.txt` with `old`. Pi attempted `edit`; the hook blocked it before
execution, and the script verified the file remained byte-identical. The same
session then received confirmation, applied the requested `old` to `new` edit,
and left exactly one unstaged `sample.txt` modification for review. The fixture
was removed afterward.

The SDK session initialized in 6 ms on the measured run. This is local setup
time, not first-token latency; the promotion gate still requires a like-for-like
RPC comparison around a streamed turn.

This proves that Lantern can own the pre-execution boundary while retaining
Pi's authentication, model runtime, conversation, and tools. It does not yet
justify production replacement. The next agent-runtime spike must stream the
same typed Lantern events, abort an active turn, compare SDK initialization and
first activity with RPC, and rerun the existing model evaluations.

The command is:

```bash
node scripts/spike-pi-sdk-control.mjs
```

It fails for a Pi version other than 0.80.6, a missing mutating tool attempt, a
mutation before approval, an incorrect approved edit, or unexpected Git state.
