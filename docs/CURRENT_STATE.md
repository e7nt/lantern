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
- Maintained Rust terminal, daemon, diagnostics, and Protocol v7 crates.
- Selection capture, exact navigation, bounded local literal search, and
  Helix-provided definition/reference context.
- Bounded two-hop outgoing-call context from Helix's active language server,
  with the deepest relevant call opened directly in Helix.
- Evidence-first symbol questions with bounded saved call-site and definition
  excerpts; the deepest resolved call or definition opens in Helix before the
  model answers.
- Trusted-workbench initialization with repository-bound requests and no
  capability ceremony.
- Selection- and symbol-grounded Pi RPC questions using Pi-owned OpenAI Codex
  authentication.
- Repository-grounded Pi questions from the empty prompt; editor context is an
  optional accelerator and never a prerequisite for talking to the agent.
- One lazily started Pi RPC process per workbench, with in-memory conversational
  continuity across sequential and multi-step turns and no Pi session file.
- Pi's pinned `read`, `grep`, `find`, `ls`, `edit`, `write`, and `bash` tools,
  launched inside the repository with typed activity in Lantern.
- Successful edit/write activity opens the changed file in Helix; `Space-g` or
  `/git` opens the focused Git review surface.
- Streaming, cancellation, crash survival, explicit local diagnostics, and
  typed evidence provenance.
- Deterministic software tests and versioned DeepEval contracts.

## Current boundary

Protocol v7 and the terminal open one trusted repository directly. The old
policy engine, capability fields, and `/trust` commands have been removed. Pi
runs its explicit built-in coding-tool allowlist in that repository. Raw tool
arguments, command output, and provider stderr are not copied into Lantern's
bounded UI protocol or diagnostics.

The persistent Pi driver remains workbench-local and sequential. Cancellation
uses RPC abort and preserves a healthy driver. A crashed or malformed driver is
stopped and reported; Lantern does not silently restart it. Closing the daemon
terminates and reaps Pi.

SQLite remains deferred by
[ADR 002](decisions/002-defer-sqlite-until-needed.md). There is no durable
session database or ad-hoc JSON replacement.

## Implement next

The external edit journey defined in
[EXTERNAL_EDIT_JOURNEY.md](EXTERNAL_EDIT_JOURNEY.md) now passes deterministically
and in a subscription-backed run. The sanitized result is recorded in
[the 2026-07-18 acceptance report](acceptance/2026-07-18-external-edit-journey.md).
Changed-file navigation derives the first bounded Git hunk instead of always
opening line 1.

Implement next:

1. Investigate remaining provider-latency outliers without weakening the strict
   three-second gate.
2. Spike semantic/vector retrieval only for a measured miss; do not add it to
   the current passing cases.

[The persistent Pi acceptance report](acceptance/2026-07-18-persistent-pi.md)
records a grounded warm follow-up beginning text in 1.52 seconds and settling
in 2.49 seconds with no tools. The initial repository-discovery turn remained
slower, so the under-three-second contract applies to warm context-grounded
follow-ups and first visible activity where tools are necessary.

The explicit live trace runner now exercises a natural repository explanation
and active interruption through the real Protocol v7 daemon. It records
grounding, bounded tool activity, time to first tool and response text, settling
time, cancellation latency, the dataset hash, and the Lantern revision in an
ignored local report. The initial repeated baseline is recorded in
[the 2026-07-18 live trace report](acceptance/2026-07-18-live-trace-baseline.md):
grounding and interruption passed, while one repetition correctly failed the
tool-efficiency ceiling instead of having its budget relaxed.

The exact-versus-LSP runner now evaluates pinned Helix and Lazygit revisions.
[The initial retrieval baseline](acceptance/2026-07-18-retrieval-baseline.md)
found that typed LSP context produced useful text 4.9 seconds sooner on Helix
and 16.2 seconds sooner on Lazygit while removing two and eight tool calls. All
answers were grounded; exact-only Lazygit failed the efficiency ceiling. This
does not justify a semantic index yet.

[The evidence-first LSP report](acceptance/2026-07-18-evidence-first-lsp.md)
records zero-tool grounded answers on both repositories. Helix began text in
2.37 seconds. Lazygit required no tools after call-site enrichment but began in
3.46 and 4.53 seconds at medium reasoning. Symbol-grounded turns now begin with
reasoning disabled and escalate to medium before the first requested tool;
repository and multi-step turns remain at medium. Three repeated Lazygit runs
began text in 3.32, 2.40, and 2.23 seconds (2.40-second median), remained
grounded, and used no tools. The strict per-run gate remains unchanged.
The version 2 retrieval dataset adds intentionally incomplete Helix evidence.
Across three live runs it correctly escalated, remained grounded and read-only,
and used three bounded tools, but first activity arrived in 3.59, 3.22, and
2.93 seconds. Its 3.22-second median is a recorded failure and the next measured
optimization target.
[The call-hierarchy spike](acceptance/2026-07-18-call-hierarchy-spike.md)
confirmed that rust-analyzer returns `goto_definition → goto_single_impl →
goto_impl`, directly locating the missing behavior. A contextual-grep prompt
experiment did not change the three-tool sequence and was reverted. The next
retrieval component is therefore bounded typed call structure, not semantic
vectors or more prompt text.
Protocol v7 now carries that bounded call structure. Three repeated runs of the
same Helix question used zero tools, began text in 2.34, 2.08, and 2.13 seconds
(2.13-second median), and remained grounded and read-only. The previous
incomplete-evidence baseline used three tools with a 3.22-second first-activity
median.
[The Go call-hierarchy validation](acceptance/2026-07-18-go-call-hierarchy.md)
probed the pinned Lazygit revision with `gopls v0.23.0`. The unchanged generic
Protocol v7 path answered a startup-flow question with zero tools and began
text in 2.34 seconds; exact discovery used three tools and began in 8.84
seconds. Dataset v4 retains both the Rust and Go regressions.
[The Python, JavaScript, and TypeScript validation](acceptance/2026-07-18-script-language-call-hierarchy.md)
uses real Pyright and TypeScript-language-server evidence. All three Protocol
v7 turns used zero tools and began text in 2.27–2.34 seconds. Dataset v5 pins
the external repositories and retains the language-specific call shapes.
The Protocol v7 external edit journey also passes: typed call evidence leads to
the implementation, Pi edits the implementation and focused test, Node
verification passes, two expected files remain unstaged for review, and active
inspection cancels and settles in 14 ms without changing repository state.

An incremental hybrid repository index remains conditional. Add
semantic/vector retrieval or commit-synchronized summaries only when a curated
question remains materially slow or incorrect after exact and typed LSP
evidence, and retain the component only if the same baseline improves.

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
