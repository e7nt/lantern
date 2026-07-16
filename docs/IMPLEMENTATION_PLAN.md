# Lantern implementation plan

## Document status

- **Status:** Active; frontend decision accepted in ADR 001
- **Target:** Open-source-quality `v0.1`
- **Primary user:** One experienced developer onboarding into unfamiliar code
- **Initial frontend:** Pinned Helix, a narrow Lazygit rail, and a full-width
  terminal agent pane
- **Core runtime:** Local editor-independent daemon
- **Reference ecosystems:** TypeScript and Rust
- **Estimated solo effort:** 28–40 full-time engineer-weeks

This plan turns the product brief into implementable work. It is intentionally
bounded: the first release proves that understanding-first agent interaction is
useful before Lantern attempts to become a general-purpose editor platform.

All implementation and roadmap decisions must pass the product decision test in
[PRODUCT_CONSTITUTION.md](PRODUCT_CONSTITUTION.md).
Every deliverable must also pass the shared Definition of Done in
[ENGINEERING_STANDARD.md](ENGINEERING_STANDARD.md); phase exit criteria add to
that standard rather than replacing it.

Permanent protocol and interaction work must also follow the
[reference-project adoption discipline](REFERENCE_REPOSITORIES.md): inspect the
relevant upstream behavior, record what Lantern adopts and rejects, and attach
the behavior to a Lantern-owned test or evaluation.

## Release definition

`v0.1` is complete when a developer can open an unfamiliar reference
repository and perform this workflow:

1. Trust the workspace and inspect what Lantern is allowed to access.
2. Select code and receive a fast, evidence-linked answer.
3. Generate and follow one repository-specific learning mission.
4. Request a feature and receive an evidence-backed readiness report.
5. Collaborate on and approve a durable implementation plan.
6. Implement the approved plan through interruptible semantic chapters.
7. Hover over changed code to understand intent, behavior, and verification.
8. Review the result against acceptance criteria and tests.

The release must work without a hosted Lantern service. Model providers may be
remote, but repository state, plans, learning state, and audit records remain
local by default.

## Planning assumptions

- One experienced engineer is working full-time with AI-assisted development.
- Helix and Lazygit revisions and every Lantern patch are pinned and auditable.
- Linux is release-blocking for `v0.1`; macOS is validated before the public
  release gate. Windows is deferred while the tmux composition is primary.
- TypeScript and Rust fixtures receive full end-to-end coverage.
- Other languages may work through editor language features but are not claimed
  as supported in `v0.1`.
- A user explicitly authenticates a supported model driver. The Phase 0 Pi RPC
  experiment may use eligible ChatGPT subscription access; generic API billing
  and subscription access are never presented as interchangeable.
- No open-source license is selected until the owner makes that legal decision.
- The repository stays private until the security model and contributor setup
  pass the public-release gate.
- Lantern's core product remains open source and does not depend on a paid
  Lantern service.
- Implementations prefer a single explicit primary path and surface failures
  rather than accumulating silent fallback behavior.

## Architecture decisions

### Separate the frontend and runtime

The Lantern terminal client is the editor-facing presentation and integration layer.
It owns editor-native operations such as selections, navigation, decorations,
hovers, diffs, commands, and plan views. It does not own agent state or security
policy. Lantern-specific Helix changes remain narrow, pinned, and documented.

The daemon owns:

- Agent sessions and model interaction.
- Tool registration and policy enforcement.
- Repository and learner models.
- Plans, decisions, and approval state.
- Guided Build change sets and checkpoints.
- Change narratives and anchors.
- Durable storage and migrations.
- Audit events and redaction.

This boundary keeps the Helix client replaceable without moving security-critical
behavior into the editor integration.

### Use a small, typed protocol

The Lantern workbench starts the daemon and communicates over JSON-RPC 2.0 on
standard input/output for `v0.1`. This avoids ports, discovery, and persistent
background processes while preserving process isolation.

Protocol requirements:

- Versioned request, response, and event schemas.
- Cancellation for every model, index, and execution operation.
- Request correlation and structured errors.
- Capability negotiation during initialization.
- No credentials or source bodies in ordinary logs.
- Back-pressure for streamed model and tool events.
- Golden protocol fixtures shared by TypeScript and Rust tests.

