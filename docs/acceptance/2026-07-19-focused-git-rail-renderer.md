# Focused Git rail renderer — 2026-07-19

Status: first renderer gate passed; Lazygit remains the maintained `/git`
surface until the complete interaction, operational, accessibility, and
performance gates pass.

The removable `spikes/git-rail` crate now includes a dependency-pinned
Crossterm renderer over the previously proven Git command boundary. At 18 by 30
terminal cells it visibly rendered the active branch and five changed paths
without panels, borders, command logs, tips, or unrelated Git concepts.

The retained first interaction is intentionally small:

- conflicts first, then staged, unstaged, and untracked changes;
- keyboard and mouse selection;
- bounded review of staged, unstaged, and new-file diffs before mutation;
- file stage and unstage with Space;
- explicit refresh and quit; and
- concise inline errors rather than silent recovery.

Deterministic tests protect Unicode-safe clipping, narrow viewport mouse
mapping, conflict deduplication, and the important case where one path has both
staged and unstaged changes. The existing six repository journeys now also
prove pre-stage review of an untracked file. Formatting, ten tests, Clippy with
warnings denied, and `git diff --check` pass.

The hunk-review follow-up dated 2026-07-19 now adds typed hunk selection,
selective stage/unstage, tall-hunk scrolling, and exact Helix navigation. This
still does not promote the rail. The next slice must add the commit, local
branch, fetch, fast-forward pull, and recent-history dialogs already supported
by the command spike. Git deadlines,
`GIT_TERMINAL_PROMPT=0`, bounded diagnostics, concurrent refresh behavior,
keyboard focus, screen-reader semantics, and measured startup/RSS versus pinned
Lazygit remain mandatory before replacement.

Run it from any exact Git workbench root with:

```bash
cargo run --manifest-path /path/to/lantern/spikes/git-rail/Cargo.toml
```
