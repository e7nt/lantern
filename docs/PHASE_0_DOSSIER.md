# Phase 0 implementation dossier

## Status and purpose

- **Status:** Historical proposal; frontend sections revised by ADR 001
- **Scope:** Decisions and spikes required before permanent v0.1 foundations
- **Primary deliverable:** A tested Quick Ask process boundary
- **Non-goal:** Implementing indexing, planning, or repository mutation

This dossier fills the gap between the product brief and task-level
implementation. Statements are either decisions for the first spike,
hypotheses that the spike must test, or open decisions with an explicit owner
and deadline.

ADR 001 accepts the Helix-centered terminal frontend after the recorded spike.
Code OSS/VSCodium workbench tasks below remain as rejected-alternative history
and are not part of the active implementation sequence.

## Canonical user journeys

### J1: Ask about a selection

1. The user selects code and invokes `Lantern: Ask About Selection`.
2. The Lantern workbench captures a bounded snapshot of editor and repository
   context.
3. The daemon validates trust and read capability.
4. The context assembler requests related symbols and bounded file evidence.
5. The agent streams an answer containing claim-to-evidence links.
6. The user opens an evidence target without losing the answer.
7. Cancelling from the editor stops model and tool work.

Budgets:

- Command acknowledgement: 50 ms p95 on a warm workbench.
- Local context snapshot: 150 ms p95 on a fixture repository.
- First local progress event: 100 ms p95.
- Local cancellation propagation: 100 ms p95.
- Bounded read/search termination after cancellation: 500 ms p95.
- Provider latency is measured separately and never hidden in local metrics.

### J2: Learn a vertical slice

1. The user chooses a repository entry point or asks Lantern to suggest one.
2. Lantern creates a six-to-ten-stop route grounded in repository evidence.
3. Each stop states the subgoal, important idea, safe-to-ignore detail, and next
   handoff.
4. A prerequisite branch preserves the exact return point.
5. The user completes a prediction, explanation, or small transfer task.
6. Mission state survives editor and daemon restarts.

The Phase 0 spike validates only the route and navigation representation. It
does not implement learner modeling.

### J3: Investigate and plan a feature

1. The user creates an editable feature brief.
2. Lantern investigates current behavior, analogues, interfaces, risks, tests,
   and unknowns without enabling write tools.
3. A readiness report separates observations, inferences, contradictions, and
   blocking unknowns.
4. The user passes the understanding gate.
5. Lantern creates a Markdown-backed plan and preserves direct user edits.
6. Architecture and implementation approvals are recorded separately.

The Phase 0 spike proves lossless Markdown round-tripping for one small plan.

### J4: Guided Build with intervention

1. The user selects an approved plan task.
2. Lantern proposes the next semantic chapter and expected affected symbols.
3. The daemon stages edits against content hashes.
4. The workbench previews and applies the validated operations as one undoable
   unit.
5. The user pauses and edits the affected code.
6. Lantern detects divergence, invalidates unsafe future operations, and
   proposes a revised continuation.

The Phase 0 output is a state-machine design and patch-application spike, not a
production playback controller.

### J5: Review against intent

1. The user opens an acceptance criterion.
2. Lantern shows associated decisions, logical changes, files, tests, and
   verification status.
3. The user navigates from a changed range to its narrative and evidence.
4. Failed, skipped, and unverified checks remain visibly distinct.
5. Lantern exports a local pull-request summary without publishing it.

### J6: Live Collaboration

1. The user starts an explicitly permissioned voice session.
2. Lantern discusses the visible selection, evidence, plan, or build chapter.
3. Spoken narration tracks the same durable operation state as the editor.
4. The user interrupts naturally or with push-to-talk.
5. Audio stops immediately, unplayed output is removed from conversation state,
   and no unstarted action proceeds.
6. Important approvals remain visible editor artifacts.

Phase 0 evaluates this journey read-only before it becomes a release
commitment. See [LIVE_COLLABORATION.md](LIVE_COLLABORATION.md).

## System boundary

