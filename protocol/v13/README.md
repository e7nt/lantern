# Lantern Protocol v13

Protocol v13 adds one bounded, multi-anchor review transaction for the active
implementation plan. The terminal may collect up to 32 comments locally. Each
comment contains a saved, exact selection from `.lantern/plans/active.md` and
up to 8 KiB of developer text; the complete review is capped at 64 KiB.

`review_plan` submits the collection in one read-only model turn. The daemon
rejects stale anchors, captures one complete revised plan, and emits it as a
`change_proposal`. Reviewing comments never edits the plan. Natural language
such as `Apply that` selects the internal `apply_plan_revision` intent; the
daemon applies the staged revision only when the active plan remains
byte-identical to the reviewed base and then emits `plan_revision_applied`.

Comments remain local until the developer asks Lantern to review them. Failed
or cancelled review turns preserve the local collection. No compatibility
fallback, partial application, mutating model tool, or hidden overwrite is
permitted.

All v12 framing, lifecycle, cancellation, trust, size, and strict-decoding
requirements remain in force. Requests are limited to 1 MiB and events to 256
KiB. Unknown methods, intents, events, and fields are hard errors; Lantern does
not negotiate an older protocol.
