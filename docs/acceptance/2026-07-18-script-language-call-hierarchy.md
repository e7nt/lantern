# Python, JavaScript, and TypeScript call hierarchy — 2026-07-18

Status: passed. Protocol v7's unchanged generic LSP path produced useful,
zero-tool answers across all three languages within the strict three-second
first-text gate.

## Pinned probes

The probes used ignored, repository-local toolchains:

- Pyright `1.1.411` for Python;
- TypeScript-language-server `5.3.0` with TypeScript `6.0.3` for JavaScript and
  TypeScript.

TypeScript `7.0.2` was initially inspected but is not compatible with
TypeScript-language-server `5.3.0` because its package no longer exposes the
expected `tsserver.js`. The probe therefore pins TypeScript `6.0.3` explicitly;
Lantern does not attempt a silent compatibility fallback.

The pinned repository questions cover three different bounded shapes:

1. Requests `Session.prepare_request` calls `cookiejar_from_dict` and
   `merge_cookies`, with a bounded second-hop utility call.
2. p-limit calls `validateConcurrency`, whose definition contains the
   `Number.isInteger` and positive-value check. Its second hop resolves only to
   the standard library and is correctly excluded as non-repository evidence.
3. Pi's `runLoop` calls `streamAssistantResponse` and
   `failToolCallsFromTruncatedMessage`, with a bounded second hop into the
   repository's event stream.

## Live result

Dataset v5 ran exact discovery and LSP-assisted discovery through the real
Protocol v7 daemon and the same Pi adapter.

| Language | Mode | Tools | First text | Settled | Result |
| --- | --- | ---: | ---: | ---: | --- |
| Python | Exact | 7 | 15,490 ms | 21,423 ms | pass |
| Python | LSP | 0 | 2,272 ms | 7,472 ms | pass |
| JavaScript | Exact | 2 | 6,843 ms | 9,497 ms | pass |
| JavaScript | LSP | 0 | 2,344 ms | 4,693 ms | pass |
| TypeScript | Exact | 5 | 14,724 ms | 16,265 ms | pass |
| TypeScript | LSP | 0 | 2,341 ms | 3,229 ms | pass |

Every LSP answer included its required grounded terms, carried definition and
call provenance, and left its pinned checkout unchanged. Dataset v5 records the
upstream URLs and revisions so a missing fixture produces an actionable setup
instruction instead of an implicit substitute.

## Retention decision

Retain all three cases without runtime changes. Lantern continues to consume
the language server selected by Helix, filters evidence to the active
repository, and accepts that useful hierarchy depth differs by language. It
does not bundle these validation servers or synthesize missing call edges.

The installed validation binaries and raw timestamped report remain ignored
local artifacts. No raw model output, prompts, repository source, credentials,
provider diagnostics, or machine-specific paths are committed.