A persistent local socket daemon may be evaluated after `v0.1` if startup cost
or cross-editor session sharing justifies it.

### Prefer deterministic code intelligence

Lantern uses deterministic evidence before semantic retrieval:

1. Current editor selection and open document.
2. Repository instructions and documentation.
3. Editor-provided symbols, definitions, references, and diagnostics.
4. Fast text and file search.
5. Tree-sitter structure and import relationships.
6. Git history and diff context.
7. Embedding-based retrieval only after measured need.

The Helix adapter normalizes editor language features into the editor-neutral
protocol. Quick Ask fails explicitly when required LSP evidence is unavailable;
literal search remains a separately invoked diagnostic operation, never a
substitute for symbol intelligence.

### Keep the agent harness replaceable

The runtime exposes an `AgentDriver` boundary around model turns and tool calls.
The first driver is a minimal single-agent loop with:

- Provider-neutral messages and streaming.
- Typed tools.
- Context compaction hooks.
- Cancellation and retry limits.
- Tool-result size limits.
- Deterministic mock models for tests.

Pi inspires the small harness, but Lantern core does not depend on Pi session
formats, extensions, tools, or prompts. Phase 0 evaluates a pinned Pi RPC
adapter for the selection-only `/agent` path before committing to a permanent
driver. Driver selection is explicit; failure never triggers an automatic
provider fallback.

### Store local state in SQLite

SQLite stores private operational state and supports transactional migrations.
Portable, reviewable plans remain Markdown with structured metadata rather than
being trapped in the database.

Core records include:

- Repositories and trust grants.
- Sessions, branches, and compacted context.
- Evidence references and freshness hashes.
- Learning missions, stops, questions, and checkpoints.
- Feature briefs, plans, decisions, tasks, and approvals.
- Change sets, chapters, operations, anchors, and verification.
- Tool calls, permission decisions, and audit metadata.

Raw model reasoning is not treated as a durable product artifact.

## Proposed repository structure

```text
lantern/
├── frontend/
│   ├── helix/                   # pinned patches, editor and Git config
│   └── terminal/                # developer-facing terminal surface
├── apps/
│   └── daemon/                  # Rust executable
├── crates/
│   ├── agent-runtime/           # agent loop and provider abstractions
│   ├── change-engine/           # change sets, chapters, replay, anchors
│   ├── code-intelligence/       # search, syntax, evidence, repository model
│   ├── diagnostics/             # metadata-only records and local export
│   ├── learning-engine/         # missions, guidance, learner state
│   ├── planning-engine/         # briefs, plans, decisions, approvals
│   ├── policy-engine/           # capabilities and permission enforcement
│   ├── protocol/                # Rust protocol types
│   └── storage/                 # SQLite schema and migrations
├── fixtures/
│   ├── rust-service/
│   └── typescript-service/
├── evaluations/
├── docs/
└── scripts/
```

The structure can be introduced incrementally; empty architectural directories
should not be created before their first real module exists.

## Work breakdown

| Phase | Scope | Effort | Depends on |
| --- | --- | ---: | --- |
| 0 | Product and architecture spikes | 2 weeks | — |
| 1 | Terminal client/daemon foundation | 3 weeks | Phase 0 |
| 2 | Quick Ask vertical slice | 3 weeks | Phase 1 |
| 3 | Repository understanding | 5–7 weeks | Phase 2 |
| 4 | Guided learning | 4–6 weeks | Phase 3 |
| 5 | Investigation and planning | 4–5 weeks | Phase 3 |
| 6 | Guided Build and change narratives | 5–7 weeks | Phases 4–5 |
| 7 | Review and verification | 3–4 weeks | Phase 6 |
| 8 | Open-source hardening | 5–7 weeks | All phases |

Some work overlaps, but the exit criteria are sequential. The implementation
should not begin a later user-facing phase while a foundational security or data
integrity criterion remains unmet.

## Phase 0: product and architecture spikes

### Objectives

Remove the highest-risk assumptions before building permanent infrastructure.

### Tasks

- `P0-01` Define five canonical user journeys and their expected interaction
  latency.
