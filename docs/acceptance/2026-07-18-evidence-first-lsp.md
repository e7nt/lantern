# Evidence-first LSP answers — 2026-07-18

Status: grounded zero-tool behavior passed on both pinned external repositories.
The cold three-second first-text target passed on Helix and remained above
budget on Lazygit at Pi's default medium reasoning level.

## Product change

For a saved symbol selection, Lantern now supplies:

- the selected line and two following lines;
- up to sixteen bounded definition lines;
- the existing bounded reference locations; and
- an explicit instruction to answer without tools when those facts are
  sufficient, or search only for a named missing fact.

The terminal ignores the already-visible selection for navigation and opens the
first resolved definition immediately. Later edit/write navigation remains
independent and still opens the changed Git hunk.

Unsaved selections retain the exact Helix-provided text rather than replacing
it with stale saved-file context.

## Isolated external results

| Case | Mode | Tools | First text | Settled | Three-second gate |
| --- | --- | ---: | ---: | ---: | --- |
| Helix definition flow | exact | 4 | 13,877 ms | 15,418 ms | not applicable |
| Helix definition flow | LSP | 0 | 2,372 ms | 2,934 ms | pass |
| Lazygit GUI construction | exact | 8 | 20,299 ms | 24,679 ms | not applicable |
| Lazygit GUI construction | LSP, medium run 1 | 0 | 3,456 ms | 4,811 ms | fail |
| Lazygit GUI construction | LSP, medium run 2 | 0 | 4,526 ms | 5,910 ms | fail |

Both LSP answers contained every curated fact, observed the expected typed
evidence, performed no tools, and left repository state unchanged. The earlier
Lazygit read was eliminated after Lantern added bounded call-site context.

## Reasoning-level diagnostic

The same isolated Lazygit case at Pi's supported low reasoning level remained
grounded, used zero tools, began text in 2,464 ms, and settled in 4,484 ms.

Lantern retains medium reasoning for now. One straightforward explanation case
does not establish that low reasoning preserves quality for incomplete
evidence, multi-step edits, verification, and recovery. The three-second gate
also remains unchanged; it was not raised to normalize the medium results.

## Next evidence required

Before dynamically lowering reasoning for a sufficient LSP question, add
curated cases that separate:

- fully sufficient selection/definition/reference evidence;
- evidence requiring one targeted repository read; and
- multi-step work where medium reasoning materially improves correctness.

Only adopt dynamic reasoning if repeated runs improve latency without reducing
grounding, tool discipline, edit quality, or recovery.

No raw answers, prompts, source dumps, credentials, provider diagnostics, or
machine-specific paths are committed.
