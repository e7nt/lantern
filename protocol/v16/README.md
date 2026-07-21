# Lantern Protocol v16

Protocol v16 makes the expanded Git canvas the sole owner of a code-review
draft. It can add, inspect, edit, and remove comments before one explicit
confirmation sends the complete batch through the private terminal control
socket. Each comment contains one modified or staged hunk, an exact zero-based
diff-line index and text, and up to 8 KiB of developer feedback. A review may
contain up to 32 comments and 128 KiB.

`review_code` admits the collection once. Before starting Pi, the daemon
re-reads the corresponding staged or unstaged Git diff and requires every hunk
to remain byte-identical. Stale comments fail visibly before model contact. The
coding profile then receives all comments, addresses them as one coherent
correction, and runs focused verification. Diff contents are labeled untrusted;
developer feedback remains the instruction.

After submission, the terminal becomes the sole owner and retains the batch
through rejection, provider failure, and cancellation. It consumes the batch
only after the correction emits `completed`.
The resulting edit/write paths use the existing focused Git handoff, so the
developer reviews a new diff. When an active plan exists, the v14 plan
checkpoint may follow the correction through its separate read-only profile.

The first contract intentionally supports modified and staged text hunks only.
Conflicts, untracked files, binary diffs, remote review threads, reviewer
identity, reactions, and automatic per-comment calls are excluded.

All v14 framing, lifecycle, cancellation, trust, size, and strict-decoding
requirements remain in force. Unknown methods, events, control requests, and
fields are hard errors; Lantern does not negotiate an older protocol.