- `P0-02` Select two non-trivial public fixture repositories: one TypeScript and
  one Rust project.
- `P0-03` Prototype workbench-to-daemon JSON-RPC with cancellation and
  streamed events.
- `P0-04` Compare a minimal native agent loop with a Pi RPC adapter on one
  read-only repository question.
- `P0-05` Prototype editor hover, decoration, navigation, and selection capture.
- `P0-06` Prototype one Markdown-backed plan with structured task metadata.
- `P0-07` Write the initial threat model and identify all trust boundaries.
- `P0-08` Record architecture decisions as short ADRs.
- `P0-09` Prototype interruptible Live Collaboration using a realtime voice
  model, one read-only editor-context tool, visible transcript truncation, and
  the existing policy boundary.

### Exit criteria

- A selection can cross the process boundary and stream a mock response back.
- Cancelling the editor request terminates daemon work.
- A real Helix language-server session supplies bounded definition/reference
  evidence to a read-only answer with no evidence fallback.
- The plan format round-trips without losing hand edits.
- Security-sensitive operations have named enforcement points.
- The voice spike measures interruption latency, grounding, cost, privacy, and
  whether voice improves understanding over the text-only workflow.
- The chosen architecture is documented with rejected alternatives.

The frontend portion of this gate passed on 2026-07-15 and is accepted in
[ADR 001](decisions/001-helix-terminal-frontend.md). Plan, threat-model, and
voice artifacts remain separately gated; they do not reopen the frontend
decision unless ADR 001's revisit conditions occur.

## Phase 1: editor and daemon foundation

### Tasks

- `P1-01` Promote the pinned Helix/Lazygit preparation flow, terminal
  composition, patch inventory, clean-replay check, and editor integration
  tests from the spike into maintained infrastructure.
- `P1-02` Scaffold the Rust workspace with formatting, Clippy, denied unsafe code
  where practical, and unit tests.
- `P1-03` Implement daemon lifecycle management, health checks, graceful shutdown,
  crash reporting, and version negotiation.
- `P1-04` Define canonical protocol schemas and generate or validate Rust client
  and daemon types from the same source.
- `P1-05` Add structured errors, correlation IDs, cancellation, and event
  back-pressure.
- `P1-06` Create SQLite migrations and transactional repository initialization.
- `P1-07` Implement workspace trust with explicit read, write, execution, and
  network capabilities.
- `P1-08` Add redacted structured logging and an opt-in diagnostic bundle.
- `P1-09` Add provider credential resolution without copying secrets into the
  database.

Foundation progress on 2026-07-16: the first `P1-03`/`P1-04` lifecycle slice is
implemented in the maintained Rust workspace and
[Protocol v3](../protocol/v3/README.md).
It provides hard version negotiation, bounded recoverable JSONL framing,
explicit admission and settlement, duplicate-submit protection, idempotent
cancellation, and joined shutdown. This is not completion of daemon health,
crash supervision, structured schema generation, or the other Phase 1 tasks;
bounded back-pressure is addressed by the following slice.

The second `P1-03`/`P1-05` slice adds a two-second initialization deadline,
visible non-restarting crash state, continuously drained 8 KiB diagnostic
tails, a 256 KiB event limit, single-operation admission, and direct blocking
stdout back-pressure. Polling health and automatic restart are deliberately
excluded for the local stdio daemon: initialization is its ready boundary and
restarting could conceal lost operation state. Durable crash reports,
diagnostic redaction, and general supervision remain promotion work.

The `P1-07` slice adds Protocol v3 workspace configuration and a dedicated
policy crate. Every session starts locked, binds one canonical repository, and
requires all operation capabilities before admission. Local reads and model
transmission are separate, revocable session grants; repository write and
process execution are hard-denied in Quick Ask. There is no implicit trust,
parent-directory inheritance, saved wildcard, or approval fallback.

`P1-06` is deliberately deferred by
[ADR 002](decisions/002-defer-sqlite-until-needed.md). The current session has
no durable operational state that justifies a database, and adding SQLx plus an
async runtime before Quick Ask proves useful would violate the smallest-
coherent-product rule.

