# Lantern product brief

Lantern's product and architecture decisions are governed by
[PRODUCT_CONSTITUTION.md](PRODUCT_CONSTITUTION.md).

## Problem

AI coding tools can generate changes quickly, but speed alone does not help a
developer understand an unfamiliar repository or retain ownership of the
result. Chat transcripts are weak planning artifacts, generated explanations
are often detached from evidence, and large patches make it difficult to see
how a feature was constructed.

Lantern is intended to help a developer understand code first and implement
second.

## Target user

The first user is an experienced, keyboard-oriented developer opening an
unfamiliar open-source repository. They value fast navigation, lightweight
operation, evidence, and control more than a large collection of IDE features.

They enjoy understanding and writing code. Lantern uses AI to deepen their
involvement in software development rather than remove them from it.

## Intended journey

1. Open or clone an unfamiliar repository.
2. Discover repository instructions, packages, entry points, tests, and runtime
   boundaries without executing untrusted code.
3. Build a code map whose claims are marked observed, inferred, unknown, or
   contradictory.
4. Learn one representative vertical execution path.
5. Ask selection-based questions without leaving the editor.
6. Request a feature and let the agent investigate the current behavior.
7. Review an understanding/readiness report before planning.
8. Collaborate on and approve a durable plan.
9. Implement the plan through interruptible Guided Build chapters.
10. Review semantic change explanations and verification evidence.
11. Optionally talk through exploration and implementation with an
    interruptible voice collaborator while preserving the same evidence,
    permissions, plans, and checkpoints as the text experience.

## Guided learning

The learning loop is:

```text
Orient -> Trace -> Predict -> Inspect -> Explain -> Apply -> Recall
```

The agent teaches through real code rather than a generated lecture. A learning
mission contains a small number of stops, each with:

- The current location and symbol.
- The subgoal served by the code.
- The important idea to notice.
- The surrounding details that can safely be ignored.
- The next handoff in the execution path.

The learner can branch into a prerequisite question and return to the precise
place where the main tour paused. Guidance fades from a fully explained tour to
navigation hints and finally a transfer task.

The product should support Tour, Navigator, and Challenge levels. Navigator is
the expected default for experienced developers.

## Feature investigation and planning

An implementation request begins with an editable feature brief and targeted
investigation. The agent identifies current behavior, analogous patterns,
affected interfaces, tests, risks, and unknowns. Planning begins only after an
explicit understanding gate.

The plan is a durable Markdown-backed document containing:

- Objective and acceptance criteria.
- Current-system summary with evidence.
- Constraints and open questions.
- Decisions and considered alternatives.
- Proposed data and control flow.
- Affected components and symbols.
- Implementation tasks and dependencies.
- Testing, migration, rollout, and documentation.
- Risks and unresolved assumptions.

Plan approval is granular. When implementation discovers a material constraint,
the agent proposes a plan amendment rather than silently deviating.

## Semantic change overlay

Agent-authored edits produce explanations outside source files. A concise hover
answers what changed and why. An expanded narrative includes:

- Behavior before and after.
- Design rationale and alternatives.
- Affected execution flow.
- Risks and assumptions.
- Related plan task and acceptance criterion.
- Tests and verification results.
- Other hunks participating in the same logical change.

Explanations attach to symbols, diff context, content hashes, and the Git base;
line numbers alone are insufficient. Subsequent edits trigger re-anchoring or a
staleness warning.

Formatting, generated files, import organization, and other mechanical edits
should normally be collapsed rather than individually explained.

## Agent policy model

One agent runtime operates under different enforced policies:

| Mode | Capabilities |
| --- | --- |
| Quick Ask | Read selection and related evidence; no edit or execution |
| Learn | Read, search, navigate, and trace; no repository modification |
| Investigate | Inspect repository and run separately approved diagnostics |
| Plan | Modify planning artifacts only |
| Implement | Modify approved scope and run permitted commands |
| Review | Inspect plans, diffs, and verification; read-only by default |

These boundaries must be enforced by the runtime, not merely described to the
model in a prompt.

Voice is a modality over these modes, not an additional privileged mode. A
spoken request receives exactly the capabilities available to the current
Quick Ask, Learn, Investigate, Plan, Implement, or Review session.

## Architecture direction

```text
Lantern terminal environment (Helix + Lantern pane + Lazygit)
  -> local editor-neutral RPC
      -> agent daemon
          -> model adapters
          -> policy and permission engine
          -> repository intelligence
          -> learning engine
          -> planning engine
          -> change narrative store
          -> execution sandbox
```

The first frontend is a pinned Helix build with a narrow, documented Lantern
patch layer, a full-width terminal agent pane, and a focused Lazygit rail.
Helix remains the editing and language-intelligence authority. The daemon
remains independent so policy, agent execution, and durable state do not become
coupled to editor internals.

## Initial scope boundaries

- Local repositories only.
- One agent with mode-specific policies.
- macOS and Linux first.
- TypeScript and Rust as reference ecosystems.
- LSP and syntax indexes before broad embedding-based retrieval.
- Markdown-backed plans rather than a general Notion clone.
- No multi-user cloud collaboration.
- No multi-agent orchestration in the first release.
- No visual webpage-element selection in the first release.
- No proprietary Lantern service or paid capability tier.
- No silent model, tool, retrieval, or execution fallbacks.

## Success criterion

A developer opening a substantial unfamiliar repository should be able to:

- Explain its major boundaries within 30 minutes.
- Trace one representative execution path with evidence.
- Identify where a requested feature belongs.
- Collaborate on a plan before code changes begin.
- Follow, interrupt, question, and review the implementation.
- Explain why the resulting logical changes exist and how they were verified.