```text
Lantern editor workbench
  presentation
  editor context adapter
  language feature adapter
  daemon supervisor
          |
          | versioned JSON-RPC 2.0 over stdio
          v
Lantern daemon
  request router and cancellation registry
  session coordinator
  capability/policy engine
  agent driver
  evidence and repository services
  planning/learning/change services
  storage and audit
          |
          +-- model provider APIs (explicit network capability)
          +-- workspace filesystem (bounded capabilities)
          +-- approved child processes (explicit execution capability)
```

The workbench is not a security boundary. All sensitive authorization is
rechecked in the daemon at the point of use.

## Component responsibilities

### Lantern workbench

Owns:

- Commands, keybindings, selection and active-editor snapshots.
- Stable editor language features: symbols, definitions, references, hover
  source data, and diagnostics.
- Native presentation: hover, decoration, tree, webview where unavoidable,
  virtual documents, diffs, notifications, and progress.
- Daemon discovery, spawn, health monitoring, restart, and shutdown.
- Mapping editor cancellation to protocol cancellation.
- Applying already validated edits through editor APIs.

Does not own:

- Model credentials, prompts, agent sessions, policy decisions, or audit state.
- Direct model-provider calls.
- Whether a requested tool invocation is authorized.
- Durable plan or learning state.

### Daemon request router

- Performs initialization and capability negotiation.
- Validates message shape and size before dispatch.
- Assigns correlation and operation identifiers.
- Owns the cancellation registry.
- Applies concurrency and queue limits.
- Emits structured errors without leaking source or credentials.

### Session coordinator

- Tracks repository identity, Git base, mode, user intent, and active operation.
- Serializes mutations within a session.
- Allows safe read-only operations to run concurrently under bounded limits.
- Creates branches for investigation and learning without copying raw history.
- Invokes compaction through an explicit artifact-producing boundary.

### Policy engine

- Resolves effective capabilities from workspace trust, mode, approvals, user
  grants, path scope, and requested operation.
- Returns allow, deny, or require-approval with a stable reason code.
- Is invoked during tool exposure, before execution, and inside privileged tool
  implementations.
- Records decisions without recording secrets or unnecessary source.

Initial capabilities:

| Capability | Examples |
| --- | --- |
| `workspace.metadata.read` | names, sizes, language IDs, Git state |
| `workspace.content.read` | bounded source and documentation reads |
| `workspace.content.write` | approved file edits inside canonical root |
| `process.execute` | allowlisted command with cwd, timeout, and limits |
| `network.model` | transmit approved context to selected provider |
| `network.other` | non-model remote access; denied by default |
| `plan.write` | write only Lantern plan artifacts |
| `state.export` | export redacted local artifacts |

### Agent driver

The driver isolates Lantern from a specific harness or provider:

```text
start_turn(input, tools, context, signal) -> stream<AgentEvent>
continue_turn(session, context, signal) -> stream<AgentEvent>
compact(session, policy, signal) -> CompactionArtifact
estimate_usage(input) -> UsageEstimate
list_models() -> ModelDescriptor[]
```

Required event categories:

- Turn and message start/update/end.
- Validated tool request, tool progress, and tool result.
- Usage update.
- Retry/backoff state.
- Compaction requested/completed.
- Warning and terminal error.

The contract does not expose provider-specific message objects, Pi session
entries, raw chain-of-thought, or frontend presentation types.

### Repository and evidence services

- Canonicalize paths without following unauthorized symlink escapes.
- Discover instructions and package boundaries read-only.
- Normalize editor-provided language intelligence.
- Provide bounded file discovery, search, reading, Git inspection, and syntax
  structure.
- Attach repository revision, content hash, path, range, and derivation to
  evidence.
- Mark durable claims observed, inferred, unknown, or contradictory.

## Protocol draft

### Transport

- JSON-RPC 2.0 over daemon stdin/stdout.
- One UTF-8 JSON object per LF-delimited record.
- Stderr is diagnostic output only and never protocol data.
- Maximum inbound record size and maximum queued outbound bytes are enforced.
- Source bodies are request payloads only when required; they do not appear in
  ordinary logs.

Pi's strict JSONL behavior is a useful transport reference, but Lantern uses
JSON-RPC semantics for standardized request correlation and cancellation.

### Initialization

`initialize` request fields:

