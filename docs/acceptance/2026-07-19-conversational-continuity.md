# Conversational continuity acceptance

## Outcome

A developer can explore a change, refine it with short natural-language
follow-ups, and say `Do it` without selecting a workflow or restating the
accepted constraints. Internal intent names remain absent from the interface.

## Smallest coherent design

Lantern retains only the typed intent of the last successfully completed agent
turn in the terminal process. Explicit language is classified first. A bounded
set of conversational continuations inherits a preceding exploratory or
planning intent. Pi's existing warm read-only session retains dialogue, and
the existing bounded one-time handoff supplies its latest result to the coding
profile.

No provider call, dependency, command, persistent record, protocol field, or
fallback path was added. Cancelled and failed turns cannot replace completed
context.

## Reference decision

At Pi revision `c6d8371521fc8357958bb21fd43552c15f46c7f4`, Lantern adopts the
stateful conversation behavior behind Pi's follow-up handling. It rejects a
visible message queue, session tree, and steering vocabulary because one
continuous composer serves Lantern's narrower developer experience.

## Acceptance criteria

- `Yes, but keep the cache bounded` continues an investigation.
- `Also include focused verification` continues planning.
- `Do it` explicitly starts implementation.
- An ambiguous prompt without completed context remains read-only.
- A cancelled turn does not become future context.
- Selection, symbol, Git-review, and repository prompts share the same rule.
- Navigable evidence is labeled `Relevant code`, not with an internal workflow
  name.

## Verification

Rust unit tests cover deterministic inference and terminal lifecycle state. The
versioned DeepEval contract is `evaluations/datasets/intent_routing/v2.json`.
