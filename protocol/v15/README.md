# Lantern Protocol v15

Protocol v15 adds one local, batched code-review transaction over exact Git
diff lines. The focused Git rail may send up to 32 comments through the private
terminal control socket. Each comment contains one modified or staged hunk, an
exact zero-based diff-line index and text, and up to 8 KiB of developer
feedback. The complete review is capped at 128 KiB.

`review_code` admits the collection once. Before starting Pi, the daemon
re-reads the corresponding staged or unstaged Git diff and requires every hunk
to remain byte-identical. Stale comments fail visibly before model contact. The
coding profile then receives all comments, addresses them as one coherent
correction, and runs focused verification. Diff contents are labeled untrusted;
developer feedback remains the instruction.

The terminal retains comments through rejection, provider failure, and
cancellation. It consumes them only after the correction emits `completed`.
The resulting edit/write paths use the existing focused Git handoff, so the
developer reviews a new diff. When an active plan exists, the v14 plan
checkpoint may follow the correction through its separate read-only profile.

The first contract intentionally supports modified and staged text hunks only.
Conflicts, untracked files, binary diffs, remote review threads, reviewer
identity, reactions, and automatic per-comment calls are excluded.

All v14 framing, lifecycle, cancellation, trust, size, and strict-decoding
requirements remain in force. Unknown methods, events, control requests, and
fields are hard errors; Lantern does not negotiate an older protocol.