- Protocol version range.
- Extension version and Code OSS API version.
- Client capabilities.
- Workspace folders with opaque client identifiers.
- Workspace trust signal.
- Locale and platform.

`initialize` response fields:

- Selected protocol version.
- Daemon version and build identifier.
- Server capabilities.
- Required migrations or incompatibility error.
- Session recovery availability.
- Limits: message bytes, concurrent operations, and supported event window.

No operational request is accepted before successful initialization.

### First-slice methods

| Method | Direction | Purpose |
| --- | --- | --- |
| `initialize` | client to daemon | Negotiate protocol and capabilities |
| `session/open` | client to daemon | Establish repository/session identity |
| `selection/ask` | client to daemon | Start a read-only streamed question |
| `operation/cancel` | client to daemon | Idempotently cancel an operation |
| `daemon/health` | client to daemon | Liveness and readiness |
| `daemon/shutdown` | client to daemon | Graceful bounded shutdown |
| `operation/event` | daemon to client | Stream progress, content, evidence, usage |
| `editor/contextRequest` | daemon to client | Request normalized editor intelligence |

### Operation lifecycle

```text
request accepted
  -> operation.created
  -> operation.progress*
  -> answer.delta*
  -> evidence.upsert*
  -> usage.update*
  -> operation.completed | operation.cancelled | operation.failed
```

Every event contains protocol version, operation ID, monotonically increasing
sequence number, event type, timestamp, and typed payload. The client detects a
sequence gap and requests a bounded replay or marks the stream incomplete.

### Cancellation semantics

- Cancellation is idempotent.
- Cancelling an unknown or terminal operation succeeds with terminal state.
- The daemon aborts provider streaming and propagates the signal to active tools.
- No new tool begins after cancellation is observed.
- Results arriving after terminal cancellation are discarded from user-visible
  state but may produce redacted diagnostic metadata.
- Shutdown first cancels operations, then waits a bounded grace period, then
  terminates.

### Back-pressure

- Each operation has an outbound byte budget.
- Text deltas may be coalesced; lifecycle, tool, evidence, and terminal events
  may not be dropped.
- The daemon pauses provider consumption where supported.
- A client that cannot drain within the configured deadline receives an
  `client_too_slow` terminal error.

### Error model

Errors contain:

- Stable machine code.
- Safe user message.
- Retryability and suggested action.
- Operation and correlation identifiers.
- Optional redacted diagnostic reference.

Initial codes include `not_initialized`, `version_incompatible`,
`workspace_untrusted`, `capability_denied`, `approval_required`,
`invalid_path`, `stale_content`, `provider_unavailable`,
`provider_rate_limited`, `cancelled`, `timeout`, `client_too_slow`,
`storage_unavailable`, and `internal`.

## Quick Ask context contract

The workbench sends:

- Workspace identifier, document URI, relative path, language ID, document
  version, selection range, selection text, and dirty-state flag.
- A content hash for the complete document snapshot when available.
- No hidden editor buffers or unrelated open tabs by default.

The daemon may request:

- Document symbols.
- Definition targets.
- Reference targets with a hard result cap.
- Diagnostics intersecting or adjacent to the selection.
- Small surrounding ranges.

Every assembled context item records source, reason selected, freshness, token
estimate, sensitivity classification, and truncation state. The context budget
prioritizes selection, containing symbol, definitions, repository instructions,
nearby tests, then bounded references.

## Persistence model

### SQLite owns

- Repository identities and canonical roots.
- Trust and capability grants.
- Sessions and compacted context artifacts.
- Evidence metadata and freshness hashes.
- Learning progress.
- Plan indexes, decisions, approvals, and revision metadata.
- Change sets, chapters, checkpoints, anchors, and verification records.
- Redacted audit and diagnostic references.

### Files own

- Human-editable Markdown plans.
- Optional exported learning notes and completion reports.
- Repository source code.
- Diagnostic bundles explicitly exported by the user.

### Identity and freshness

- Repository identity is a generated UUID associated with canonical root and
  optional Git identity; paths alone are not durable identity.
- File evidence uses repository ID, normalized relative path, content hash, and
  optional Git blob/base.
