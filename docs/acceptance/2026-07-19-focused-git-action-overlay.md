# Focused Git action overlay — 2026-07-19

Status: the focused rail's functional scope passes; Lazygit remains maintained
until operational, accessibility, and performance promotion gates pass.

Pressing `a` now opens one temporary list over the unchanged review position:
Commit, Branches, Fetch, Pull, and History. Escape returns without changing the
selected file or hunk. Keyboard and mouse can choose list entries. Text entry
is bounded to Git's existing commit-message and branch-name contracts and
renders a visible cursor marker in the narrow rail.

Commit shows the staged-file count and accepts one developer-written message.
Branches lists local branches and one create action. History contains at most
twenty full object IDs plus summaries; choosing one opens a diff bounded to 512
KiB. Fetch is explicit. Pull computes upstream state from machine-readable ref
and revision counts, then runs `git pull --ff-only` only when behind. No
upstream, up-to-date, ahead, and diverged states are displayed without mutation.

A live 30 by 18 journey committed one staged file as `focused commit`, created
and switched to `review`, displayed the two-entry history, and opened the
selected commit diff containing `+new`. Repository state and Git history were
asserted directly. The existing remote journey separately proved typed
`Behind { commits: 1 }` before pull and `UpToDate` afterward.

The suite now has sixteen deterministic tests: four Git parsing/state tests,
six renderer-state tests, and six repository journeys. Formatting, all tests,
Clippy with warnings denied, the live overlay journey, and `git diff --check`
pass.

No push, discard, reset, stash, rebase, amend, cherry-pick, bisect, remote
administration, or embedded conflict editor was added. Functional expansion
stops here. The next work is promotion hardening: command deadlines,
noninteractive credential behavior, bounded typed errors, refresh concurrency,
accessibility verification, and startup/RSS comparison with pinned Lazygit.
