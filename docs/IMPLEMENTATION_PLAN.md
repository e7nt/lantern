# Lantern implementation plan

## Document status

- **Status:** Active; immediate sequence summarized in
  [CURRENT_STATE.md](CURRENT_STATE.md)
- **Target:** Open-source-quality `v0.1`
- **Primary user:** One experienced developer onboarding into unfamiliar code
- **Initial frontend:** Pinned Helix, a narrow focused Git rail, and a full-width
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

1. Ask a natural repository question and move directly to the relevant code.
2. Request a small change and watch the agent inspect, edit, test, and expose
   the Git diff while remaining interruptible.
3. Generate and follow one repository-specific learning mission.
4. Request a feature and receive an evidence-backed readiness report.
5. Collaborate on and refine a durable implementation plan.
6. Implement the plan through interruptible semantic chapters.
7. Hover over changed code to understand intent, behavior, and verification.
8. Review the result against acceptance criteria and tests.

The release must work without a hosted Lantern service. Model providers may be
remote, but repository state, plans, learning state, and audit records remain
local by default.

## Planning assumptions

- One experienced engineer is working full-time with AI-assisted development.
- Helix and every Lantern patch are pinned and auditable; the focused Git rail
  is built from this workspace.
- macOS is the supported `v0.1` release target. Linux must pass its own package
  and interaction acceptance before support is claimed. Windows is deferred
  while the tmux composition is primary.
- TypeScript and Rust fixtures receive full end-to-end coverage.
- Other languages may work through editor language features but are not claimed
  as supported in `v0.1`.
- A user explicitly authenticates a supported model driver. The Phase 0 Pi RPC
  experiment may use eligible ChatGPT subscription access; generic API billing
  and subscription access are never presented as interchangeable.
- Lantern is licensed under AGPL-3.0-only. Commercial use remains possible, but
  modified network services must preserve corresponding-source access.
- The public repository retains the security and contributor gates established
  before `v0.1.0`.
- Lantern's core product remains open source and does not depend on a paid
  Lantern service.
- Implementations prefer a single explicit primary path and surface failures
  rather than accumulating silent fallback behavior.

## Architecture decisions

### Separate the frontend and runtime

The Lantern terminal client is the editor-facing presentation and integration layer.
It owns editor-native operations such as selections, navigation, decorations,
hovers, diffs, commands, and plan views. It does not own agent state or model
execution. Lantern-specific Helix changes remain narrow, pinned, and documented.

The daemon owns:

- Agent sessions and model interaction.
- Typed tool registration and execution coordination.
- Repository and learner models.
- Plans and decisions.
- Guided Build change sets and checkpoints.
- Change narratives and anchors.
- Durable storage and migrations.
- Audit events and redaction.

This boundary keeps the Helix client replaceable without moving agent behavior
into the editor integration.

### Use a small, typed protocol

The Lantern workbench starts the daemon and communicates through strict,
versioned LF-delimited JSON on standard input/output for `v0.1`. This avoids
ports, discovery, and persistent background processes while preserving process
isolation.

Protocol requirements:

- Versioned request, response, and event schemas.
- Cancellation for every model, index, and execution operation.
- Request correlation and structured errors.
- Explicit trusted-workbench initialization.
- No credentials or source bodies in ordinary logs.
- Back-pressure for streamed model and tool events.
- Golden protocol fixtures shared by TypeScript and Rust tests.

A persistent local socket daemon may be evaluated after `v0.1` if startup cost
or cross-editor session sharing justifies it.

### Use measured hybrid code intelligence

Lantern combines complementary repository signals:

1. Current editor selection and open document.
2. Repository instructions and documentation.
3. Editor-provided symbols, definitions, references, and diagnostics.
4. Fast text and file search.
5. Tree-sitter structure and import relationships.
6. Git history and diff context.
7. Incremental semantic/vector retrieval.
8. Commit-synchronized summaries verified against current source.

The Helix adapter normalizes editor language features into the editor-neutral
protocol. Retrieval ranks these sources into one bounded context package and
records provenance internally. Generated summaries and embeddings remain
disposable indexes rather than source of truth. Each component must earn its
latency and relevance cost in repository-question evaluations.

### Keep the agent harness replaceable

The runtime exposes an `AgentDriver` boundary around model turns and tool calls.
The first driver is the pinned Pi harness with:

- Provider-neutral messages and streaming.
- Typed tools.
- Context compaction hooks.
- Cancellation and retry limits.
- Tool-result size limits.
- Deterministic fake-harness behavior for tests.

