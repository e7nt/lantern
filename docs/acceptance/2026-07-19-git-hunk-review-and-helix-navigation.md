# Git hunk review and Helix navigation — 2026-07-19

Status: focused hunk-review interaction passes; Lazygit remains maintained
until the remaining ADR 005 promotion gates pass.

The focused rail now parses bounded zero-context Git output into typed
`DiffHunk` values. Each value owns an independently applicable patch, display
content, and an optional positive current-file navigation range. A
deletion-only hunk remains reviewable but explicitly reports that no current
source range exists. Binary and malformed diffs fail instead of being presented
as selectable text hunks.

In the rail, `j` and `k` select a hunk, Page Up/Down and the mouse wheel review
a hunk taller than the viewport, Space stages or unstages only the selected
hunk, and Enter passes its exact repository-relative path and changed-line
range to the existing `lantern-open-range` bridge. Conflicted files skip diff
parsing and open directly in Helix for manual resolution. The rail never owns
editing semantics.

A live 30 by 18 terminal journey created two separated changes in one file.
The rail rendered `1/2` and only the first hunk. Pressing Space produced a Git
index containing `+ONE` but not `+FIVE`; the second change remained unstaged.
The assertion inspected the repository's cached diff rather than trusting the
rendered screen.

The focused suite now has thirteen deterministic tests: three diff-parser tests,
four renderer-state tests, and six repository journeys. Formatting, all tests,
Clippy with warnings denied, the narrow live interaction, and
`git diff --check` pass.

The next slice is one compact action overlay for commit message, local branch
creation/switching, fetch, fast-forward-only pull, and bounded recent history.
It must not introduce permanent panels or expose excluded destructive Git
operations.
