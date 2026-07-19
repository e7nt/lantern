# Lantern current state

This is the short implementation entry point for a new contributor. Read it
after `AGENTS.md` and before choosing work from the full roadmap.

## Product direction

Lantern is an open-source, lightweight coding workbench for developers who love
to understand and write code. The primary surface is Helix above a full-width
agent pane, with Lantern's focused Git review available as a narrow rail. Pi is the initial agent
harness. A launched workbench is trusted by default: the agent is intended to
search, edit, run development commands, and use Git while narrating meaningful
work and remaining immediately interruptible.

Answers should be natural and concise. Lantern should quietly ground them with
Helix/LSP evidence, exact search, syntax and Git structure, semantic/vector
retrieval, and commit-synchronized summaries. Internal protocol, provenance,
and tool vocabulary should not become user-facing ceremony.

## What works today

- Reproducible pinned Helix and maintained Lantern runtime preparation.
- The 80/20 Helix/agent composition and on-demand 10% focused Git rail.
- Mouse and keyboard interaction across the surfaces.
- A `Ctrl-a` contextual composer over Helix and reversible `F2` full-screen
  reading mode for the persistent agent pane.
- Bounded, typed composer submission over a private session-local Unix socket;
  tmux owns presentation and focus but never transports questions.
- Maintained Rust terminal, daemon, diagnostics, and Protocol v10 crates.
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

Protocol v10 and the terminal open one trusted repository directly. The old
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

1. Validate the read-only readiness brief on a real feature investigation
   before adding Markdown persistence or plan tasks. Do not add another
   retrieval or caching layer without a measured unmet question.

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
[The semantic retrieval spike](acceptance/2026-07-18-semantic-retrieval-spike.md)
establishes three vocabulary-mismatch cases. Exact discovery missed the
three-second activity gate twice and timed out once. Local embeddings ranked
the correct JavaScript and Python regions in 11–14 ms, but naïve fixed-window
indexing took 2.9–35.9 seconds and exceeded two minutes on Pi, so that
implementation was rejected rather than added to the runtime.
The Protocol v7 external edit journey also passes: typed call evidence leads to
the implementation, Pi edits the implementation and focused test, Node
verification passes, two expected files remain unstaged for review, and active
inspection cancels and settles in 14 ms without changing repository state.

[The retained incremental semantic index](acceptance/2026-07-18-incremental-semantic-index.md)
adds Protocol v8 `semantic` provenance. Ready-index vocabulary-mismatch turns
across Requests, p-limit, and Pi used zero tools and began text in 2.21–2.27
seconds. The daemon reopens every candidate against current source before use;
stale indexes are rejected and changed symbols reuse content-hashed vectors.

Ready semantic evidence now changes the compact activity line to `Found
relevant code · thinking…` without adding transcript noise or another protocol
event. Three repeated p-limit turns exposed verified code in 27–36 ms, used no
tools, and began model text in 2.27–2.41 seconds. The remaining 2.23–2.38 seconds
was provider wait, so another local cache was rejected. The measurement is
recorded in
[the grounded-wait report](acceptance/2026-07-18-grounded-wait-status.md).

Cold repository questions now expose `Preparing code understanding…` while the
local index builds, or `Searching the repository…` when semantic grounding is
unavailable. Protocol v9 introduced those two typed transient states and v10
retains them unchanged. A real
cold Requests clone exposed preparation in 1 ms without waiting for the index
or provider. The result is recorded in
[the cold-grounding report](acceptance/2026-07-18-cold-grounding-status.md).

Repeated semantic matches from the same file are now grouped per agent turn in
the transcript. The primary exact range remains directly navigable; `Space` or
mouse interaction expands and collapses every retained location. Definitions
and call paths remain individual because their relationship is meaningful.
The presentation change is recorded in
[the semantic evidence grouping report](acceptance/2026-07-18-semantic-evidence-grouping.md).

The incremental hybrid repository index is retained. Its model, virtual
environment, and revision-keyed artifacts are disposable local state. Initial
builds and changed-file refreshes run in the background; unchanged symbols
reuse vectors. A real uncommitted p-limit edit refreshed in 549 ms, embedded one
changed symbol, reused 16 vectors, and returned ready query results. Details are
recorded in
[the changed-file refresh report](acceptance/2026-07-18-semantic-refresh.md).
Commit-synchronized summaries remain conditional.

ADR 005 now proposes replacing the Pi CLI RPC adapter and broad Lazygit surface
only if bounded replacement spikes pass. The first real Pi SDK spike blocked an
edit before execution, preserved byte-identical source, then allowed exactly
one approved edit in the same subscription-backed session. Production promotion
still requires streaming, interruption, latency, and DeepEval parity. The Git
rail scope is deliberately limited to status, diffs, stage/unstage, commits,
local branches, fetch/fast-forward pull, recent history, conflicts, and opening
the selected range in Helix. Details are in
[ADR 005](decisions/005-lantern-owned-runtime-and-git-surface.md) and the
[Pi SDK spike report](acceptance/2026-07-18-pi-sdk-tool-control-spike.md).

