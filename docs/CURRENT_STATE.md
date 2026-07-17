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
- Maintained Rust terminal, daemon, diagnostics, and Protocol v5 crates.
- Selection capture, exact navigation, bounded local literal search, and
  Helix-provided definition/reference context.
- Trusted-workbench initialization with repository-bound requests and no
  capability ceremony.
- Selection-only Pi RPC questions using Pi-owned OpenAI Codex authentication;
  Pi tools are not enabled yet.
- Streaming, cancellation, crash survival, explicit local diagnostics, and
  typed evidence provenance.
- Deterministic software tests and versioned DeepEval contracts.

## Current boundary

Protocol v5 and the terminal now open one trusted repository directly. The old
policy engine, capability fields, and `/trust` commands have been removed. Pi
still starts without tools, so agent questions can explain supplied evidence
but cannot yet inspect, edit, execute, or use Git on their own.

SQLite remains deferred by
[ADR 002](decisions/002-defer-sqlite-until-needed.md). There is no durable
session database or ad-hoc JSON replacement.

## Implement next

Build the smallest end-to-end full-access Pi harness:

1. Define the narrow typed lifecycle Lantern uses to present Pi's repository
   reads, edits, and development commands without exposing unbounded payloads.
2. Start Pi in the repository with its pinned `read`, `write`, `edit`, and
   `bash` tools.
3. Preserve cancellation, bounded protocol events, visible
   activity, and deterministic fake-harness tests.
4. Show edits through Helix and Git state through Lazygit; do not manipulate
   either surface by emitting arbitrary model keystrokes.
5. Prove one journey: ask for a small change, inspect the relevant code, edit
   it, run its focused test, and show the resulting diff.
6. Add DeepEval cases for natural explanation, correct tool choice, grounding,
   and interruption.

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
