# Go call-hierarchy validation — 2026-07-18

Status: passed. Protocol v7's existing bounded call evidence works across the
generic LSP boundary; no Go-specific runtime code or broader hierarchy was
needed.

## Probe

The pinned Lazygit revision
`080da5cacfcff63a89ea23493bb91b11b0612876` was queried with the official
`gopls v0.23.0` call-hierarchy implementation. The probe selected `NewApp` in
`pkg/app/app.go` and observed these relevant edges within Lantern's existing
limits:

1. direct calls to `validateGitVersion`, `setupRepo`, `GetRepoPaths`, and
   `NewNullGuiIO`;
2. a second hop through the first same-document callee,
   `validateGitVersion`, to `minGitVersionErrorMessage`, `GetGitVersion`,
   `ParseGitVersion`, and `IsOlderThanVersion`.

This is enough to answer how startup validates Git and prepares a repository
before constructing the GUI. The result also confirms that gopls ordering is
semantic rather than source-order, so the case asks only what the bounded
evidence supports.

## Live result

The version 4 retrieval baseline ran exact discovery and LSP-assisted discovery
through the real Protocol v7 daemon and the same Pi adapter.

| Mode | Tools | First text | Settled | Result |
| --- | ---: | ---: | ---: | --- |
| Exact | 3 | 8,838 ms | 15,634 ms | pass |
| LSP | 0 | 2,343 ms | 8,330 ms | pass |

The LSP answer included `validateGitVersion` and `setupRepo`, carried selection,
definition, reference, and call provenance, met the strict three-second gate,
and left the Lazygit checkout unchanged. The retained Helix regression also
passed in the same run with zero LSP-mode tools and first text at 2,295 ms.

## Retention decision

Dataset v4 retains both Rust and Go cases. Lantern does not bundle gopls: the
workbench continues to use the language server configured for the repository,
and explicitly reports unavailable LSP features. The validation binary and raw
timestamped report remain ignored local artifacts.

No raw model output, prompts, repository source, credentials, provider
diagnostics, or machine-specific paths are committed.
