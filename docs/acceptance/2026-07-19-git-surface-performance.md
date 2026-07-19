# Git surface performance gate — 2026-07-19

Status: pass. The focused rail starts faster and uses less memory than pinned
Lazygit on the same fixture. Promotion to `/git` is now permitted; removal of
the old Lazygit path must happen in the same checkpoint.

## Method

`scripts/benchmark-git-surfaces.mjs` creates a disposable repository with 1,001
tracked files plus staged, unstaged, and untracked changes. It launches both
release binaries in alternating order through isolated tmux sessions with
identical 120×40 panes and waits for an implementation-specific usable-frame
marker. Lazygit's one-time promotional popup is disabled in the disposable
benchmark configuration so it measures the maintained Git surface rather than
first-run marketing.

Six processes per implementation measure startup and process-tree RSS after a
500 ms settle. The final process also measures a contextual-help frame change
and automatic appearance of an externally created, visible file. The fixture,
home directory, tmux server, and Lazygit state are deleted afterward. The
script prints environment metadata, every raw sample, summaries, and the
machine-readable pass decision as JSON.

Run from a release build:

```bash
cargo build --release --manifest-path spikes/git-rail/Cargo.toml
node scripts/benchmark-git-surfaces.mjs
```

## Result

Environment: Linux x64, Node v22.23.1, tmux 3.4, Git 2.43.0. Lazygit revision:
`080da5cacfcff63a89ea23493bb91b11b0612876`.

| Measure | Focused rail | Lazygit |
|---|---:|---:|
| Startup median | 79.7 ms | 95.6 ms |
| Startup p95 | 81.4 ms | 110.4 ms |
| First launch | 97.8 ms | 101.4 ms |
| RSS median | 2,960 KiB | 24,888 KiB |
| Binary size | 1,110,696 bytes | 27,357,395 bytes |
| Help input to changed frame | 14.7 ms | 18.6 ms |
| External edit to visible frame | 704.7 ms | 8,364.7 ms |
| Parent/live-tree idle CPU ticks per second | 1 median | 0 median |

Raw startup milliseconds:

- focused rail: 97.758, 81.142, 67.773, 79.719, 81.401, 78.389;
- Lazygit: 101.394, 86.455, 94.579, 95.562, 119.039, 110.395.

Raw RSS KiB:

- focused rail: 2,952, 2,960, 2,960, 2,980, 2,996, 2,984;
- Lazygit: 24,648, 25,352, 25,196, 24,660, 25,008, 24,888.

Raw one-second idle CPU ticks:

- focused rail: 2, 0, 1, 1, 0, 1;
- Lazygit: 0, 0, 0, 0, 0, 0.

## Interpretation and limit

The focused rail is 16.6% faster at median startup, uses 88.1% less resident
memory, and its binary is 95.9% smaller. It also exposes external changes far
sooner. Its bounded 750 ms status scan costs a median one scheduler tick per
second in this fixture; that is an explicit simplicity/visibility tradeoff,
not a zero-cost watcher claim.

Linux `/proc` point sampling includes the live process tree but cannot account
for a Git child that starts and exits wholly between samples. Idle CPU is
therefore directional, while startup, RSS, binary size, input response, and
visible refresh are direct measurements. The promotion requirement is faster
startup and lower memory on the same fixture; both pass by substantial margins.
