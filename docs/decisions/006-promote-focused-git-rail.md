# ADR 006: Promote Lantern's focused Git rail

- **Status:** Accepted
- **Date:** 2026-07-19
- **Decision owner:** Lantern project

## Context

ADR 005 authorized a removable focused Git spike while retaining Lazygit until
functional, operational, interaction, and performance gates passed. The spike
now supports the accepted review journey: typed status, bounded file and hunk
diffs, stage and unstage, Helix navigation, commit, local branches, fetch,
fast-forward-only pull, history, external refresh, keyboard and mouse input,
and contextual help.

All Git commands share bounded output, typed private errors, noninteractive
credentials, deadlines, and process-group cancellation. Twenty-two
deterministic tests pass. The external-refresh journey preserves a selected
hunk when an earlier hunk appears and exits safely when the file becomes clean.

On the reproducible 1,001-file fixture, the release rail measured 79.7 ms
median startup and 2,960 KiB RSS. Pinned Lazygit measured 95.6 ms and 24,888
KiB. The rail's visible external refresh was 704.7 ms versus 8,364.7 ms. Full
method and raw values are in the performance acceptance report.

## Decision

Promote `apps/git-rail` into the maintained Rust workspace and make it the only
implementation behind `Space-g` and `/git`.

Remove the Lazygit binary from preparation and launch requirements, its runtime
environment variables, configuration, popup launcher, and product tests in the
same checkpoint. Keep Lazygit only as a pinned historical reference and
benchmark comparator; it is not built, installed, launched, or offered as a
fallback.

The focused scope remains closed. New Git operations require evidence from a
real developer journey and an ADR; parity with broad Git clients is not a goal.

## Consequences

- A prepared workbench no longer requires Go or a 27 MB Lazygit binary.
- Git state and focus remain readable without relying on color.
- The workbench owns one small Git UX aligned with code review and authorship.
- Advanced and destructive Git operations remain available through an explicit
  terminal or developer-requested agent action, not permanent rail chrome.
- A regression in the rail is fixed directly; Lazygit is not retained as a
  silent or indefinite fallback.

## Evidence

- [Command hardening](../acceptance/2026-07-19-git-command-hardening.md)
- [External refresh](../acceptance/2026-07-19-git-external-refresh.md)
- [Interaction accessibility](../acceptance/2026-07-19-git-interaction-accessibility.md)
- [Performance gate](../acceptance/2026-07-19-git-surface-performance.md)
