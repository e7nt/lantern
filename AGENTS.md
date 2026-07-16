# Instructions for coding agents

These instructions apply to the entire Lantern repository.

## Start with the project principles

Before planning or changing code, read:

1. [Product constitution](docs/PRODUCT_CONSTITUTION.md)
2. [Engineering standard](docs/ENGINEERING_STANDARD.md)
3. [Implementation plan](docs/IMPLEMENTATION_PLAN.md) for current architecture,
   sequencing, and phase gates
4. [Reference repositories](docs/REFERENCE_REPOSITORIES.md) before designing a
   protocol, agent-loop, terminal interaction, editor, or Git boundary
5. [Protocol v3](protocol/v3/README.md) before changing the terminal
   client/daemon wire contract or operation lifecycle
6. [Diagnostic privacy contract](docs/DIAGNOSTICS.md) before adding logs,
   crash output, or diagnostic export fields

All decisions and changes must satisfy the product constitution. Every
deliverable must meet the engineering standard's Definition of Done and the
applicable phase exit criteria. If these documents conflict with a proposed
implementation, change the proposal—not the principles—unless the project has
explicitly amended the governing document.

## Working rules

- Build for developers who love to understand and write code.
- Choose the smallest coherent solution and resist feature, dependency, UI, and
  architectural bloat.
- Do not add silent fallbacks. Preserve root causes and fail with an actionable
  recovery step.
- Keep Lantern's Helix patch set narrow, documented, pinned, and removable.
- Inspect relevant upstream behavior before inventing a permanent interaction or
  protocol, then record what Lantern adopts and rejects instead of copying a
  reference project's product scope.
- Preserve the editor/daemon boundary and keep provider-specific behavior in
  adapters.
- Use strict types, explicit state transitions, bounded resources, and clear
  trust-boundary validation.
- Match tests to behavior: deterministic tests for software contracts and
  versioned DeepEval cases for model-mediated outcomes.
- Treat security, privacy, accessibility, performance, documentation, and
  reproducibility as completion requirements.
- Do not introduce speculative abstractions, compatibility paths, empty
  architectural scaffolding, or unrelated refactors.

## Before declaring work complete

- State the user outcome and acceptance criteria.
- Run the focused formatting, static analysis, type, unit, integration, and
  evaluation checks required by the changed area.
- Record meaningful results, limitations, risks, and deliberate follow-up work.
- Verify the diff contains no secrets, machine-specific paths, unexplained
  generated output, unrelated formatting churn, or silent exceptions.
- Do not claim success when a required check was skipped or failed.

Subdirectories may add an `AGENTS.md` with more specific instructions. Those
instructions extend this file and must not weaken the constitution or
engineering standard.
