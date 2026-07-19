# Natural-language intent routing

## Outcome

Developers now speak naturally instead of selecting workflow commands:

- “What does this parser do?” uses understand mode.
- “Look into adding authentication; do not change anything yet” uses
  investigation mode.
- “Turn this into a plan” uses planning mode.
- “Proceed with the first task” uses implementation mode.

The same routing applies to repository, selection, symbol, and Git-review
questions. `/investigate` has been removed from the terminal and documentation.

## Tool boundary

Protocol v11 carries the inferred intent explicitly. Only `implement` reaches
the warm Pi coding profile with edit, write, and bash. Every other intent uses
a separate warm read-only Pi profile containing only read, grep, find, and ls.
Ambiguous language defaults to `understand`; no model prompt or repository text
can promote a turn to implementation after tool selection.

Investigation and planning output remains bounded in memory and is handed once
to the next explicit implementation turn. Understand turns retain continuity
inside the read-only Pi session. Neither path adds durable chat storage.

## Adoption record

Lantern retains Pi's explicit tool allowlists and separate settled boundary,
and OpenCode's principle that prompt admission precedes execution. It rejects a
model-only router because that would add latency and make mutation authority
nondeterministic. It also rejects keyword-triggered editing as the default:
informational and ambiguous phrasing resolves read-only.

## Evidence

- Rust tests cover questions containing words such as `add` and `implemented`,
  negated change requests, investigations, planning, explicit implementation,
  and the safe default.
- Daemon integration proves investigation receives the exact read-only tool
  allowlist, produces navigable evidence, leaves source byte-identical, and
  hands its brief once to an implementation turn.
- Protocol v11 golden fixtures require intent on repository, selection, and
  symbol agent requests and reject older shapes.
- The versioned DeepEval routing dataset includes ambiguous language,
  negation, planning, implementation, and repository prompt-injection cases.

No new command, panel, provider call, dependency, or fallback was introduced.