Lantern owns typed workbench tools, context assembly, visibility, and
evaluation; Pi owns the initial agent loop behind a replaceable adapter. Do not
build a parallel native loop unless ADR 004's revisit condition is met. Driver
failure never triggers an automatic provider fallback.

### Defer durable operational storage

ADR 002 defers SQLite until a proven journey requires state across process
restarts. Session-only state remains in memory and portable plans remain
Markdown. Do not introduce ad-hoc JSON persistence as a substitute.

If the revisit condition is met, evaluate durable records such as:

- Repositories and workbenches.
- Sessions, branches, and compacted context.
- Evidence references and freshness hashes.
- Learning missions, stops, questions, and checkpoints.
- Feature briefs, plans, decisions, and tasks.
- Change sets, chapters, operations, anchors, and verification.
- Tool calls and audit metadata.

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
│   ├── agent-driver/            # Pi adapter and deterministic fake harness
│   ├── change-engine/           # change sets, chapters, replay, anchors
│   ├── code-intelligence/       # hybrid repository retrieval
│   ├── diagnostics/             # metadata-only records and local export
│   ├── learning-engine/         # missions, guidance, learner state
│   ├── planning-engine/         # briefs, plans, tasks, and decisions
│   ├── workbench-tools/         # typed file, command, Git, and editor actions
│   └── protocol/                # Rust protocol types
├── fixtures/
│   ├── rust-service/
│   └── typescript-service/
├── evaluations/
├── docs/
└── scripts/
```

The structure can be introduced incrementally; empty architectural directories
should not be created before their first real module exists. A storage crate is
not part of this structure unless ADR 002's revisit condition is met.

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

Phases 0 and 1 include historical tasks that explain the current Protocol v4
checkpoint. They are not the next-work queue. `CURRENT_STATE.md` and accepted
ADRs take precedence when those records describe superseded architecture.

## Phase 0: product and architecture spikes

### Objectives

Remove the highest-risk assumptions before building permanent infrastructure.

### Tasks

- `P0-01` Define five canonical user journeys and their expected interaction
  latency.
- `P0-02` Select two non-trivial public fixture repositories: one TypeScript and
  one Rust project.
- `P0-03` Prototype workbench-to-daemon typed JSONL with cancellation and
  streamed events.
- `P0-04` Validate the Pi RPC adapter on one repository question.
- `P0-05` Prototype editor hover, decoration, navigation, and selection capture.
- `P0-06` Prototype one Markdown-backed plan with structured task metadata.
- `P0-07` Write the initial threat model and identify all trust boundaries.
- `P0-08` Record architecture decisions as short ADRs.
- `P0-09` Prototype interruptible Live Collaboration using a realtime voice
  model, one editor-context tool, and visible transcript truncation.

### Exit criteria

- A selection can cross the process boundary and stream a mock response back.
- Cancelling the editor request terminates daemon work.
- A real Helix language-server session supplies bounded definition/reference
  evidence to a grounded answer.
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
- `P1-06` **Deferred by ADR 002:** create SQLite migrations only after a proven
  durable-state requirement.
- `P1-07` **Historical, superseded by ADR 003:** the Protocol v4 workspace trust
  implementation.
- `P1-08` Add redacted structured logging and an opt-in diagnostic bundle.
- `P1-09` Add provider credential resolution without copying secrets into the
  database.
- `P1-10` Replace the startup file-list picker with a persistent, bounded
  folder tree that opens files through Helix's typed navigation seam and shows
  existing Git/review state without owning edits or Git mutation. Implemented
  as `apps/explorer`; ignored files, file operations, and icon systems remain
  outside the first slice.

Foundation progress on 2026-07-16: the first `P1-03`/`P1-04` lifecycle slice is
implemented in the maintained Rust workspace and
[Protocol v4](../protocol/v4/README.md).
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

The historical `P1-07` slice added locked workspace configuration and a
dedicated policy crate. ADR 003 superseded that direction. Protocol v5 removed
the policy crate, capability negotiation, and `/trust`; Protocol v17 retains the
single trusted-workbench path, adds repository questions without requiring
editor context, and carries verified local-semantic evidence. Protocol v4 and
v5 fixtures remain historical evidence only.

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

The first `P2-04` slice added typed evidence provenance in Protocol v4. The
terminal derives compact reasons locally and reuses the existing streamed
evidence and exact-range navigation path; it performs no additional scan,
index, or model request.

The maintained Protocol v17 product now exposes Pi's pinned coding-tool set in
the trusted repository and submits `Ctrl-a` composer questions through a
bounded private Unix socket. Plain questions are the only conversational path;
selection and LSP context enrich it without creating a fallback agent mode.

A 2026-07-17 live probe found and removed two avoidable local-search costs:
Python environment and tool-cache directories now share the existing explicit
dependency exclusion, and deterministic answers no longer simulate model
typing with a 35 ms delay per word. On the Lantern repository, the same exact
search inspected 50 relevant files and reached `completed` in 75 ms, down from
1.13 seconds and a 2,000-file ceiling before the correction. Real provider
responses continue to stream as they arrive.

### Exit criteria

- Lantern starts the session-scoped daemon only with the agent pane.
- Daemon failure does not crash or block normal editing.
- Protocol compatibility failures produce actionable errors.
- A workbench initializes through one explicit trusted-session boundary.
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
- `P2-05` Expose typed repository read, edit, command, Git, and navigation tools
  to the Pi harness.
- `P2-06` Render natural concise answers in the full-width bottom pane and open
  supporting code directly in Helix.
- `P2-07` Link claims to files, symbols, and line ranges.
- `P2-08` Support cancellation, retry, provider errors, and usage visibility.
- `P2-09` Add a deterministic fake Pi harness and golden end-to-end scenarios.

### Performance budgets

- Extension command dispatch: under 50 ms locally.
- Cached selection-context assembly: under 150 ms for reference fixtures.
- First streamed model content: measured and reported separately from local work.
- Cancelling a request: local tools stop within 500 ms.

### Exit criteria

- A user can ask what code does and move directly to supporting evidence.
- A user can request a small change and watch Pi inspect, edit, run the focused
  test, and expose the resulting diff.
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
- `P3-03a` Add incremental semantic/vector indexing and commit-synchronized
  summaries, measured against the LSP/exact baseline.
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
- `P5-06` Add bounded multi-place plan comments and one coherent, previewed
  agent revision without silently overwriting user text. Implemented in
  Protocol v13; real-journey validation remains.
- `P5-07` Add visible checkpoints for the brief, architecture, and
  implementation phases.
- `P5-08` Version plan revisions and preserve resolved decisions.
- `P5-09` Keep plan changes and implementation divergence visible without
  turning the plan into a tool permission gate. Protocol v14 stages one
  evidence-bounded plan checkpoint after a reviewable implementation turn;
  real-journey validation remains.

### Exit criteria

- A feature request becomes a human-editable plan grounded in repository
  evidence.
- Hand editing the Markdown remains safe and round-trippable.
- Implementation can proceed from natural developer intent while material plan
  divergence remains visible.
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
- `P7-03` Add exact-line, batched review comments over modified and staged Git
  hunks. Protocol v17 implements an editable draft, one explicit submission,
  and one coherent correction turn; real-journey validation remains.
- `P7-03a` Expand the compact Git rail into an editor-sized review canvas when a
  diff opens, then collapse without losing the file, hunk, line, scroll
  position, or pending review count. Implemented in the focused Git surface;
  live visual validation remains.
- `P7-03b` Return submitted comments as a passive `Your review` section beside
  the agent's new diff. Implemented without resolution state, approval gates,
  or mandatory interaction; live visual validation remains.
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
- `P8-02` Add adversarial fixture repositories and tool-boundary regression
  tests.
- `P8-03` Protect credential files, VCS internals, environment files, and paths
  outside the trusted workspace.
- `P8-04` Make remote transmission visible and redact likely secrets.
- `P8-05` Add visible command classification, timeouts, cancellation, and
  resource limits without routine approval prompts.
- `P8-06` Document what Lantern reads, stores, executes, and transmits.

### Reliability

- `P8-07` Test daemon crashes, interrupted streams, provider outages, malformed
  tools, cancelled operations, and storage failures.
- `P8-08` Add forward and backward migration tests for every persisted schema.
- `P8-09` Add large-repository performance and memory benchmarks.
- `P8-10` Ensure normal editing remains usable when Lantern is disabled or broken.
- `P8-11` Add backup, export, and reset procedures for local state.

### Contributor and release experience

- `P8-12` Add contributor stewardship documents. `CONTRIBUTING.md` and
  `SECURITY.md` are implemented; a concise architecture entry point, code of
  conduct, and issue/pull-request templates remain.
- `P8-13` Select an open-source license before changing repository visibility.
  AGPL-3.0-only is selected and recorded in package and contributor metadata.
- `P8-14` Provide a one-command development bootstrap and fixture setup. The
  pinned Helix preparation and canonical `scripts/check.sh` gate work from a
  clean checkout and have passed on both supported macOS architectures.
- `P8-15` Publish protocol and storage compatibility policies.
- `P8-16` Produce signed or checksummed daemon binaries for macOS and Linux.
  The tag workflow now produces checksummed, attested macOS workbench archives;
  Linux release artifacts remain.
- `P8-17` Package, checksum, and validate installable Lantern builds containing
  the supported Helix, focused Git rail, pane, daemon, semantic worker, and Pi
  revisions. Architecture-specific Homebrew packages, tap updates, and fresh
  Apple Silicon and Intel installs are implemented and validated. A real
  cross-version `brew upgrade` journey remains to be recorded.
- `P8-18` Add dependency, license, secret, and vulnerability checks to CI. The
  least-privilege deterministic Rust, terminal, evaluation, and semantic jobs
  are implemented; supply-chain audit jobs remain.
- `P8-19` Write user documentation for workbenches, model configuration, learning,
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
- Destructive Git history operations require an explicit request and are
  covered by tests.
- Persisted schemas have migrations and a documented compatibility policy.
- Known limitations and unsupported languages are clearly stated.

## Test strategy

The behavioral evaluation contract is defined in
[EVALUATION_STRATEGY.md](EVALUATION_STRATEGY.md). DeepEval is the initial
open-source harness for model-mediated evaluations and remains isolated from
the production editor and daemon.

### Deterministic tests

- Unit tests for tools, planning, learning, anchoring, replay, and migrations.
- Protocol contract tests shared between Rust and TypeScript.
- State-machine tests for Guided Build checkpoints and interruption.
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

## Multi-folder workbench evaluation track

Lantern should eventually allow a developer to attach multiple explicit local
folders from unrelated filesystem locations to one workbench. This supports
cross-repository questions and changes such as tracing a client into a service,
updating a shared library and its consumer together, or comparing coordinated
Git history without copying repositories into one parent directory.

The workbench—not directory ancestry—defines the folder set. Each folder keeps
its own repository identity, branch, working tree, instructions, index, and Git
operations. Retrieval may search across the attached set and must identify the
source repository of every result. Cross-repository plans and changes must show
which repository each step affects, while Helix navigation opens the matching
attached folder and Lazygit remains scoped to one repository at a time.

Each multi-folder workbench has a human-readable `WORKBENCH.md`. It explains:

- the attached repositories and their stable workbench aliases;
- what each repository owns and why it belongs in the workbench;
- important relationships such as client/service, library/consumer, or shared
  protocol boundaries;
- common development, test, and integration commands; and
- cross-repository conventions or coordination notes the agent must understand.

`WORKBENCH.md` is the reviewable source of workbench intent, not an embedding
dump or generated inventory. Local attachment resolution may map aliases to
machine-specific absolute paths without writing those paths into portable
documentation. Derived symbol indexes, embeddings, and commit snapshots remain
rebuildable caches. Lantern may propose an update when repository relationships
change, but it must show that Markdown change like any other developer artifact.

This is not part of the first single-repository slice. Evaluate it after the
repository-understanding path is fast and reliable in one repository. A spike
must measure incremental indexing cost, cross-repository retrieval quality,
ambiguous symbol handling, coordinated change review, and whether explicit
folder attachment remains understandable without introducing a heavyweight
workspace manager.

## Live Collaboration evaluation track

The detailed product and evaluation contract is in
[LIVE_COLLABORATION.md](LIVE_COLLABORATION.md).

This track evaluates an OpenAI Realtime voice collaborator that can discuss
visible code, narrate semantic implementation stages, call constrained Lantern
tools, and be interrupted naturally. It is not a v0.1 release commitment until
the evaluation gate passes.

Voice must reuse the same session coordinator, evidence model, plan state,
visible tool activity, and Guided Build checkpoints as text. It must not create
a second agent state or hidden action path.

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
| Repository instructions manipulate the agent | Treat repository content as data and validate typed tool boundaries |
| Cross-platform scope delays usefulness | macOS/Linux first; defer Windows release guarantee |

## Explicitly deferred beyond `v0.1`

- Multi-agent orchestration.
- Multi-folder, cross-repository workbenches; retain the evaluation track above
  so the single-repository architecture does not accidentally make them
  impossible.
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

## Immediate implementation slice

The authoritative current sequence is in
[CURRENT_STATE.md](CURRENT_STATE.md). The completed selection-only Quick Ask
scope remains documented in [FIRST_USEFUL_SLICE.md](FIRST_USEFUL_SLICE.md) as
historical implementation evidence. The next slice promotes Pi into a trusted
coding harness with typed search, edit, command, Git, and Helix-navigation
tools, followed by measured hybrid indexing. It does not include SQLite, voice,
multiple agents, or provider fallbacks.
