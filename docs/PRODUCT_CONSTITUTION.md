# Lantern product constitution

## Purpose

This document is Lantern's decision filter. Product, architecture, design, and
roadmap decisions must be justified against it. When a proposal conflicts with
these principles, the proposal changes—not the principles—unless the project
explicitly amends this document.

## Position

> **Lantern is an open-source AI coding environment for developers who love to
> understand and write code.**

Lantern helps developers explore unfamiliar systems, reason through changes,
plan with evidence, and build software alongside an interruptible AI
collaborator without giving up authorship or understanding.

Our concise promise is:

> **Understand the code. Shape the plan. Build it together.**

Lantern does not measure success by how completely it removes the developer
from programming. It succeeds when the developer understands the system,
makes better decisions, writes or shapes better code, and can explain the
result.

## Commitments

### Open source only

- Lantern's core editor, daemon, protocol, schemas, and first-party features
  remain open source.
- Core workflows do not require a Lantern subscription or mandatory hosted
  Lantern service.
- Repository understanding, plans, learning state, and audit state remain local
  by default.
- Users may bring a remote model provider or choose supported local models.
- Provider-specific features are optional adapters, never the foundation of
  Lantern's product identity.
- A feature that requires proprietary Lantern infrastructure is outside the
  project's direction.

Open source does not prohibit donations, sponsorship, grants, or paid support.
It prohibits withholding core product capability behind a proprietary Lantern
service or paid product tier.

### Understanding before automation

- Explanations are grounded in inspectable evidence.
- Investigation precedes planning; planning precedes implementation.
- Uncertainty is shown rather than smoothed into a confident narrative.
- The developer can ask why, interrupt, revise, reject, or take over.
- Agent-created changes remain understandable in terms of intent, behavior,
  risk, and verification.

### The developer remains an author

- Lantern encourages reading, reasoning, predicting, editing, and reviewing.
- Direct user edits are first-class and are never overwritten to restore an
  agent timeline.
- Guided Build exposes coherent decisions and changes instead of presenting a
  finished patch as magic.
- Voice collaboration feels like pairing at a shared screen, not delegating to
  an invisible replacement.
- Manual coding is always supported; Lantern must not make ordinary editing
  worse in order to promote agent usage.

### Trusted workbench, visible agency

- Launching Lantern for an explicitly chosen workbench starts a trusted local
  coding session rather than a capability-configuration workflow.
- The initial agent may read, edit, run development commands, use Git, and
  contact the selected model without repeated permission prompts.
- Meaningful operations remain visible and immediately interruptible.
- Destructive Git history operations require an explicit developer request.
- Full access does not authorize invisible background autonomy, credential
  exposure, or silent expansion beyond attached workbench folders.

### Build for the love of coding

- Interactions should preserve curiosity, flow, craft, and satisfaction.
- Lantern teaches from real systems rather than replacing exploration with
  generated lectures.
- The product should make difficult code more approachable without pretending
  it is simple.
- Speed matters, but not at the expense of understanding, correctness, or
  ownership.
- Features should respect experienced developers' attention and intelligence.

### Resist bloat

Every new feature must answer:

1. Which core developer problem does it solve?
2. How does it improve understanding, authorship, control, or verification?
3. Can the same value be delivered through a smaller mechanism?
4. What ongoing conceptual, UI, runtime, security, and maintenance cost does it
   introduce?
5. What existing feature can be removed or simplified because of it?

A feature does not enter the roadmap because competitors have it, a provider
supports it, or it is technically interesting. Evaluation tracks must have
promotion gates and a clear deletion path.

### Avoid fallback stacks

- Prefer one well-understood primary path over layers of compatibility behavior.
- Do not silently change models, tools, policies, data sources, or execution
  strategies when the selected path fails.
- Fail clearly with the actual cause and an actionable recovery step.
- Do not return a lower-confidence answer while presenting it as equivalent.
- Do not preserve obsolete implementations indefinitely after a replacement is
  accepted.
- Optional adapters must expose their limitations rather than emulate missing
  capabilities poorly.

Intentional graceful degradation is allowed only when it is visible, preserves
correctness, and remains a designed product state. For example,
Quick Ask may report that symbol intelligence is unavailable and operate on the
selection alone, but it must not imply that symbol-backed investigation
occurred.

### Keep boundaries narrow

- The Helix patch set stays small, documented, pinned, and justified by product
  blockers.
- The editor owns presentation and editor-native transactions.
- The daemon owns agent execution, tool coordination, evidence, and durable
  state.
- Provider contracts stay replaceable and provider-specific behavior stays in
  adapters.
- Persist only artifacts that help the developer understand, resume, audit, or
  review work.

### Treat code quality as product quality

- Lantern's source should teach by example: readable, explicit, and worthy of
  careful public review.
- Correctness, security, accessibility, performance, and maintainability are
  release requirements rather than deferred cleanup.
- Prefer small cohesive modules, strict types, and visible invariants over
  clever abstractions or speculative frameworks.
- Errors preserve their cause and provide an actionable recovery step; they are
  never swallowed to make a workflow appear successful.
- Tests match the behavior: deterministic tests protect software contracts and
  DeepEval scenarios evaluate model-mediated outcomes.
- Dependencies, generated artifacts, and upstream patches remain reproducible,
  attributable, and auditable.
- Code is complete only when it meets the project-wide Definition of Done in
  [ENGINEERING_STANDARD.md](ENGINEERING_STANDARD.md).

## Decision test

Before accepting a material decision, record:

| Question | Required answer |
| --- | --- |
| Does it help developers understand or write code? | A concrete user outcome |
| Does it preserve authorship and interruption? | The user control mechanism |
| Is it the smallest coherent solution? | Rejected smaller alternatives |
| Does it add a fallback or parallel path? | Why that path is unavoidable, visible, and removable |
| Can it remain open and local-first? | Dependencies and external services |
| What evidence will validate it? | Spike, test, measurement, or user study |
| What causes us to remove it? | Explicit failure or deprecation condition |

Decisions that cannot answer these questions remain experiments rather than
architecture commitments.

Model-mediated features require behavioral evaluations as described in
[EVALUATION_STRATEGY.md](EVALUATION_STRATEGY.md). Deterministic software tests
alone are not sufficient evidence for explanation, learning, planning,
narration, or agent-flow quality.

## Roadmap implications

- The full-access Pi harness must prove one evidence-backed coding journey
  before broader product modes.
- Guided learning is a core capability, not decorative onboarding.
- Planning remains a durable, developer-editable artifact.
- Guided Build optimizes for legibility and intervention, not agent spectacle.
- Live Collaboration advances only if it improves understanding and control.
- Multi-agent orchestration, autonomous background work, dashboards, social
  features, and provider marketplaces remain out of scope unless they directly
  satisfy this constitution with less complexity than alternatives.
- A smaller, coherent v0.1 is preferable to a broad editor with shallow AI
  features.

## Amendment rule

Changes to this constitution require:

1. A written motivation.
2. The user problem that cannot be served under the current principles.
3. Considered alternatives.
4. Consequences for openness, authorship, simplicity, and fallback behavior.
5. An explicit project decision rather than an incidental implementation
   choice.

Accepted amendments are recorded in
[ADR 003](decisions/003-trusted-workspace-default.md) and later decision
records. Historical read-only spikes remain evidence, not current direction.