- Symbol evidence adds language, stable symbol key where available, range, and
  syntax fingerprint.
- Line numbers are display hints, never the sole anchor.

### Migration rules

- The daemon is the sole database writer.
- Migrations are transactional, numbered, and forward-only in normal use.
- Each release tests upgrade from the oldest supported schema.
- A pre-migration backup is created for destructive transformations.
- Newer unsupported schemas fail closed without modification.

## Security and trust model

### Trust boundaries

1. Repository content is untrusted data, including instructions and tool output.
2. The workbench is a client, not an authorization authority.
3. The daemon is the policy and persistence authority.
4. Model providers are external recipients of selected data.
5. Spawned processes are untrusted and capability-scoped.
6. Rendered Markdown and command URIs are untrusted presentation input.

### Enforcement sequence

```text
mode permits capability?
  -> workspace grant permits capability?
  -> approval artifact still valid?
  -> path/command/network target within scope?
  -> operation limits valid?
  -> execute inside privileged implementation
  -> record redacted audit result
```

### Required adversarial cases

- Repository instructions request secrets, network, or policy changes.
- Symlink and case-normalization escape from the workspace.
- Paths traverse through `..`, alternate separators, or encoded forms.
- Git submodule or worktree points outside the trusted root.
- A tool name is aliased to a stronger capability.
- Model emits a malformed, oversized, duplicate, or late tool call.
- Cancellation races with file replacement.
- Hover Markdown attempts command execution.
- Tool output contains prompt injection or terminal escape sequences.
- Diagnostic logs receive tokens, environment variables, or source bodies.

## Daemon lifecycle

1. Extension activation remains lazy.
2. First Lantern command resolves a compatible bundled or configured daemon.
3. The workbench spawns one daemon per window for v0.1.
4. A random session nonce is supplied during initialization.
5. Initialization has a short timeout and actionable version errors.
6. Heartbeats run only while operations are active or views require freshness.
7. Unexpected exit marks active operations failed and offers a bounded restart.
8. Restart uses persisted state only after integrity and migration checks.
9. Window shutdown requests graceful daemon shutdown, then enforces a deadline.

Crash-loop protection disables automatic restart after three rapid failures and
preserves normal editor operation.

## Editor integration choices

- Use stable VS Code APIs compatible with the pinned VSCodium baseline.
- Use a command plus side view for Quick Ask; hover is a concise secondary
  surface because hovers have limited interaction and lifetime.
- Use `TextEditorDecorationType` only for transient evidence and tour ranges.
- Use `TreeView` for learning routes and plan/review navigation.
- Use virtual text documents and `vscode.diff` for initial staged review.
- Use `WorkspaceEdit` for validated multi-file edits, guarded by document
  versions and daemon content hashes.
- Treat webviews as isolated untrusted renderers with a strict content security
  policy; prefer native surfaces where possible.

## Architecture decisions to record as ADRs

| ADR | Decision | Spike evidence required |
| --- | --- | --- |
| 001 | Lantern editor workbench plus local daemon | lifecycle prototype and failure isolation |
| 002 | Rust daemon and TypeScript workbench | build size, startup, cross-platform packaging |
| 003 | JSON-RPC 2.0 over stdio | streaming, cancellation, framing, back-pressure |
| 004 | Narrow documented Code OSS integration surface | stable API coverage and internal bridge prototype |
| 005 | Replaceable `AgentDriver` | native mock and Pi-backed comparison |
| 006 | Daemon-enforced capabilities | denial tests at three enforcement layers |
| 007 | SQLite plus Markdown plans | transaction and hand-edit round-trip spike |
| 008 | Deterministic retrieval before embeddings | fixture evaluation and latency data |
| 009 | Staged semantic edits | stale-input and crash-recovery spike |
| 010 | Local-first state and explicit transmission | threat model and data-flow inventory |

## Phase 0 spike matrix

