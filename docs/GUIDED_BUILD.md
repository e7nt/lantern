# Guided Build

## Purpose

Guided Build is an optional implementation mode in which the agent constructs a
feature through visible, coherent stages. It is intended to preserve human
attention, understanding, and intervention—not to simulate human typing for its
own sake.

Passive character animation is not assumed to produce understanding. Guided
Build becomes valuable when the developer can predict, pause, inspect, question,
modify, reject, rewind, or take over.

## Core flow

```text
Select approved plan task
  -> state the next intent
  -> stage one coherent edit
  -> validate the staged edit
  -> play it into the visible editor
  -> allow questions or intervention
  -> create a durable checkpoint
  -> verify the result
  -> continue to the next task
```

The agent may reason ahead, but it must not silently complete the entire feature
and present a fake real-time implementation. It should work in the smallest
coherent increments that can be independently explained and verified.

## Chapters

A feature is divided into semantic chapters such as:

```text
1. Introduce the domain type
2. Extend the repository contract
3. Implement persistence
4. Integrate the domain behavior
5. Handle concurrency
6. Add tests
7. Run verification
```

Before each chapter, the editor presents:

- What the agent is about to change.
- Why this is the next logical step.
- Which plan task and acceptance criterion it serves.
- Which files and symbols are expected to change.
- Any meaningful risk or decision.

## Playback levels

| Level | Behavior | Intended use |
| --- | --- | --- |
| Keystroke | Characters appear progressively | Optional aesthetic mode |
| Line | Lines appear sequentially | Small functions and examples |
| Semantic | Coherent operations appear together | Default |
| Instant | Apply the validated chapter immediately | Mechanical or familiar work |

Suggested controls include `0.5x`, `1x`, `2x`, `4x`, `Step`, `Semantic`, and
`Instant`. The default should be semantic playback, with automatic slowing at
important decisions and collapsing of mechanical edits.

## User controls

At any point, the developer can:

- Pause or resume.
- Stop before the next edit is applied.
- Change playback speed.
- Step one operation at a time.
- Skip mechanical edits or the remainder of a chapter.
- Rewind an undoable chapter.
- Ask about the current line, operation, or decision.
- Ask what would happen without the change.
- Reject the current approach.
- Modify the code directly.
- Take control and continue manually.

Users can configure automatic pauses for architectural decisions, new
abstractions, public API changes, migrations, security-sensitive behavior, or
other risk categories.

## Staging and safety

Raw model tokens must not stream directly into repository files. The agent first
creates a small edit in a hidden staging buffer and validates its structure.
Playback then applies that edit to the visible editor.

Each chapter is an undoable transaction with:

- Before and after content hashes.
- Affected files and symbols.
- Ordered text operations.
- Plan and acceptance-criterion links.
- Validation and test results.
- A semantic explanation.

Visual playback may pause between characters or lines, but durable checkpoints
occur at coherent edit boundaries. Stopping prevents subsequent operations from
being applied and leaves the last durable checkpoint recoverable.

## User divergence

If the user modifies code during playback, their edit is preserved. The system
must detect whether the remaining staged operations are still applicable.

If the edit changes relevant structure or behavior:

1. Invalidate affected future operations.
2. Explain why the previous continuation is no longer safe.
3. Re-evaluate the remaining plan from the current repository state.
4. Propose any necessary plan amendment.
5. Continue only after the user accepts the revised direction.

The system must never overwrite a user edit merely to restore its original
playback timeline.

## Learning integration

Guided Build may pause at high-value moments and ask a short prediction or
self-explanation question. These prompts must be sparse, optional, and tied to
transferable design knowledge rather than filenames or syntax trivia.

Examples:

- Why is an application-level existence check insufficient under concurrency?
- Which layer should own this validation, and why?
- What test would distinguish the old behavior from the new behavior?

The user can answer, request a hint, reveal, or continue immediately.

## Relationship to change narratives

Every Guided Build chapter produces the semantic explanation later used by the
change overlay. The implementation timeline therefore becomes a reusable review
and onboarding artifact:

```text
Guided Build timeline
  -> review narrative
  -> semantic change hover
  -> future repository learning context
```

## Exclusions

The following should normally be applied instantly or collapsed:

- Formatting.
- Import organization.
- Generated files.
- Dependency lockfile noise.
- Mechanical renames.
- Repetitive test cases after the pattern is established.
- Tool-generated migrations that are reviewed as a single artifact.

## Product position

Guided Build does not claim that watching code appear is equivalent to writing
it. Its value comes from making agent implementation legible and interruptible
while connecting each edit to intent, evidence, planning, and verification.
