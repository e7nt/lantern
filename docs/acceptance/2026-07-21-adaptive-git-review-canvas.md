# Adaptive Git review canvas acceptance — 2026-07-21

## Outcome

The focused Git surface remains a 10% file/status rail until a developer opens
a diff. It then replaces only the upper editor region with an 80%-wide review
canvas, leaving the full-width agent terminal below untouched. `Esc` returns to
the rail.

The transition restarts the small focused Git process rather than adding a
permanent pane or a second implementation. A strict, bounded local resume
envelope preserves the file, hunk identity, selected diff line, scroll offset,
and pending review-comment count. The existing typed `GitReviewContext` remains
the only payload sent to the agent.

The expanded canvas adds `p`/`n` file navigation to existing `[`/`]` hunk
navigation, keyboard and mouse line selection, line comments, staging, Helix
navigation, and explicit review submission.

## Verification

- `cargo test -p lantern-git-rail`: passed (24 tests).
- `node --test scripts/test/terminal-foundation.test.mjs`: passed (12 tests).
- The resume test proves exact hunk, line, offset, and pending-count restoration.
- The terminal foundation test proves the 10% compact and 80% expanded popup
  geometry and explicit layout handoff.

No model behavior changed, so the deterministic DeepEval contract suite is not
required for this UI-only slice. Live visual validation remains for the next
developer tryout.