The `P1-08` slice emits bounded, versioned diagnostic JSONL containing only
typed event codes, timestamps, operation IDs, and runtime metadata. Arbitrary
messages, source, prompts, paths, environment values, and provider stderr are
excluded by schema. `/diagnostics` is an explicit local export available after
daemon failure; Lantern never creates or transmits a bundle automatically.

`P1-09` uses delegated authentication for the single explicit Pi driver. The
developer authenticates directly through Pi's `/login`; Lantern has no
credential input, protocol field, persistence, refresh path, or provider
fallback. Provider rejection detail is treated as sensitive and replaced by a
fixed error with an actionable Pi status and login recovery step. The boundary
is defined in [the provider credential contract](CREDENTIALS.md).

### Exit criteria

- Lantern starts the session-scoped daemon only with the agent pane.
- Daemon failure does not crash or block normal editing.
- Protocol compatibility failures produce actionable errors.
- A workspace begins locked and untrusted; read and model transmission require
  separate visible grants.
- CI tests protocol and patch replay on Linux, then adds macOS at the public
  release gate.

## Phase 2: Quick Ask vertical slice

### Tasks

- `P2-01` Capture the active document, selection, language, and repository root.
- `P2-02` Normalize document symbols, definitions, references, and diagnostics
  from Helix's active language-server sessions.
- `P2-03` Implement bounded file reading, file discovery, and text search tools.
- `P2-04` Build a context assembler that records why each context item was
  selected.
- `P2-05` Implement the read-only agent policy and reject edit or execution tool
  calls at runtime.
- `P2-06` Render concise answers as hovers and expanded answers in a side view.
- `P2-07` Link claims to files, symbols, and line ranges.
- `P2-08` Support cancellation, retry, provider errors, and usage visibility.
- `P2-09` Add a deterministic mock provider and golden end-to-end scenarios.

### Performance budgets

- Extension command dispatch: under 50 ms locally.
- Cached selection-context assembly: under 150 ms for reference fixtures.
- First streamed model content: measured and reported separately from local work.
- Cancelling a request: local tools stop within 500 ms.

### Exit criteria

- A user can ask what selected code does and inspect supporting evidence.
- The agent cannot modify files or execute commands in Quick Ask.
- Answers degrade clearly when symbols or provider access are unavailable.
- Repeated questions reuse fresh deterministic context without re-indexing the
  entire repository.

## Phase 3: repository understanding

### Tasks

- `P3-01` Discover repository instructions, READMEs, contribution guides, ADRs,
  manifests, lockfiles, and workspace boundaries.
- `P3-02` Detect languages, frameworks, build tools, entry points, tests, generated
  code, and deployment surfaces.
- `P3-03` Build an incremental file, package, import, and symbol inventory.
- `P3-04` Add an explicit syntax-only inspection mode for the two reference
  ecosystems, with its reduced evidence capabilities visible to the user.
- `P3-05` Model executable entry points and representative runtime handoffs.
- `P3-06` Add test-to-symbol and test-to-feature relationships.
- `P3-07` Classify architectural claims as observed, inferred, unknown, or
  contradictory.
- `P3-08` Attach evidence and freshness hashes to every durable claim.
- `P3-09` Invalidate only affected knowledge after file or branch changes.
- `P3-10` Add safe Git history and diff inspection.
- `P3-11` Build repository-understanding evaluations using questions with curated
  evidence sets.

### Exit criteria

- Lantern produces a useful map for both reference repositories without running
  untrusted setup scripts.
- Every durable claim exposes supporting evidence and confidence class.
- Incremental updates avoid full re-indexing for a one-file change.
- Known documentation/code contradictions appear explicitly.
- Evaluation results are reproducible with a mock model where interpretation is
  not required.

## Phase 4: guided learning

### Tasks

- `P4-01` Define learning mission, stop, subgoal, branch, checkpoint, and transfer
  task schemas.
- `P4-02` Generate a small orientation map from repository evidence.
- `P4-03` Generate a six-to-ten-stop vertical execution trace.
- `P4-04` Render the route in a native tree view and highlight the active code
  range without stealing focus.
- `P4-05` Support `focus`, `ignore`, `next handoff`, `deeper`, `simpler`, `skip`,
  and `resume` actions.
