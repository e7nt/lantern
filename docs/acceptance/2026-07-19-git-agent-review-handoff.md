# Git-to-agent review handoff — 2026-07-19

Status: complete. Git review and agent conversation now share one typed,
bounded context path and the existing composer.

From the change list or a selected hunk, `Ctrl-a` exports a `GitReviewContext`
containing the repository-relative path, conflict/staged/modified/untracked
state, file-or-hunk scope, current line range, and bounded review evidence. A
file selection includes every bounded hunk; a hunk selection includes only that
hunk. Oversized file reviews instruct the developer to select the exact hunk.

The modal Git popup exits through one private status understood by its launcher,
then the existing `lantern-agent-composer` opens with “Ask about the selected
Git change.” No nested popup, second chat process, Git-specific prompt syntax,
or alternate agent mode was added. The terminal consumes the context once,
clears stale Helix symbol context, and sends the existing typed
`AskAgentSelection` request. Dismissing the composer deletes the unsubmitted
one-shot context.

The daemon receives the complete bounded diff rather than only its first line.
The context remains untrusted evidence and grants no new capability. Deleted
files are valid Git review evidence even when no working-tree path can be
canonicalized; all paths remain validated repository-relative values.

Because tmux popups are modal, the Git rail closes before the developer talks
in the full-width agent pane. A separate session-local resume marker preserves
the reviewed file and hunk. Reopening `/git` restores the exact hunk when it
survives an agent edit, otherwise the nearest current hunk; a cleaned file is
reported explicitly. The conversational context and resume marker have
separate lifetimes and are removed during session cleanup.

The workspace now passes ninety-four Rust tests. Coverage proves typed review
validation and conversion, complete diff delivery to Pi, deleted-file review,
exact hunk export, file-scope multi-hunk export, restoration after an earlier
hunk appears, and one-shot terminal consumption. Ten terminal-foundation tests
prove the single-composer handoff and dismissal cleanup. Formatting, strict
Clippy, shell syntax, and `git diff --check` pass.
