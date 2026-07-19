# Focused Git promotion — 2026-07-19

Status: complete. ADR 006 is implemented; Lantern has one maintained Git
surface and no Lazygit runtime fallback.

The formerly removable crate now lives at `apps/git-rail` as workspace package
`lantern-git-rail`. The root locked release build produces
`target/release/lantern-git-rail`. Both Helix `Space-g` and the agent pane's
`/git` command call one `lantern-git` popup launcher, which passes the exact
workbench root to the rail and preserves the existing 10%-wide, upper-80%
layout.

The same checkpoint removes Lazygit from upstream preparation, revision
verification, Go build requirements, launch validation, session environment,
Helix key binding, popup launcher, configuration, frontend documentation, and
terminal-foundation tests. The upstream manifest now contains only Helix.
Lazygit remains named solely in historical evidence, reference-repository
records, retrieval fixtures, and the explicit performance comparator; none is
a product launch path or fallback.

The maintained release workspace builds successfully. All ninety-one Rust
workspace tests pass, including the twenty-two Git tests and journeys. All
eight terminal-foundation tests pass against the focused popup launcher.
Workspace Clippy with warnings denied, Rust formatting, JavaScript syntax,
shell syntax, and `git diff --check` pass.

The migration test caught and corrected one packaging fault before promotion:
the renamed popup launcher initially lacked its executable bit. No compatibility
wrapper or old command alias was retained.
