# Lantern current state

This is the short implementation entry point for a new contributor. Read it
after `AGENTS.md` and before choosing work from the full roadmap.

## Product direction

Lantern is an open-source, lightweight coding workbench for developers who love
to understand and write code. The primary surface is Helix above a full-width
agent pane, with Lazygit available as a narrow rail. Pi is the initial agent
harness. A launched workbench is trusted by default: the agent is intended to
search, edit, run development commands, and use Git while narrating meaningful
work and remaining immediately interruptible.

Answers should be natural and concise. Lantern should quietly ground them with
Helix/LSP evidence, exact search, syntax and Git structure, semantic/vector
retrieval, and commit-synchronized summaries. Internal protocol, provenance,
and tool vocabulary should not become user-facing ceremony.

## What works today

- Reproducible pinned Helix and Lazygit preparation.
- The 80/20 Helix/agent composition and on-demand 10% Lazygit rail.
- Mouse and keyboard interaction across the surfaces.
- A `Ctrl-a` contextual composer over Helix and reversible `F2` full-screen
  reading mode for the persistent agent pane.
- Bounded, typed composer submission over a private session-local Unix socket;
  tmux owns presentation and focus but never transports questions.
- Maintained Rust terminal, daemon, diagnostics, and Protocol v6 crates.
- Selection capture, exact navigation, bounded local literal search, and
  Helix-provided definition/reference context.
- Trusted-workbench initialization with repository-bound requests and no
  capability ceremony.
- Selection- and symbol-grounded Pi RPC questions using Pi-owned OpenAI Codex
  authentication.
- Repository-grounded Pi questions from the empty prompt; editor context is an
  optional accelerator and never a prerequisite for talking to the agent.
- Pi's pinned `read`, `grep`, `find`, `ls`, `edit`, `write`, and `bash` tools,
  launched inside the repository with typed activity in Lantern.
- Successful edit/write activity opens the changed file in Helix; `Space-g` or
  `/git` opens the focused Git review surface.
- Streaming, cancellation, crash survival, explicit local diagnostics, and
  typed evidence provenance.
- Deterministic software tests and versioned DeepEval contracts.

## Current boundary

Protocol v6 and the terminal open one trusted repository directly. The old
policy engine, capability fields, and `/trust` commands have been removed. Pi
runs its explicit built-in coding-tool allowlist in that repository. Raw tool
arguments, command output, and provider stderr are not copied into Lantern's
bounded UI protocol or diagnostics.

SQLite remains deferred by
[ADR 002](decisions/002-defer-sqlite-until-needed.md). There is no durable
session database or ad-hoc JSON replacement.

## Implement next

Complete the external edit journey defined in
[EXTERNAL_EDIT_JOURNEY.md](EXTERNAL_EDIT_JOURNEY.md):

1. Prove one live journey: ask for a small change, inspect the relevant code, edit
   it, run its focused test, and show the resulting diff.
2. Extend the DeepEval dataset from deterministic tool-order contracts to
   recorded live traces for natural explanation, grounding, and interruption.
3. Tighten changed-file navigation from file start to Pi's exact edit range
   when the pinned harness exposes that range reliably.

After that journey works, spike the incremental hybrid repository index. Start
with measured LSP/exact baselines, then add semantic/vector retrieval and
commit-synchronized summaries only where they improve real repository
questions.

## Not next

- SQLite or durable chat history.
- Voice collaboration.
- Multi-agent orchestration.
- A custom editor renderer or broad Helix fork.
- Multi-folder workbench implementation; preserve the roadmap design and
  `WORKBENCH.md` direction, but first make one repository excellent.
- A native agent loop parallel to Pi.
- General settings, provider marketplaces, or permission frameworks.

## Required checks

Run the repository gates documented in `README.md`. Model-mediated behavior
also requires the deterministic DeepEval contract suite. A live provider run is
recorded separately and must never be required for ordinary open-source CI.

## Authority of documents

- `PRODUCT_CONSTITUTION.md` defines product values.
- ADRs record accepted architectural changes.
- This file states current implementation truth and immediate sequence.
- `IMPLEMENTATION_PLAN.md` contains the longer roadmap.
- `FIRST_USEFUL_SLICE.md`, `PHASE_0_DOSSIER.md`, and spike documents preserve
  historical reasoning; they do not override accepted ADRs or this file.