- `P4-06` Preserve nested prerequisite questions as branches from the main route.
- `P4-07` Add optional prediction and self-explanation prompts.
- `P4-08` Add micro-tasks and one transfer task per mission.
- `P4-09` Implement Tour, Navigator, and Challenge guidance levels.
- `P4-10` Persist learner notes, demonstrated knowledge, unresolved gaps, and
  mission progress.
- `P4-11` Add concise recall prompts when a relevant mission resumes later.

### Exit criteria

- A user can complete and resume a representative mission in each fixture.
- Branching questions return to the exact prior learning stop.
- Navigator mode avoids narrating obvious code.
- Learning remains structurally read-only.
- A transfer task checks a reusable system concept rather than filename recall.

## Phase 5: feature investigation and planning

### Tasks

- `P5-01` Implement the feature brief with objective, behavior, constraints,
  acceptance criteria, exclusions, and open questions.
- `P5-02` Investigate analogous features, affected flows, interfaces, data,
  tests, migrations, security, and operations.
- `P5-03` Produce a readiness report separating facts, inferences, and blocking
  unknowns.
- `P5-04` Define the portable plan schema and Markdown serialization.
- `P5-05` Support plan tasks, dependencies, decisions, alternatives, risks, and
  verification requirements.
- `P5-06` Add plan comments and agent suggestions without silently overwriting
  user text.
- `P5-07` Add granular approval for the brief, architecture, and implementation
  phases.
- `P5-08` Version plan revisions and preserve resolved decisions.
- `P5-09` Enforce that implementation tools remain unavailable until required
  approvals exist.

### Exit criteria

- A feature request becomes a human-editable plan grounded in repository
  evidence.
- Hand editing the Markdown remains safe and round-trippable.
- The daemon refuses implementation without required approval state.
- Material unknowns cannot be hidden by a high-confidence narrative.
- Plan revisions preserve authorship and decision history.

## Phase 6: Guided Build and semantic change narratives

The detailed interaction contract is in
[GUIDED_BUILD.md](GUIDED_BUILD.md).

### Tasks

- `P6-01` Define change set, chapter, operation, checkpoint, and narrative
  schemas.
- `P6-02` Generate the smallest coherent next chapter from an approved plan task.
- `P6-03` Stage edits against content hashes and reject stale inputs.
- `P6-04` Validate patch applicability and syntax before visible playback.
- `P6-05` Implement semantic, line, keystroke, and instant playback controllers.
- `P6-06` Support pause, resume, stop, step, skip, rewind, and speed controls.
- `P6-07` Group each chapter into one recoverable editor undo transaction.
- `P6-08` Detect user edits during playback and preserve them.
- `P6-09` Invalidate and replan future operations affected by user divergence.
- `P6-10` Run scoped diagnostics and approved verification after each chapter.
- `P6-11` Generate concise and expanded semantic change narratives.
- `P6-12` Anchor narratives using path, symbol, hunk fingerprint, content hashes,
  and Git base.
- `P6-13` Re-anchor narratives after edits or mark them stale when confidence is
  insufficient.
- `P6-14` Collapse generated, formatting, import, lockfile, and repetitive edits.
- `P6-15` Link plan tasks and acceptance criteria to changed files and tests.

### Safety invariants

- Raw model tokens never stream directly into repository files.
- A stale content hash prevents an operation from applying.
- User-authored edits are never overwritten to restore a playback timeline.
- Stop prevents subsequent operations and retains the last durable checkpoint.
- A material deviation creates a proposed plan amendment before continuing.

### Exit criteria

- A user can pause a chapter, ask a question, edit the code, and safely resume
  from a revised continuation.
- Every applied logical change has a narrative and verification state.
- Rewind restores the exact pre-chapter content.
- Mechanical edits do not overwhelm the playback timeline.
- A simulated crash during playback recovers to a known checkpoint.

## Phase 7: review and verification

### Tasks

- `P7-01` Add acceptance-criterion, plan-task, logical-change, file, and raw-diff
  review views.
- `P7-02` Add risk-focused filtering for public APIs, migrations, security, and
  concurrency.
