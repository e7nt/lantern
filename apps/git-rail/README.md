# Lantern focused Git rail

This maintained Rust application implements the bounded Git command and
renderer boundary promoted by ADR 006. Its only runtime dependency is the same
pinned Crossterm version used by Lantern's terminal.

```bash
cargo fmt --manifest-path apps/git-rail/Cargo.toml --check
cargo test --manifest-path apps/git-rail/Cargo.toml
cargo clippy --manifest-path apps/git-rail/Cargo.toml --all-targets -- -D warnings
cargo build --release --manifest-path apps/git-rail/Cargo.toml
node scripts/benchmark-git-surfaces.mjs
```

The spike supports exact categorized status, bounded file diffs, file and hunk
stage/unstage, local branch create/switch, commit, fetch, fast-forward-only pull,
recent history, conflict visibility, and detached HEAD. Every path operation is
bound to the exact canonical workbench root and rejects traversal.

The first 10% renderer shows the active branch and one compact, conflict-first
list using readable `conflict`, `staged`, `modified`, and `untracked` states.
The focused row has a textual `>` marker, so neither state nor focus depends on
color. Arrow keys or `j`/`k` and the mouse select a path; Enter or `d` opens its
bounded staged, unstaged, or untracked diff; and Space stages or unstages the
selected file. In diff view, `j`/`k`
moves between independently applicable hunks, Page Up/Down or the mouse wheel
scrolls a tall hunk, Space stages or unstages only that hunk, and Enter opens
its changed-line range in the existing Helix process. Conflicts open directly
in Helix. `r` refreshes and `q` exits. A file with both staged and unstaged
changes intentionally has two independently reviewable rows.

Press `?` for shortcuts specific to the current view. Long paths retain their
filename in narrow rails. With the mouse, left click selects and reviews, right
click stages or unstages, middle click opens in Helix, and the wheel scrolls.
Press `Ctrl-a` on a file or hunk to close the modal rail and ask through the
existing Lantern composer with that exact bounded Git review attached. Opening
`/git` again restores the reviewed file and exact or nearest surviving hunk.

Press `a` for the temporary action overlay: commit the staged set, create or
switch a local branch, fetch, fast-forward pull, or inspect twenty recent
commits and one bounded commit diff. Pull reports no-upstream, up-to-date,
ahead, behind, and diverged states explicitly and executes only for behind
branches through `git pull --ff-only`. The overlay contains no push, discard,
reset, stash, rebase, amend, or remote-administration action.

Try the renderer from a repository in a separate terminal:

```bash
cargo run --manifest-path /path/to/lantern/apps/git-rail/Cargo.toml
```

The renderer is wired to `/git`. Its functional scope is intentionally closed.
Commands have bounded output, typed private
errors, noninteractive credentials, and local/network deadlines; fetch and
fast-forward pull remain responsive and support `Esc` cancellation. One
coalesced background scan detects external edits while preserving the selected
path and hunk. Accessibility and performance promotion gates pass, and no
parallel Git fallback is maintained.
