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

The terminal also retains the last successfully completed read-only intent for
the life of the workbench. Short refinements such as `Yes, but keep it bounded`
or `Only use the existing cache` continue that conversation; explicit language
such as `Do it` still starts implementation. Cancelled and failed turns never
replace this context. These typed intents remain an internal routing contract,
not user-facing modes or commands.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, intents,
event types, fields, grounding states, and evidence provenance are hard errors.
Lantern does not negotiate or fall back to an older protocol.
