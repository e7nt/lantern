# Contributing to Lantern

Lantern is built for developers who want to understand and author code with an
agent, not hand authorship over to it. Contributions should make that loop
faster, clearer, or more trustworthy without adding speculative surface area.

## Supported development environment

The current contributor path targets Linux. Windows is not supported yet, and
macOS has not passed the complete workbench journey. Please do not add an
untested compatibility fallback to make another platform appear supported.

Required tools:

- Git
- a current stable Rust toolchain with Rust 2024 edition support
- Node.js 22
- Python 3.12
- [uv](https://docs.astral.sh/uv/) 0.11 or newer
- tmux 3.2 or newer for the interactive workbench
- Pi 0.80.6 for live agent use; provider credentials are never required for CI

## Prepare a checkout

Clone Lantern, then prepare its pinned Helix frontend, Rust runtime, Python
environment, and semantic model:

```bash
git clone https://github.com/e7nt/lantern.git
cd lantern
./frontend/helix/prepare.sh
```

The preparation script fetches exactly the Helix revision recorded in
`frontend/helix/upstream.json`, applies Lantern's audited patch inventory, and
uses locked Rust and Python dependencies. It fails on revision or patch drift.

To run the product, authenticate Pi privately with `/login`, then launch a Git
repository:

```bash
./scripts/launch-lantern.sh /path/to/repository
```

## Verify a change

Run the complete non-provider gate:

```bash
./scripts/check.sh
```

During development, run one or more focused suites:

```bash
./scripts/check.sh rust terminal
./scripts/check.sh evaluations
./scripts/check.sh semantic-index
```

The script is the canonical local and CI entry point. It installs only locked
Python dependencies. Live provider evaluations remain explicit because they
require private authentication and may incur cost.

## Propose a change

Before coding, read `AGENTS.md`, `docs/CURRENT_STATE.md`, and the product and
engineering standards they reference. Open an issue before work that changes a
protocol boundary, adds a dependency, broadens platform support, or introduces
a new permanent UI surface.

A pull request should state:

1. the developer outcome and why the change is the smallest coherent solution;
2. important decisions and rejected alternatives;
3. verification commands and meaningful results;
4. limitations, risks, and deliberate follow-up work;
5. an interaction recording or screenshots for visual changes.

Keep commits reviewable. Do not include credentials, source captured from a
private workbench, machine-specific paths, generated caches, or unrelated
formatting changes.

## Reporting problems

Use a GitHub issue for reproducible bugs and focused product proposals. Use the
private process in `SECURITY.md` for vulnerabilities. Diagnostic exports must
follow `docs/DIAGNOSTICS.md`; never paste provider stderr, prompts, repository
source, credentials, or an unreviewed environment dump into an issue.
