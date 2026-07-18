# Evidence-first LSP answers — 2026-07-18

Status: grounded zero-tool behavior passed on both pinned external repositories.
Evidence-aware reasoning produced a 2,403 ms median first-text time over three
repeated Lazygit runs; one provider-latency outlier still failed the strict
three-second per-run gate.

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

Symbol-grounded turns start with Pi reasoning disabled. If Pi requests a tool,
Lantern raises reasoning to medium before accepting that tool's result. Lantern
then restores medium after the turn. Repository questions and multi-step coding
work stay at medium throughout. This boundary depends on observed typed evidence
and tool activity, not a natural-language intent classifier.

## Isolated external results

| Case | Mode | Tools | First text | Settled | Three-second gate |
| --- | --- | ---: | ---: | ---: | --- |
| Helix definition flow | exact | 4 | 13,877 ms | 15,418 ms | not applicable |
| Helix definition flow | LSP | 0 | 2,372 ms | 2,934 ms | pass |
| Lazygit GUI construction | exact | 8 | 20,299 ms | 24,679 ms | not applicable |
| Lazygit GUI construction | LSP, medium run 1 | 0 | 3,456 ms | 4,811 ms | fail |
| Lazygit GUI construction | LSP, medium run 2 | 0 | 4,526 ms | 5,910 ms | fail |
| Lazygit GUI construction | LSP, reasoning off run 1 | 0 | 3,315 ms | 5,513 ms | fail |
| Lazygit GUI construction | LSP, reasoning off run 2 | 0 | 2,403 ms | 4,362 ms | pass |
| Lazygit GUI construction | LSP, reasoning off run 3 | 0 | 2,229 ms | 4,323 ms | pass |

Both LSP answers contained every curated fact, observed the expected typed
evidence, performed no tools, and left repository state unchanged. The earlier
Lazygit read was eliminated after Lantern added bounded call-site context.

## Reasoning-level diagnostic

The same isolated Lazygit case at Pi's supported low reasoning level remained
grounded, used zero tools, began text in 2,464 ms, and settled in 4,484 ms. The
reasoning-off repetitions had a 2,403 ms first-text median and preserved every
curated answer term with zero tools.

Protocol tests prove both lifecycle branches: a sufficient-evidence answer is
restored directly from off to medium, while an answer that requests a tool is
raised to medium before the tool result and restored after settlement. Existing
repository coding journeys continue to run at medium reasoning.

## Incomplete-evidence baseline

Dataset v2 asks how Helix presents multiple definition locations. The supplied
selection and definition evidence deliberately omit `goto_impl`, `Picker`, and
`jump_to_location`, so a direct answer cannot satisfy the contract.

| Run | Tools | First activity | First text | Settled | Three-second gate |
| --- | ---: | ---: | ---: | ---: | --- |
| 1 | 3 (`grep`, `read`, `read`) | 3,586 ms | 10,176 ms | 11,581 ms | fail |
| 2 | 3 (`grep`, `read`, `read`) | 3,223 ms | 10,658 ms | 11,910 ms | fail |
| 3 | 3 (`grep`, `read`, `read`) | 2,932 ms | 7,570 ms | 10,036 ms | pass |

All three answers contained `Picker` and `jump_to_location`, completed without
mutation, and stayed within the three-tool ceiling. The escalation boundary is
working; the 3,223 ms median and repeated discovery/read sequence remain a
failing optimization target. The gate was not relaxed.

## Next evidence required

Continue expanding curated cases that separate:

- fully sufficient selection/definition/reference evidence;
- evidence requiring targeted repository work (now represented by the failing
  Helix multiple-location case); and
- multi-step work where medium reasoning materially improves correctness.

Keep the three-second gate strict and investigate observed outliers rather than
normalizing them. Retain dynamic reasoning only while grounding, tool discipline,
edit quality, and recovery continue to pass.

No raw answers, prompts, source dumps, credentials, provider diagnostics, or
machine-specific paths are committed.
