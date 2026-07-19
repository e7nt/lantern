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
   boundaries using the trusted local workbench.
3. Build a code map whose claims are marked observed, inferred, unknown, or
   contradictory.
4. Learn one representative vertical execution path.
5. Ask selection-based questions without leaving the editor.
6. Request a feature and let the agent investigate the current behavior.
7. Review an understanding/readiness report before planning.
8. Collaborate on and refine a durable plan.
9. Implement the plan through interruptible Guided Build chapters.
10. Review semantic change explanations and verification evidence.
11. Optionally talk through exploration and implementation with an
    interruptible voice collaborator while preserving the same evidence,
    plans, visible operations, and checkpoints as the text experience.

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

Plan changes remain visible. When implementation discovers a material
constraint, the agent proposes a plan amendment rather than silently deviating.

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

## Trusted workbench model

Launching Lantern inside an explicitly chosen workbench grants the initial
single agent normal coding access: repository search and reads, edits, local
development commands, Git operations, and the configured model. Quick Ask,
Learn, Investigate, Plan, Implement, and Review describe user intent and
presentation—not separate permission profiles.

The agent narrates meaningful actions, shows edits and Git state through the
workbench, and remains immediately interruptible. Destructive Git history
operations require an explicit request. Voice is another interaction modality
over the same visible, interruptible agent session.

## Architecture direction

```text
Lantern terminal environment (Helix + Lantern pane + focused Git rail)
  -> local editor-neutral typed stdio protocol
      -> agent daemon
          -> model adapters
          -> typed workbench tools
          -> repository intelligence
          -> learning engine
          -> planning engine
          -> change narrative store
          -> hybrid repository index
```

The first frontend is a pinned Helix build with a narrow, documented Lantern
patch layer, a full-width terminal agent pane, and a focused Lantern Git rail.
Helix remains the editing and language-intelligence authority. The daemon
remains independent so agent execution and durable state do not become coupled
to editor internals. Pi is the initial harness behind a replaceable adapter.

## Initial scope boundaries

- Local repositories only.
- One trusted, visible, interruptible agent.
- macOS and Linux first.
- TypeScript and Rust as reference ecosystems.
- Hybrid LSP, exact, structural, Git, and semantic/vector retrieval with
  measured incremental indexing.
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
