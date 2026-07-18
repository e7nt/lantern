# Focused Git command spike — 2026-07-18

Status: ADR 005 command boundary passed; Lazygit remains the maintained product
until the rail renderer, operational hardening, and performance gates pass.

The dependency-free Rust crate under `spikes/git-rail` drives only the Git CLI
inside a canonical repository root. Six disposable integration journeys prove:

- staged, unstaged, untracked, and conflicted paths remain separate;
- unstaged and staged file diffs are bounded to 512 KiB;
- one file can be staged, unstaged, committed, and left in exact review state;
- one of two zero-context hunks can be staged and reversed without touching the
  other;
- branch creation and switching, detached HEAD, and bounded recent history are
  explicit;
- fetch plus `pull --ff-only` advances through a real local bare remote without
  creating a hidden merge; and
- absolute paths, traversal, invalid branch names, and an inexact nested
  workbench root are rejected.

Git paths remain raw platform paths on Unix rather than passing through lossy
UTF-8 conversion. Conflicts are reported as paths; the spike never resolves or
discards them.

This result supports a focused rail but does not justify removing Lazygit.
Before promotion Lantern must add command deadlines, disable interactive Git
credential prompts, prevent sensitive remote diagnostics from reaching the UI,
define refresh behavior during external edits, build the real 10% keyboard and
mouse surface, open selected ranges in Helix, and measure startup and resident
memory against the pinned Lazygit build.
