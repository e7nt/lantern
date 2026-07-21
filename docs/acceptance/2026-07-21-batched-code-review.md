# Batched code-review acceptance — 2026-07-21

## User outcome

After an agent edit, the developer opens focused Git and reviews the actual
diff. Within a modified or staged hunk:

1. `j/k` or a left click selects an exact diff line.
2. `c` or a right click opens the review-comment input.
3. Enter queues the comment in the agent pane without contacting Pi.
4. The developer repeats across files and hunks; `[` and `]` move between
   hunks.
5. `R` or a footer click submits the complete review once and closes the rail.
6. The coding profile addresses every comment as one correction, verifies it,
   and focused Git receives the new edited-file handoff.

The terminal owns the in-session collection, so closing and reopening the Git
rail does not discard already delivered comments. The rail's `+N` count covers
comments added during that rail view; submission remains available after a
reopen.

## Contracts proved

- Protocol v15 bounds 32 comments, 8 KiB per comment, and 128 KiB total.
- Every anchor contains one typed Git state, hunk bytes, exact diff-line index,
  and exact line text.
- Only modified and staged textual code lines are accepted initially.
- The daemon re-reads Git and rejects any stale hunk before model contact.
- Diff contents are untrusted evidence; developer feedback is the instruction.
- Comments survive rejection, provider failure, and interruption and are
  consumed only by a completed correction.
- One integration fixture submits two line comments, observes one coding turn,
  verifies the correction, and leaves a new reviewable diff.
- DeepEval contracts reject partial review responses and invented broad
  verification.

## Deliberate limits

The first slice excludes untracked files, conflicts, binary diffs, multi-line
range selection, remote review threads, reviewer identity, reactions, and
durable review history. Comments are not declared resolved from model prose;
the new diff is the evidence the developer reviews.