| Spike | Question | Artifact | Pass condition |
| --- | --- | --- | --- |
| S0-A | Can stdio RPC stream and cancel reliably? | workbench, daemon, protocol fixture | cancellation stops mock work within budget |
| S0-B | Which editor capabilities require internal integration? | Lantern workbench prototype | stable APIs are used where sufficient and internal bridges are justified |
| S0-C | Native loop or Pi adapter? | two `AgentDriver` implementations | same fixtures and policy behavior pass |
| S0-D | Can plans round-trip safely? | schema, parser, Markdown sample | hand edits and unknown fields are preserved |
| S0-E | Can staged edits recover? | patch/checkpoint prototype | stale input rejected; crash returns known state |
| S0-F | Are trust boundaries enforceable? | threat model and adversarial tests | denied capability is unreachable through all paths |
| S0-G | Does interruptible voice improve collaboration? | read-only realtime voice prototype | grounding, interruption, privacy, cost, and user-value gates pass |

## First vertical slice pull-request sequence

### PR 1: repository foundations

- Workspace manifests and pinned toolchains.
- Strict TypeScript and Rust checks.
- Minimal CI on Linux and macOS.
- Architecture test naming and fixture conventions.

### PR 2: protocol contract

- Canonical schema source.
- Generated Rust and TypeScript types.
- JSONL framing, size limits, version negotiation, and golden fixtures.
- Structured error and operation-event envelopes.

### PR 3: daemon lifecycle

- Rust stdio server with initialize, health, cancel, and shutdown.
- Cancellation registry and deterministic streaming task.
- Redacted tracing to stderr.

### PR 4: workbench lifecycle

- Lazy command registration.
- Daemon spawn, negotiation, crash handling, and shutdown.
- Output channel containing safe diagnostics.

### PR 5: selection context

- Active selection snapshot and version/hash metadata.
- Editor language-feature adapter with strict caps.
- Read-only request construction and cancellation bridge.

### PR 6: mock Quick Ask

- Deterministic `AgentDriver`.
- Streamed answer and one evidence item.
- Native answer view, navigation, and retry/cancel states.

### PR 7: policy proof

- Quick Ask capability set.
- Denial of write, execute, and non-model network operations.
- Adversarial tool-call fixtures and audit metadata.

### PR 8: end-to-end hardening

- Editor integration test in the pinned Code OSS/VSCodium baseline.
- Daemon crash, malformed record, slow client, cancellation race, and version
  mismatch scenarios.
- Latency report for both fixture repositories.

No PR in this sequence introduces SQLite, a real provider, full indexing,
planning UI, or file mutation.

## Definition of ready for permanent implementation

- Canonical journeys and budgets are accepted.
- All six Phase 0 spikes have recorded results.
- ADRs 001 through 007 are accepted or explicitly revised.
- The protocol contract passes generated-type and golden-fixture tests.
- The threat model names every privileged operation and enforcement point.
- A deterministic end-to-end Quick Ask works in VSCodium.
- Cancellation and daemon crash behavior meet their budgets.
- Native versus Pi-backed driver choice is evidence-based.
- Fixture repositories and evaluation questions are selected.
- License selection may remain open, but no incompatible dependency is adopted.

## Open decisions

| Decision | Needed by | Default if unresolved |
| --- | --- | --- |
| Exact VSCodium/Code OSS baseline | S0-B | latest stable at scaffold time, then pin |
| Rust async runtime | PR 2 | Tokio after a minimal dependency review |
| Protocol schema generator | PR 2 | JSON Schema as canonical source |
| Pi integration form | end of S0-C | native minimal loop behind `AgentDriver` |
| Database library | Phase 1 storage PR | SQLx with offline metadata evaluation |
| Plan metadata syntax | S0-D | constrained YAML front matter plus Markdown body |
| Bundled versus downloaded daemon | release packaging | bundled platform binary |
| Credential store | first real provider | OS keychain or provider environment lookup |
| Fixture repositories | before repository intelligence | compact owned fixtures first, public repos for evaluations |

## Immediate next actions

1. Convert ADR rows 001 through 007 into individual proposed records.
2. Define the protocol schema for PR 2 before scaffolding either implementation.
3. Build S0-A and S0-B as disposable spikes.
4. Specify the read-only tool set and policy test table.
5. Implement the same scripted repository question through the native mock and
   Pi adapter.
6. Record measurements and decide whether the first permanent driver uses Pi
   packages or a minimal native loop.
