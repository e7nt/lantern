# Focused Git rail spike

This isolated Rust crate tests the smallest Git command and renderer boundary
proposed by ADR 005. Its only runtime dependency is the same pinned Crossterm
version used by Lantern's terminal, and it does not replace Lazygit.

```bash
cargo fmt --manifest-path spikes/git-rail/Cargo.toml --check
cargo test --manifest-path spikes/git-rail/Cargo.toml
cargo clippy --manifest-path spikes/git-rail/Cargo.toml --all-targets -- -D warnings
```

The spike supports exact categorized status, bounded file diffs, file and hunk
stage/unstage, local branch create/switch, commit, fetch, fast-forward-only pull,
recent history, conflict visibility, and detached HEAD. Every path operation is
bound to the exact canonical workbench root and rejects traversal.

The first 10% renderer shows the active branch and one compact, conflict-first
list using `!`, `+`, `~`, and `?` markers. Arrow keys or `j`/`k` and the mouse
select a path; Enter or `d` opens its bounded staged, unstaged, or untracked
diff; and Space stages or unstages the selected file. In diff view, `j`/`k`
moves between independently applicable hunks, Page Up/Down or the mouse wheel
scrolls a tall hunk, Space stages or unstages only that hunk, and Enter opens
its changed-line range in the existing Helix process. Conflicts open directly
in Helix. `r` refreshes and `q` exits. A file with both staged and unstaged
changes intentionally has two independently reviewable rows.

Press `a` for the temporary action overlay: commit the staged set, create or
switch a local branch, fetch, fast-forward pull, or inspect twenty recent
commits and one bounded commit diff. Pull reports no-upstream, up-to-date,
ahead, behind, and diverged states explicitly and executes only for behind
branches through `git pull --ff-only`. The overlay contains no push, discard,
reset, stash, rebase, amend, or remote-administration action.

Try the renderer from a repository in a separate terminal:

```bash
cargo run --manifest-path /path/to/lantern/spikes/git-rail/Cargo.toml
```

The renderer is deliberately not production-ready or wired to `/git`. Its
functional scope is complete. Commands now have bounded output, typed private
errors, noninteractive credentials, and local/network deadlines; fetch and
fast-forward pull remain responsive and support `Esc` cancellation. External
state-preserving refresh, accessibility checks, and a performance comparison
remain.
Failure of those promotion gates deletes this crate and retains pinned Lazygit.
