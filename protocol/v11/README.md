# Lantern Protocol v11

Protocol v11 replaces v10's exposed investigation method with one typed
`intent` on every agent turn: `understand`, `investigate`, `plan`, or
`implement`. The terminal infers this intent from the developer's natural
language before the daemon selects tools.

Only `implement` receives Pi's coding profile. Every other intent uses the
read-only `read`, `grep`, `find`, and `ls` profile. Ambiguous language resolves
to `understand`, so uncertainty cannot silently grant mutation tools.
Investigation and planning briefs are retained only as bounded in-memory
context for the next explicit implementation turn.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, intents,
event types, fields, grounding states, and evidence provenance are hard errors.
Lantern does not negotiate or fall back to an older protocol.
