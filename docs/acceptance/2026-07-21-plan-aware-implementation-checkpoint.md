# Plan-aware implementation checkpoint acceptance — 2026-07-21

## User outcome

After a developer says `Proceed with the first task`, a successful coding turn
still ends with the ordinary code result and focused Git review. When the turn
reported an edited path and Git contains a reviewable diff, Lantern then shows
`Preparing the plan checkpoint…` and stages a separate full-plan diff. The
developer can inspect it and say `Apply that`; declining it leaves the active
plan untouched.

## Smallest coherent design

Lantern reuses the existing active Markdown plan, persistent Pi profiles,
`ChangeProposal` preview, and byte-identical apply guard. It does not introduce
task identifiers, a scheduler, status storage, prose parsing, or a second plan
format. The coding profile is told never to edit the active plan. The read-only
profile drafts the checkpoint from four bounded facts:

- the current active plan;
- the final implementation summary;
- whether any development command succeeded, explicitly labeled as insufficient
  evidence of verification by itself;
- the Git diff for agent-reported edit/write paths only.

Unrelated developer changes are not included. A turn with no reviewable agent
edit creates no checkpoint.

## Contracts proved

- Protocol v14 exposes explicit checkpoint-start and checkpoint-failure states.
- The checkpoint profile has read, grep, find, and list tools but no mutation or
  shell access.
- The original plan remains byte-identical until explicit application.
- Manual edits after proposal generation make application fail visibly.
- A checkpoint failure does not misreport the successful code turn as failed;
  its cause and manual/comment-review recovery remain visible.
- Deterministic Git fixtures prove touched-path isolation and end-to-end staged
  application.
- DeepEval contracts reject completion of unsupported tasks, unrelated paths,
  invented broad verification, incomplete plans, and wrapper text.

## Deliberate limits

Lantern records only that a successful development command occurred because raw
commands and outputs do not cross the privacy-preserving UI boundary. That bit
does not prove verification. The model may retain a specific verification named
in its bounded completion summary, but must not invent one. Checkpoints are
generated only for edit/write paths reported by Pi; opaque file mutation
performed inside a shell command does not qualify.
