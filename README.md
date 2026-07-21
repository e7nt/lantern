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

> **Project status:** Lantern is an early developer preview. The maintained
> contracts are tested in CI, and tagged Homebrew releases are available for
> supported Apple Silicon and Intel Macs.

## Try Lantern

The current path requires Git, Rust, Node.js 22, Python 3.12, uv, tmux 3.2 or
newer, and Pi 0.80.6. From a clean checkout:

```bash
./frontend/helix/prepare.sh
./scripts/launch-lantern.sh /path/to/a/git/repository
```

Preparation fetches one pinned Helix revision, applies Lantern's audited patch
set, builds the locked Rust workspace, and installs the locked semantic worker.
Pi authentication remains private; start `pi`, use `/login`, and choose OpenAI
Codex before launching Lantern. Every developer uses their own Pi-managed
OpenAI identity; Lantern does not bundle or share an API key, read the resulting
credential, or fall back to another provider. See
[the frontend guide](frontend/helix/README.md) for the complete interaction.

The supported macOS install and upgrade path is:

```bash
brew install e7nt/tap/lantern
lantern help
brew upgrade lantern
```

Maintainers should follow [the release contract](docs/RELEASING.md); ordinary
commits never publish packages or update the tap.

## Product principles

1. Understand before implementing.
2. Teach from real code and real execution paths.
3. Keep explanations grounded in files, symbols, tests, and runtime evidence.
4. Make plans durable, editable artifacts rather than disposable chat output.
5. Start from a trusted workbench; keep agent operations visible and
   immediately interruptible.
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
  focused Lantern Git rail, and a full-width terminal agent pane.
- A separate local daemon that connects the Pi agent harness to typed workbench
  tools, repository intelligence, plans, and change narratives.
- An editor-neutral protocol that keeps agent and model execution out of the
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

- [docs/CURRENT_STATE.md](docs/CURRENT_STATE.md)
- [docs/decisions/003-trusted-workspace-default.md](docs/decisions/003-trusted-workspace-default.md)
- [docs/decisions/004-pi-harness-hybrid-retrieval.md](docs/decisions/004-pi-harness-hybrid-retrieval.md)
- [docs/PRODUCT_BRIEF.md](docs/PRODUCT_BRIEF.md)
- [docs/GUIDED_BUILD.md](docs/GUIDED_BUILD.md)
- [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)
- [docs/REFERENCE_REPOSITORIES.md](docs/REFERENCE_REPOSITORIES.md)
- [docs/PHASE_0_DOSSIER.md](docs/PHASE_0_DOSSIER.md)
- [docs/LIVE_COLLABORATION.md](docs/LIVE_COLLABORATION.md)
- [docs/PRODUCT_CONSTITUTION.md](docs/PRODUCT_CONSTITUTION.md)
- [docs/EVALUATION_STRATEGY.md](docs/EVALUATION_STRATEGY.md)
- [docs/FIRST_USEFUL_SLICE.md](docs/FIRST_USEFUL_SLICE.md)
- [docs/DIAGNOSTICS.md](docs/DIAGNOSTICS.md)
- [docs/CREDENTIALS.md](docs/CREDENTIALS.md)

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) for the supported environment, clean
checkout setup, pull-request expectations, and safe reporting guidance. Report
vulnerabilities privately according to [SECURITY.md](SECURITY.md).

## License

Lantern is licensed under the
[GNU Affero General Public License v3.0 only](LICENSE). Commercial use is
permitted, but distribution and modified network services must satisfy the
license's corresponding-source requirements. Contributions are accepted under
the same license without copyright assignment.

The code license does not grant permission to imply endorsement by the Lantern
project or its maintainers. `Lantern` remains a working codename that has not
undergone trademark clearance.

## Contributor verification

Lantern's maintained Rust code is one workspace with explicit owners:
`crates/protocol` defines the wire contract, `crates/diagnostics` owns safe
diagnostic records and exports, `apps/daemon` owns agent execution,
`apps/explorer` owns workbench navigation, `apps/git-rail` owns focused review,
and `frontend/terminal` owns the developer-facing agent surface.

Run every non-provider check through the same entry point used by CI:

```bash
./scripts/check.sh
```

Focused suites are also available:

```bash
./scripts/check.sh rust terminal
./scripts/check.sh evaluations semantic-index
```

Live provider evaluation is explicit and credential-dependent; it is not part
of ordinary contributor CI. See
[evaluations/README.md](evaluations/README.md) for the separate command.

The reproducible Helix/Lantern environment and its launch command remain
documented in [frontend/helix/README.md](frontend/helix/README.md).
