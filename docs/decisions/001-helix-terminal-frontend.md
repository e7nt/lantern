# ADR 001: Use a Helix-centered terminal frontend

- **Status:** Accepted
- **Date:** 2026-07-15
- **Decision owner:** Lantern project

## Context

Lantern needs an editor surface that keeps code primary, supports direct mouse
and keyboard interaction, makes Git state visible, and lets an understanding-
first agent navigate to exact evidence. A Code OSS/VSCodium workbench offered
maximum UI control but imposed a large permanent editor platform before the
core product interaction was proven.

## Decision

Lantern v0.1 will use pinned Helix as its primary coding surface, Lazygit as an
on-demand narrow Git rail, and a full-width terminal agent pane. The local
daemon and typed protocol remain editor-independent. Lantern will carry only
documented, reproducible Helix patches for missing product-critical boundaries;
it will not maintain a Code OSS frontend in parallel.

The first permanent vertical slice is read-only symbol-grounded Quick Ask:
select a saved symbol, press `Ctrl-a`, resolve exactly one repository definition
and at most eight references through Helix's active LSP session, stream every
range as clickable evidence, and ask the explicit model driver without tools.
An unavailable LSP or unresolved definition is an error, not a request to
literal search.

## Evidence

- The terminal geometry holds Helix at roughly 80% above a full-width 20%
  agent surface; Lazygit opens inside a 10% rail in the upper work region.
- Mouse focus, scrolling, editor selection, picker-preview selection, Git
  interaction, agent controls, and clickable evidence work in one tmux session.
- Two pinned Helix patch files replay byte-for-byte on the recorded upstream
  revision. The inventory names their boundaries and removal conditions.
- The real Helix/rust-analyzer trace resolved `resolved_port` to one definition
  and three exact references in the saved Rust fixture.
- A repeated live `gpt-5.4` answer used a four-line bounded definition window
  to identify the exact `u16` return value `8080`, listed all references, and
  separated observation, inference, and uncertainty.
- DeepEval `quick_ask` v2 passed all four live deterministic contracts. Rust
  protocol, UI, cancellation, strict Clippy, Helix tests, and release builds
  pass.

## Product fit

This is the smallest tested surface that keeps developers editing while the
agent explains its evidence. It preserves authorship and interruption, remains
open and local-first, avoids a second frontend, and adds no silent evidence or
provider fallback. The agent supports the coding surface rather than turning
the editor into a model dashboard.

## Consequences

- Phase 1 hardens the existing Rust daemon/protocol and narrow Helix bridge; it
  does not scaffold Code OSS or a VS Code extension.
- Unsaved buffers are rejected for LSP symbol context until versioned unsaved
  evidence can be represented without ambiguity.
- The tmux command-delivery seam remains provisional. A stable Helix IPC or an
  accepted narrow fork must replace it before v0.1 packaging.
- Windows is not a v0.1 target for this terminal composition.
- Voice collaboration remains a later read-only spike on the same event,
  cancellation, and evidence-policy boundary.

## Rejected alternatives

- **Code OSS/VSCodium initially:** more control, but substantially more platform
  code and duplicate capabilities before usefulness is proven.
- **A Helix extension-only integration:** Helix does not expose the typed remote
  selection, picker-preview, and LSP export seams this flow requires.
- **Maintaining both frontends:** violates Lantern's no-parallel-path rule and
  divides quality and interaction work.
- **Literal search when LSP fails:** changes evidence meaning and quality without
  the developer's consent.

## Revisit conditions

Revisit if normal editing becomes fragile, coherent editor-native change
transactions and undo cannot use a narrow audited surface, or the 20% agent pane
fails measured longer-form understanding tasks. Rejection means deleting this
frontend path, not adding another fallback.
