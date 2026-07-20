# Durable active plan acceptance

## Readiness gate

The feature was investigated against Lantern itself before persistence was
added. The existing path was ready:

- Natural-language routing was already centralized in
  `crates/protocol/src/lib.rs:139`.
- The read-only planning prompt already required objective, evidence,
  acceptance, exclusions, decisions, tasks, risks, and verification in
  `apps/daemon/src/main.rs:1168`.
- Completed planning output is transactionally captured and bounded in the
  cohesive plan module at `apps/daemon/src/plan.rs:45`.
- The terminal already owned natural-language submission and editor navigation
  at `frontend/terminal/src/main.rs:926`.

The remaining decisions were bounded: one local path, a minimal serialized
schema, create-new behavior, and an editor-open event. No planning database,
task dashboard, provider call, or general file-management UI was required.

## User outcome

After shaping a complete plan in conversation, the developer can say `Write
this down`. Lantern creates `.lantern/plans/active.md`, opens it in Helix, and
keeps the conversation's planning context available. The user never selects a
planning or persistence mode.

For later implementation turns, the current on-disk plan is authoritative.
Lantern reopens and validates it, including manual edits and after a process
restart, rather than using stale conversational text.

## Artifact contract

The active plan contains fixed `lantern_plan: 1` and `status: active` front
matter followed by ordinary Markdown. Persistence requires these headings:

- Objective
- Repository evidence
- Acceptance criteria
- Exclusions
- Decisions
- Tasks
- Risks and unknowns
- Verification

The daemon creates the file only after a successfully completed planning turn.
It rejects missing or incomplete context, validates the repository-local plan
directory, bounds captured content, flushes and syncs the file, removes a
partial file after a write failure, and never overwrites an existing plan.

## Reference decision

Pi revision `c6d8371521fc8357958bb21fd43552c15f46c7f4` persists broad JSONL
sessions under a user-level session hierarchy. Lantern adopts the principle
that durable state is explicit and resumable, but rejects Pi's transcript
format, session browser, migration surface, and ambient persistence. One
developer-editable Markdown artifact carries only the accepted plan.

## Verification

- Protocol v12 golden fixtures cover the typed `persist_plan` intent and
  `plan_saved` event.
- Intent-routing dataset v3 covers natural persistence phrases and preserves
  prompt-injection, ambiguity, refinement, and explicit-action regressions.
- Daemon integration proves plan creation requires a completed model turn,
  does not make another model request, and preserves the first file
  byte-for-byte when a duplicate save is rejected. The same journey edits the
  Markdown manually and proves the implementation prompt receives that edit.
- Unit tests cover schema completeness and absent plan context.

## Deliberate boundary

This slice supports one active plan, validates manual edits, and uses them for
implementation. It does not create structured revisions, comments, task state,
or semantic chapters. Those remain separate slices rather than growing this
artifact into a project-management system.
