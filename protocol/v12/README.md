# Lantern Protocol v12

Protocol v12 adds one internal `persist_plan` agent intent and one
`plan_saved` event to v11. Natural language remains the only agent-facing
workflow surface: phrases such as `write this down` select persistence without
adding a command or visible mode.

Persistence is deterministic and daemon-owned. It requires a successfully
completed in-memory planning turn, creates `.lantern/plans/active.md` with
create-new semantics, and emits its repository-relative path so the terminal
can open it in Helix. Existing plan files are never overwritten. No model,
provider, or tool fallback runs when persistence cannot proceed.

Once the file exists, it is authoritative for implementation turns. The daemon
reopens and validates the current bounded file, including developer edits,
instead of falling back to stale conversational text.

All v11 framing, lifecycle, cancellation, trust, size, and strict-decoding
requirements remain in force. Requests are limited to 1 MiB and events to 256
KiB. Unknown methods, intents, events, and fields are hard errors; Lantern does
not negotiate an older protocol.
