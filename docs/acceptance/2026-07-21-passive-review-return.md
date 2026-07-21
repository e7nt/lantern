# Passive review return acceptance — 2026-07-21

## Outcome

After the agent completes a submitted code review, the existing one-shot Git
focus now carries the developer's submitted comments beside the agent-edited
paths. Opening the expanded diff shows a quiet `Your review · N comments`
section. `v` displays the comments; selecting one navigates to the nearest
surviving file and hunk context.

The section is informational. It has no inferred resolution state, acceptance
checkboxes, approval requirement, or completion gate. The developer can simply
read the new diff and continue working. If correction is still needed, the
ordinary line-comment draft remains available.

Protocol v17 strictly validates and bounds the returned comments. The focused
Git resume envelope preserves the section across compact and expanded modes.

## Verification

- `cargo test --workspace --all-targets`: passed (117 tests).
- Clippy with warnings denied, the locked release build, and 12 terminal
  composition tests passed.
- The existing DeepEval suite passed (55 tests), with Ruff formatting and lint
  checks also passing.
- Terminal tests prove submitted feedback transfers only after successful
  completion and is cleared from the terminal after the Git handoff.
- Git rail tests prove the one-shot focus consumes the review section and the
  resume envelope preserves it.

No model behavior changed; this is a deterministic presentation and handoff
slice.
