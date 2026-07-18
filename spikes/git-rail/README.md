# Focused Git rail spike

This isolated Rust crate tests the smallest Git command boundary proposed by
ADR 005. It has no runtime dependencies and does not replace Lazygit.

```bash
cargo fmt --manifest-path spikes/git-rail/Cargo.toml --check
cargo test --manifest-path spikes/git-rail/Cargo.toml
cargo clippy --manifest-path spikes/git-rail/Cargo.toml --all-targets -- -D warnings
```

The spike supports exact categorized status, bounded file diffs, file and hunk
stage/unstage, local branch create/switch, commit, fetch, fast-forward-only pull,
recent history, conflict visibility, and detached HEAD. Every path operation is
bound to the exact canonical workbench root and rejects traversal.

It is deliberately not production-ready. Git commands still need deadlines,
noninteractive credential behavior, bounded and privacy-reviewed diagnostics,
concurrent refresh semantics, and typed error categories. The 10% keyboard and
mouse rail, Helix navigation, performance comparison, and accessibility checks
are not implemented. Failure of those promotion gates deletes this crate and
retains pinned Lazygit.
