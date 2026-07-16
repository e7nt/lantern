# Lantern

Lantern is a provisional name for a lightweight, understanding-first AI coding
environment. It helps a developer enter an unfamiliar open-source repository,
build an evidence-backed mental model, learn the relevant execution paths, agree
on a plan with an agent, and implement changes without surrendering control.

> **Lantern is an open-source AI coding environment for developers who love to
> understand and write code.**
>
> **Understand the code. Shape the plan. Build it together.**

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
9. Keep Lantern's core product open source and local-first.
10. Prefer the smallest coherent feature and resist product bloat.
11. Prefer one explicit primary path; avoid silent or indefinite fallback
    stacks.
12. Build for developer authorship, curiosity, and the love of coding.

## Initial product shape

The first implementation uses:

- A pinned Helix editor with a narrow, documented Lantern patch layer, a
  compact Lazygit rail, and a full-width terminal agent pane.
- A separate local, Pi-inspired agent daemon for sessions, models, tools,
  permissions, repository understanding, learner state, plans, and change
  narratives.
- An editor-neutral protocol that keeps policy and model execution out of the
  editor process.

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
- **Live Collaboration (evaluation):** talk through learning, planning, building,
  and review with an interruptible voice collaborator grounded in visible code
  and durable session state.

More detail is captured in:

- [docs/PRODUCT_BRIEF.md](docs/PRODUCT_BRIEF.md)
- [docs/GUIDED_BUILD.md](docs/GUIDED_BUILD.md)
- [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)
- [docs/REFERENCE_REPOSITORIES.md](docs/REFERENCE_REPOSITORIES.md)
- [docs/PHASE_0_DOSSIER.md](docs/PHASE_0_DOSSIER.md)
- [docs/LIVE_COLLABORATION.md](docs/LIVE_COLLABORATION.md)
- [docs/PRODUCT_CONSTITUTION.md](docs/PRODUCT_CONSTITUTION.md)
- [docs/EVALUATION_STRATEGY.md](docs/EVALUATION_STRATEGY.md)
- [docs/FIRST_USEFUL_SLICE.md](docs/FIRST_USEFUL_SLICE.md)

## Contributor verification

Lantern's maintained Rust code is one workspace with four explicit owners:
`crates/protocol` defines the wire contract, `crates/policy-engine` owns
capability enforcement, `apps/daemon` owns agent execution, and
`frontend/terminal` owns the developer-facing terminal surface.

Run its complete deterministic gate from the repository root:

```bash
cargo fmt --all --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release --locked
```

The reproducible Helix/Lazygit environment and its launch command remain
documented in [frontend/helix/README.md](frontend/helix/README.md).
