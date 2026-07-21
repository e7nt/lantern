# Editable code-review draft acceptance — 2026-07-21

## Outcome

The expanded Git canvas now provides one local PR-review-like draft before the
agent is contacted. Commented lines have visible markers. A developer can add
comments with `c` or right click, edit the selected line's comment with `e`,
remove it with `x`, and inspect the complete cross-file draft with `v`.
Selecting a summary entry returns to its exact file, hunk, and diff line.

`R` opens a concise confirmation with the exact comment count. `Enter` or `y`
sends the complete validated batch as one correction turn; `Esc` returns to the
draft. The Git canvas is the sole owner before confirmation. The terminal
becomes the sole owner after confirmation and retains the batch through
rejection, provider failure, or interruption until a completed correction.

Protocol v16 removes the obsolete one-comment-at-a-time control messages. It
does not retain a compatibility fallback.

## Verification

- `cargo test --workspace --all-targets`: passed (117 tests).
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `node --test scripts/test/terminal-foundation.test.mjs`: passed (12 tests).
- `DEEPEVAL_DISABLE_DOTENV=1 uv run pytest`: passed (55 tests), with Ruff
  formatting and lint checks also passing.
- Git rail tests cover exact marker anchors, add, edit, remove, and full-draft
  resume across compact and expanded modes.
- Terminal control tests prove the complete validated batch crosses the private
  socket in one request; existing lifecycle tests prove post-submit retention.
- The daemon real journey still proves that multiple comments produce one
  coherent correction turn.

No model prompt or routing behavior changed, so this deterministic interaction
slice required no new DeepEval case; the existing contract suite still passes.
