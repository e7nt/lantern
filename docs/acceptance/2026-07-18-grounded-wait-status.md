# Grounded wait status — 2026-07-18

Status: accepted.

When Lantern has reopened and verified relevant source but Pi has not emitted
text yet, the compact activity line now reads `Found relevant code ·
thinking…`. The signal is derived from the existing typed evidence event, so it
adds no protocol concept, transcript entry, background work, or simulated
progress. Tool activity and response streaming continue to replace it normally.

The live trace now separates first verified evidence from first model text and
records their difference as provider wait. Three warm runs against pinned
p-limit used ready local semantic evidence and no tools:

| Run | First evidence | First text | Wait after evidence | Settled |
| --- | ---: | ---: | ---: | ---: |
| 1 | 36 ms | 2,270 ms | 2,234 ms | 7,535 ms |
| 2 | 33 ms | 2,408 ms | 2,375 ms | 4,518 ms |
| 3 | 27 ms | 2,323 ms | 2,296 ms | 5,295 ms |

The local grounding signal remained far below the one-second visible-activity
budget and every first-text measurement passed the strict three-second gate.
Most wait time was downstream of verified evidence, so adding another index,
cache, or speculative response path would add complexity without addressing the
measured bottleneck. Provider variability remains visible in evaluation output;
Lantern does not hide it with fake typing or a fallback model.

A cold run immediately after the semantic revision-format change correctly
returned no stale evidence and used repository tools while the new index built.
It began tool activity in 3,483 ms and failed the three-second activity gate.
That migration-only failure is retained in ignored local output and was not
reclassified as a passing ready-index turn.