The first focused Git spike also passes its command-boundary gate. Six
dependency-free Rust journeys prove exact categorized state, bounded diffs,
file and hunk stage/unstage, commits, branches, detached HEAD, conflicts,
history, and fetch plus fast-forward-only pull. Lazygit remains maintained until
the real rail, command deadlines, noninteractive authentication, privacy review,
and startup/memory comparison pass. See the
[focused Git command report](acceptance/2026-07-18-focused-git-command-spike.md).

The first removable focused-rail renderer now shows the branch plus a compact
conflict/staged/unstaged/untracked list at 10%-rail dimensions. Keyboard and
mouse selection, bounded staged/unstaged/untracked diff review, and file
stage/unstage pass ten deterministic tests. It is not wired to `/git`; pinned
Lazygit remains maintained until the remaining interaction, hardening,
accessibility, and performance gates pass. See the
[renderer report](acceptance/2026-07-19-focused-git-rail-renderer.md).

The renderer now reviews one typed hunk at a time, selectively stages or
unstages it, scrolls tall hunks, and opens the changed-line range through
Helix's existing typed navigation command. A live two-hunk journey proved the
index received only the selected change. See the
[hunk-review report](acceptance/2026-07-19-git-hunk-review-and-helix-navigation.md).

One temporary `a` overlay now completes the bounded functional scope: commit
the staged set, create or switch local branches, fetch, pull only when typed
upstream state permits a fast-forward, and inspect bounded recent history and
commit diffs. A live narrow-terminal journey proves commit, branch, history,
and commit-diff interaction. Functional expansion stops; Lazygit replacement
now has one bounded, noninteractive Git runner with typed private errors and
process-group deadlines. Fetch and fast-forward pull run without blocking the
rail and support `Esc` cancellation. Lazygit replacement still requires
state-preserving external refresh, accessibility checks, and measured
startup/RSS superiority. See the
[action-overlay report](acceptance/2026-07-19-focused-git-action-overlay.md).
The command and concurrency proof is recorded in the
[hardening report](acceptance/2026-07-19-git-command-hardening.md).

One coalesced background scan now detects edits and Git operations performed by
Helix, the agent, or another terminal without blocking the rail. Selection is
preserved by exact path and unified-hunk identity rather than row number; stale
results cannot overwrite newer local actions, and a cleaned file returns to the
list with an explanation. See the
[external-refresh report](acceptance/2026-07-19-git-external-refresh.md).

The focused rail now expresses Git state and keyboard focus in text, preserves
filenames in narrow paths, offers contextual `?` help without replacing review
state, and maps mouse review/stage/open interactions explicitly. Rendering is
event-driven rather than continuously repainting every 50 ms. The interaction
gate is recorded in the
[accessibility report](acceptance/2026-07-19-git-interaction-accessibility.md).

The reproducible 1,001-file performance gate passes. The focused rail measured
79.7 ms median startup and 2,960 KiB RSS versus pinned Lazygit's 95.6 ms and
24,888 KiB; visible external refresh measured 704.7 ms versus 8,364.7 ms. ADR
006 therefore promotes `apps/git-rail` as the only `/git` implementation.
Lazygit's build, configuration, launcher, environment, and product tests have
been removed together. See the
[performance report](acceptance/2026-07-19-git-surface-performance.md) and
[promotion decision](decisions/006-promote-focused-git-rail.md).

Git review now connects directly to the one conversational path. `Ctrl-a` on a
changed file or hunk opens the existing composer with typed, bounded path,
state, range, and diff evidence. The modal rail closes so the agent pane remains
interactive; reopening `/git` restores the exact or nearest surviving hunk
after agent edits. See the
[review-handoff report](acceptance/2026-07-19-git-agent-review-handoff.md).

The return path is connected as well. Successful edit/write events are retained
only for the active turn in a bounded path set. On settlement, Lantern offers
one compact review instruction; the next `Space-g` or `/git` consumes that
focus and selects the first agent-edited path that still has a live Git change.
The rail remains the authority for current Git state and continues to show all
repository changes. See the
[agent-change review report](acceptance/2026-07-19-agent-change-review-handoff.md).

`/investigate <feature objective>` runs one explicitly read-only Pi turn with
only `read`, `grep`, `find`, and `ls`. It streams a concise readiness brief
through the existing pane, separates observations from unknowns, requires an
explicit Ready or Blocked result, and exposes inspected files as navigable
investigation evidence. Its bounded brief is handed once to the warm coding
session when the developer follows up, so “proceed” retains context without
durable chat storage. It does not persist a plan or create another UI. See
the [investigation report](acceptance/2026-07-19-read-only-feature-investigation.md).

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