- `P7-03` Link changed behavior to tests and command results.
- `P7-04` Record commands, exit status, duration, and redacted output summaries.
- `P7-05` Distinguish passed, failed, skipped, and unverified criteria.
- `P7-06` Generate a completion report from structured evidence rather than chat
  history.
- `P7-07` Export a pull-request-ready summary without publishing it automatically.
- `P7-08` Add review comments that can produce scoped revision tasks.

### Exit criteria

- Every acceptance criterion maps to implementation and verification or is
  explicitly unverified.
- A user can navigate from a hover to its plan decision and test evidence.
- Failed checks cannot be summarized as successful completion.
- Exported summaries contain no secrets or hidden raw model reasoning.

## Phase 8: open-source hardening

### Security

- `P8-01` Complete a threat model covering repository prompt injection, secret
  exposure, tool escalation, malicious paths, symlinks, and command execution.
- `P8-02` Add adversarial fixture repositories and policy regression tests.
- `P8-03` Protect credential files, VCS internals, environment files, and paths
  outside the trusted workspace.
- `P8-04` Make remote transmission visible and redact likely secrets.
- `P8-05` Add command classification, approvals, timeouts, and resource limits.
- `P8-06` Document what Lantern reads, stores, executes, and transmits.

### Reliability

- `P8-07` Test daemon crashes, interrupted streams, provider outages, malformed
  tools, cancelled operations, and storage failures.
- `P8-08` Add forward and backward migration tests for every persisted schema.
- `P8-09` Add large-repository performance and memory benchmarks.
- `P8-10` Ensure normal editing remains usable when Lantern is disabled or broken.
- `P8-11` Add backup, export, and reset procedures for local state.

### Contributor and release experience

- `P8-12` Add `CONTRIBUTING.md`, `ARCHITECTURE.md`, `SECURITY.md`, a code of
  conduct, issue templates, and pull request templates.
- `P8-13` Select an open-source license before changing repository visibility.
- `P8-14` Provide a one-command development bootstrap and fixture setup.
- `P8-15` Publish protocol and storage compatibility policies.
- `P8-16` Produce signed or checksummed daemon binaries for macOS and Linux.
- `P8-17` Package, checksum, and validate installable Lantern builds containing
  the supported Helix, Lazygit, pane, and daemon revisions.
- `P8-18` Add dependency, license, secret, and vulnerability checks to CI.
- `P8-19` Write user documentation for trust, model configuration, learning,
  planning, Guided Build, review, and recovery.
- `P8-20` Identify bounded good-first issues that do not require agent-runtime
  expertise.

### Public-release gate

The repository can become public only after:

- A license is explicitly selected.
- No credentials or private fixture data exist in history.
- The threat model and security-reporting process are published.
- Fresh-machine setup succeeds on supported macOS and Linux versions.
- CI passes without paid model credentials.
- Destructive tools are disabled by default and enforcement is covered by tests.
- Persisted schemas have migrations and a documented compatibility policy.
- Known limitations and unsupported languages are clearly stated.

## Test strategy

The behavioral evaluation contract is defined in
[EVALUATION_STRATEGY.md](EVALUATION_STRATEGY.md). DeepEval is the initial
open-source harness for model-mediated evaluations and remains isolated from
the production editor and daemon.

### Deterministic tests

- Unit tests for policy, planning, learning, anchoring, replay, and migrations.
- Protocol contract tests shared between Rust and TypeScript.
- State-machine tests for approvals and Guided Build checkpoints.
- Property tests for patch application and anchor re-identification.
- Golden tests for portable plan serialization.
- Extension-host tests for selections, hovers, decorations, and commands.
- Integration tests against compact TypeScript and Rust fixtures.
- End-to-end tests using a scripted mock model and tool responses.

### Model evaluations

DeepEval-based model evaluations record:

- Evidence precision and unsupported-claim rate.
- Repository question answer quality.
- Runtime-flow trace correctness.
- Learning-stop relevance and verbosity.
- Plan completeness and unknown disclosure.
- Change-narrative usefulness.
- Recovery from user divergence.

Evaluations store prompts, model identifiers, tool traces, costs, and judgments
without committing proprietary model output or secrets to the repository.

