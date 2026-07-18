# Semantic evidence grouping — 2026-07-18

Status: accepted.

The terminal now groups repeated Protocol v9 semantic evidence by agent turn
and repository-relative file. A collapsed row reports the number of verified
locations and retains the highest-ranked range as its primary navigation
target. Definitions, calls, literal matches, and evidence from different turns
remain separate.

`Up` and `Down` cycle through visible evidence. `Enter` opens the selected exact
range. `Space` expands or collapses a multi-range semantic group. A mouse click
on the collapsed row expands it; a click on its expanded primary row collapses
it; individual expanded rows remain clickable navigation targets. Collapsing
while a secondary range is selected returns selection to the primary range.

The deterministic terminal tests prove:

- two semantic ranges from the same file and turn render as one collapsed row;
- the same path in a later turn remains a separate group;
- expansion restores every original raw evidence index and exact range;
- keyboard selection cycles only currently visible evidence;
- mouse expansion and collapse perform no accidental Helix navigation; and
- evidence source excerpts remain absent from transcript labels.

The daemon, Protocol v9 payloads, semantic retrieval, model prompt, and index
are unchanged. Grouping adds no request, scan, embedding, or provider work.
