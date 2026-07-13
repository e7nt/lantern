# Lantern

Lantern is a provisional name for a lightweight, understanding-first AI coding
environment. It helps a developer enter an unfamiliar open-source repository,
build an evidence-backed mental model, learn the relevant execution paths, agree
on a plan with an agent, and implement changes without surrendering control.

The name reflects the product's role: illuminate the part of the system the
developer is exploring rather than attempting to replace the developer.

> **Naming status:** `Lantern` is a working codename. It has not undergone
> trademark, package-name, or domain clearance.

## Product principles

1. Understand before implementing.
2. Teach from real code and real execution paths.
3. Keep explanations grounded in files, symbols, tests, and runtime evidence.
4. Make plans durable, editable artifacts rather than disposable chat output.
5. Keep learning read-only and implementation explicitly permissioned.
6. Explain logical changes outside source files.
7. Let the developer interrupt, question, revise, or take over at any time.
8. Keep the agent runtime independent from the editor frontend.

## Initial product shape

The first implementation is expected to use:

- A thin VSCodium extension for selections, navigation, hovers, decorations,
  guided tours, plan presentation, and diffs.
- A separate local, Pi-inspired agent daemon for sessions, models, tools,
  permissions, repository understanding, learner state, plans, and change
  narratives.
- An editor-neutral protocol so a Helix or terminal frontend can be added later.

## Core experiences

- **Quick Ask:** select code and ask a short, context-aware question.
- **Learn:** follow a structured vertical slice with predictions, prerequisite
  branches, self-explanation, micro-tasks, and fading guidance.
- **Investigate:** understand the existing system and identify relevant evidence
  before suggesting a change.
- **Plan:** collaborate with the agent on a durable plan containing decisions,
  acceptance criteria, risks, tasks, and verification.
- **Guided Build:** watch implementation unfold in coherent, interruptible
  stages rather than receiving a complete patch at once.
- **Change Narrative:** hover over an agent-authored change to understand its
  intent, behavior, risks, plan relationship, and verification.
- **Review:** assess the result by acceptance criterion, logical change, file,
  raw diff, risk, and test evidence.

More detail is captured in:

- [docs/PRODUCT_BRIEF.md](docs/PRODUCT_BRIEF.md)
- [docs/GUIDED_BUILD.md](docs/GUIDED_BUILD.md)
- [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)
