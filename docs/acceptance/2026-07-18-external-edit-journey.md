# External edit journey — 2026-07-18

Status: passed on Linux against a disposable three-file JavaScript repository
outside the Lantern checkout.

## Runtime

- Pi: `0.80.6`
- Model: `gpt-5.4` through Pi-owned OpenAI Codex authentication
- First tool: 4,918 ms
- First response text: 15,140 ms
- Settled: 15,898 ms
- Tool calls: 8
- Ordered tools: `find`, `find`, `list`, `read`, `read`, `read`, `edit`, `bash`

## Outcome

- One implementation file changed.
- The requested one-line behavior change was correct.
- The repository's focused Node test passed.
- The change remained unstaged and uncommitted.
- Git reported one insertion and one deletion in one file.
- Helix opened the changed file after the edit.
- The compact pane rendered a concise result, verification, and real caveat.
- The same journey remained readable through the reversible full-screen mode.

## Interruption and cleanup

A separate live turn was interrupted from the Lantern pane. The accepted
operation reported cancellation in 35 ms, settled, returned to the quiet
prompt, and removed its session-local runtime directory and control socket when
the workbench closed.

## Correction made during acceptance

The first live attempt used 12 tools, including seven discovery calls, and
repeated routine narration. The harness prompt now tells Pi that Lantern already
shows tool activity, forbids equivalent repeated discovery, and requests one
concise final result. The repeated run used eight justified calls and completed
about five seconds faster. DeepEval now enforces a maximum tool-call budget and
rejects repeated discovery traces.

No source, prompt, command output, credential, absolute repository path, or
provider diagnostic is recorded here.
