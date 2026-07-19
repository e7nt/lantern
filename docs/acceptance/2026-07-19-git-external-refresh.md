# State-preserving Git refresh — 2026-07-19

Status: the focused rail's external-refresh gate passes. Lazygit remains
maintained until accessibility and measured startup/RSS gates pass.

The rail now requests one bounded background status scan every 750 ms. A scan
never blocks rendering or input, and another scan cannot start while one is in
flight. No filesystem-watcher dependency or persistent indexing service was
added. Manual `r` refresh remains available.

Change-list selection is preserved by exact Git state and path rather than row
number. If staging state changes externally, the rail selects the same path in
its new state. In diff review, the selected hunk is identified by its unified
hunk body rather than the file-level patch: Git changes the patch index hash
when an unrelated hunk changes. This keeps the same hunk selected even when a
new earlier hunk changes its list position. The scroll offset is retained and
bounded to the refreshed hunk.

If the selected file becomes clean, the rail returns to the change list and
says so. If its staged state changes, it returns to the corresponding refreshed
entry. Results started before a local mutation are rejected by repository
generation, and results for a diff the developer has since left or navigated
away from cannot reopen or move that view.

A real-repository test begins on the second of two hunks, externally adds a
third earlier hunk and an untracked file, and proves the original hunk moves
from position two to position three without losing selection. It then restores
the file and proves the rail returns to the list with a clean-file notice.

The suite now has twenty-one deterministic tests: eight command/parser tests,
seven renderer and refresh tests, and six repository journeys. Formatting,
Clippy with warnings denied, and `git diff --check` pass.

Remaining promotion work is keyboard/mouse accessibility verification and a
reproducible startup/RSS comparison with pinned Lazygit.