Small offline cases, hard invariants, dataset validation, and rubric validation
run in required CI. Networked model-judge runs are advisory at first and run
fully for scheduled and release-candidate evaluation. A semantic release gate
requires calibrated human rubrics, repeated samples, and regression review
rather than one stochastic score.

## Product measurements

The private alpha should measure:

- Time to first useful answer.
- Time to trace one representative flow.
- Percentage of claims with inspectable evidence.
- Number of irrelevant files opened during a learning mission.
- Questions answered without losing the main learning path.
- Plan amendments discovered during implementation.
- Guided Build pauses, skips, takeovers, and rewinds.
- Stale or incorrectly anchored change narratives.
- Verification failures found before completion.
- Local indexing time, steady-state memory, and model cost.

Metrics remain local unless the user explicitly exports them.

## Live Collaboration evaluation track

The detailed product and evaluation contract is in
[LIVE_COLLABORATION.md](LIVE_COLLABORATION.md).

This track evaluates an OpenAI Realtime voice collaborator that can discuss
visible code, narrate semantic implementation stages, call constrained Lantern
tools, and be interrupted naturally. It is not a v0.1 release commitment until
the evaluation gate passes.

The spike begins read-only. Voice must reuse the same session coordinator,
policy engine, evidence model, plan state, and Guided Build checkpoints as text.
It must not create a second agent state or allow spoken requests to bypass
approval.

If promoted, implementation should begin after Quick Ask and before Guided
Build playback is finalized, so interruption and narration become inputs to the
shared operation state machine rather than a late presentation layer.

## Milestone checkpoints

### Eight-week checkpoint

Expected scope: Phases 0–2 and the first portion of Phase 3.

Continue only if Quick Ask is already useful on real repositories and evidence
selection is measurably better than sending an entire file to a model.

### Private alpha checkpoint

Expected scope: Phases 0–5.

Continue only if a developer can learn a real vertical slice and produce a plan
whose affected areas and unknowns survive maintainer review.

### `v0.1` release candidate

Expected scope: all phases.

Release only if Guided Build preserves user edits, semantic narratives remain
anchored through ordinary revisions, and the product works without privileged
or destructive defaults.

## Principal risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Repository understanding sounds plausible but is wrong | Evidence classes, curated evaluations, explicit unknowns |
| Learning becomes a verbose tutorial | Navigator default, `ignore` guidance, strict stop budgets |
| Rich planning grows into a document platform | Markdown-backed bounded schema; defer general blocks |
| Playback is cosmetic theatre | Implement coherent chapters, verification, and real intervention |
| Partial edits corrupt the workspace | Content hashes, staged changes, checkpoints, undo transactions |
| Editor choice locks the architecture | Thin frontend and versioned editor-neutral protocol |
| Model/provider churn destabilizes core | Provider-neutral driver and deterministic mock tests |
| Indexing makes the editor feel heavy | Lazy activation, incremental invalidation, resource budgets |
| Repository instructions manipulate the agent | Trust separation and untrusted-content handling |
| Cross-platform scope delays usefulness | macOS/Linux first; defer Windows release guarantee |

## Explicitly deferred beyond `v0.1`

- Multi-agent orchestration.
- Cloud-hosted repository indexing.
- Multi-user plan collaboration.
- Visual webpage element-to-source selection.
- General-purpose autonomous computer control.
- A Helix fork or custom editor renderer.
- Broad extension marketplace.
- Generated course media.
- Production Live Collaboration unless its latency, grounding, privacy, cost,
  and user-value evaluation gate passes.
- Organization analytics and learner ranking.
- Claims of universal language support.

## First implementation slice

The authoritative scope, contracts, delivery sequence, exclusions, and
acceptance criteria are defined in
[FIRST_USEFUL_SLICE.md](FIRST_USEFUL_SLICE.md).

The first internally useful milestone is a Lantern editor connected to a
streaming mock daemon. The first externally meaningful slice adds enforced
read-only tools, one explicit provider, validated evidence, DeepEval behavioral
tests, and a usefulness gate. It does not include indexing, SQLite, planning,
Guided Build, file mutation, command execution, voice, or provider fallbacks.
