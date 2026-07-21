# Lantern Protocol v14

Protocol v14 closes the implementation-to-plan loop without adding a task
system. When an implementation turn has an active plan and leaves a reviewable
diff through reported edit or write tools, the daemon emits
`plan_progress_started` and asks the read-only Pi profile for one complete plan
checkpoint. The checkpoint receives only the current plan, bounded final
implementation summary, successful-verification presence, and Git diff for the
reported edited paths.

The checkpoint is emitted as the existing `change_proposal`; it never edits the
plan. Natural `Apply that` uses the v13 byte-identical base guard. A checkpoint
failure emits `plan_progress_failed` with its actual cause and recovery while
preserving the successful code result. Turns with no reviewable agent edit do
not manufacture a checkpoint.

The coding profile is explicitly instructed not to edit the active plan. The
checkpoint profile exposes only read, grep, find, and list tools. It must
preserve unaffected plan detail, mark only diff-supported work complete, record
material divergence or risk, and never invent verification details.

All v13 framing, review, lifecycle, cancellation, trust, size, and
strict-decoding requirements remain in force. Unknown methods, intents, events,
and fields are hard errors; Lantern does not negotiate an older protocol.
