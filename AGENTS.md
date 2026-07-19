# Instructions for coding agents

These instructions apply to the entire Lantern repository.

## Start with the project principles

Before planning or changing code, read:

1. [Current state](docs/CURRENT_STATE.md) for what exists, what is
   transitional, and what to implement next
2. [Product constitution](docs/PRODUCT_CONSTITUTION.md)
3. [Engineering standard](docs/ENGINEERING_STANDARD.md)
4. [Implementation plan](docs/IMPLEMENTATION_PLAN.md) for current architecture,
   sequencing, and phase gates
5. [Reference repositories](docs/REFERENCE_REPOSITORIES.md) before designing a
   protocol, agent-loop, terminal interaction, editor, or Git boundary
6. [Protocol v10](protocol/v10/README.md) before changing the terminal
   client/daemon wire contract or operation lifecycle
7. [Diagnostic privacy contract](docs/DIAGNOSTICS.md) before adding logs,
   crash output, or diagnostic export fields
8. [Provider credential contract](docs/CREDENTIALS.md) before changing model
   authentication, provider selection, or driver process boundaries

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
- Preserve Protocol v10's single trusted-workbench path. Do not reintroduce a
  capability ceremony or reduced-function fallback.
- Use Pi as the initial harness and measure hybrid retrieval per ADR 004. Do not
  create a parallel native agent loop without a recorded revisit condition.
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
