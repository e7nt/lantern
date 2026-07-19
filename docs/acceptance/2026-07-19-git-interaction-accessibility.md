# Git interaction accessibility — 2026-07-19

Status: the focused rail's interaction-quality gate passes. Lazygit remains
maintained until the measured startup/RSS gate passes.

Git state no longer depends on color or punctuation. Every change row says
`conflict`, `staged`, `modified`, or `untracked`, and the focused row begins
with `>`. Menus use the same textual focus marker. Narrow rows retain the
filename, replacing leading directories with `…/`, rather than clipping away
the part developers use to identify a file.

`?` opens help for the current view without replacing that view or its selected
file, hunk, history entry, input, or scroll position. The overlay lists only
the commands available in changes, diff, actions, branches, history, or commit
diff. `?`, Escape, or a mouse press closes it. In text input, `?` remains text.

Mouse interaction now covers the focused review operations: left click selects
and then reviews a row, right click stages or unstages the selected file or
hunk, middle click opens the selected file or hunk in Helix, the wheel scrolls,
and existing menu clicks choose actions. Conflicts still open in Helix rather
than presenting an unsafe stage action.

A live terminal check proved the text state/focus row, contextual changes help,
overlay close, and clean quit path. It also exposed continuous 50 ms repainting;
that debt was removed in this checkpoint. The rail now renders only after
input, resize, or completed background work, while retaining 50 ms input and
cancellation polling.

The suite now has twenty-two deterministic tests: eight command/parser tests,
eight renderer/refresh/accessibility tests, and six repository journeys.
Formatting, Clippy with warnings denied, `git diff --check`, and the live
terminal check pass.

The only remaining promotion gate is a reproducible cold/warm startup, idle
RSS, refresh cost, and input-latency comparison with pinned Lazygit. Failure
retains Lazygit; success permits wiring the focused rail to `/git` and removing
the old path in one checkpoint.
