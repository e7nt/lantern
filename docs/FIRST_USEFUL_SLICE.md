# First useful slice: evidence-backed Quick Ask

## Outcome

The first useful Lantern software is a bootable Lantern editor in which a
developer can open a local repository, approve read access, select code, ask a
question, receive a streamed evidence-backed answer, navigate to supporting
code, and cancel immediately.

```text
Open repository
  -> establish read and transmission permissions
  -> select code
  -> ask a question
  -> inspect an evidence-backed streamed answer
  -> navigate into the execution path
  -> interrupt safely
```

This slice tests Lantern's smallest product promise:

> Can Lantern help a developer understand real code better, with evidence,
> while remaining lightweight and under their control?

## User-visible behavior

### Open

- Lantern opens an ordinary local repository.
- Normal editing works before the daemon starts and if Lantern is disabled.
- The Lantern surface explains whether repository reads and model transmission
  are enabled.
- File writes and process execution are unavailable to Quick Ask.

### Ask

- The developer selects a non-empty code range.
- They invoke `Lantern: Ask About Selection`.
- Lantern captures the document version, selection, containing symbol where
  available, and bounded surrounding context.
- The answer surface opens without replacing the editor.
- Local progress and model content stream visibly.

Initial supported questions include:

- What does this code do?
- Where does this value come from?
- What calls this symbol?
- What does this return or mutate?
- Which test demonstrates this behavior?
- What is uncertain from the available evidence?

### Inspect evidence

- Important claims link to files, symbols, and ranges.
- Activating evidence opens the exact location.
- Claims are classified as observed, inferred, unknown, or contradictory.
- Stale or missing evidence is visible.
- Lantern never presents selection-only reasoning as symbol-backed analysis.

### Interrupt and recover

- Escape or Cancel stops speech or text generation and requests daemon
  cancellation.
- No new tool starts after cancellation.
- Late results do not alter the terminal answer state.
- A daemon crash produces an actionable error and does not affect ordinary
  editing.

## Deliberate exclusions

The first slice does not include:

- Repository-wide indexing.
- SQLite persistence.
- Feature briefs or planning.
- Guided learning missions.
- Guided Build or file mutation.
- Command execution.
- Live Collaboration voice.
- Multiple agents.
- Multiple production model providers.
- Automatic provider, model, retrieval, or tool fallbacks.
- Elaborate settings, onboarding, or extension ecosystem work.

## Repository shape

```text
lantern/
├── frontend/
│   └── helix/
│       ├── patches/             # narrow editor-owned integration
│       ├── config/              # Helix, Lazygit, and terminal composition
│       └── upstream.json        # immutable upstream revisions
├── apps/
│   └── daemon/                  # Rust executable
├── crates/
│   ├── protocol/
│   ├── policy-engine/
│   ├── agent-runtime/
│   └── repository-tools/
├── evaluations/
├── fixtures/
├── docs/
└── scripts/
```

The initial editor build uses pinned Helix and Lazygit sources plus a narrow
Lantern patch layer. The upstream source is not copied into Lantern's authored
modules. Each patch records its product reason, upstream base, affected
boundary, validation, and removal condition. ADR 001 records why the successful
terminal spike replaces the proposed Code OSS release path rather than creating
a second frontend. The editor/daemon boundary remains unchanged.

## Delivery sequence

### FS-01: Buildable Lantern editor

Deliver:

- Pinned Helix and Lazygit provenance.
- Reproducible local source preparation and clean patch replay.
- Native Helix selection, navigation, LSP-context, and picker interaction.
- Full-width Lantern pane and narrow Lazygit rail.
- Linux development build; macOS validation remains a release gate.
- Patch inventory and upstream-update script contract.

Acceptance:

- A fresh checkout can prepare and launch the development editor using
  documented commands.
- The prepared binaries match the recorded upstream revisions.
- Lantern-authored changes are distinguishable from upstream source.
- No model or daemon starts during ordinary editor launch.

### FS-02: Protocol contract

Deliver:

- Canonical schemas for `initialize`, `session/open`, `selection/ask`,
  `operation/cancel`, `operation/event`, `editor/contextRequest`,
  `daemon/health`, and `daemon/shutdown`.
- One canonical Rust request/event contract validated by shared wire fixtures.
- LF-delimited JSON-RPC framing.
- Version, size, sequence, and structured-error rules.
- Shared golden fixtures.

Acceptance:

- Client and daemon pass the same protocol fixtures.
- Incompatible versions fail clearly.
- Malformed and oversized records are rejected without crashing.

### FS-03: Streaming mock daemon

Deliver:

- Rust stdio daemon.
- Initialization, health, cancellation registry, and shutdown.
- Deterministic scripted `AgentDriver`.
- Safe structured diagnostics on stderr.

Acceptance:

- A scripted answer streams in ordered events.
- Cancellation reaches terminal state within the local budget.
- Shutdown cancels active work and exits within its deadline.

### FS-04: Editor-to-daemon Quick Ask

Deliver:

- Lazy daemon startup.
- Selection and document snapshot.
- Lantern answer surface.
- Stream rendering, evidence navigation, cancellation, and crash state.

Acceptance:

- The user completes the full mocked Quick Ask journey.
- Normal editing remains usable after daemon failure.
- No repository mutation occurs.

This is the first internally usable milestone.

### FS-05: Read-only tools and policy

Deliver:

- Canonical path handling.
- Bounded discovery, search, and reads.
- Normalized symbols, definitions, references, and diagnostics.
- Quick Ask capability set and audit metadata.
- Denial tests for writes, execution, network, sensitive paths, and workspace
  escapes.

Acceptance:

- Unauthorized operations are unreachable at tool exposure, preflight, and
  privileged implementation layers.
- Reduced syntax-only operation is visibly identified and never presented as
  editor-backed language intelligence.

### FS-06: Minimal production agent loop

Deliver:

- One small native loop behind `AgentDriver`.
- One explicit user-configured model-provider adapter.
- Tool-call validation, bounded turns, usage reporting, and provider errors.
- Explicit credential resolution without persistence.

The loop is:

```text
question
  -> assemble bounded context
  -> call selected model
  -> validate requested read-only tool
  -> authorize and execute tool
  -> return structured result
  -> produce evidence-linked answer
```

Pi remains a reference and evaluation candidate. It is not embedded unless a
separate spike demonstrates concrete benefit over this smaller native path.

Acceptance:

- A real selection question completes with the configured provider.
- Provider failure is reported directly; Lantern does not switch providers or
  models automatically.
- Turn, tool, context, and output limits are enforced.

### FS-07: Evidence validation

Deliver:

- Structured claims and evidence records.
- Observed, inferred, unknown, and contradictory classifications.
- Path, symbol, range, content hash, freshness, and selection-reason metadata.
- Pre-render validation and stale-evidence state.

Acceptance:

- Every important durable claim links to valid evidence or explicit
  uncertainty.
- Evidence navigation reaches the expected content.
- Changed content invalidates stale evidence.

### FS-08: Behavioral evaluation and usefulness gate

Deliver:

- Curated TypeScript and Rust fixture questions.
- Human-reviewed evidence sets and forbidden claims.
- DeepEval metrics for faithfulness, relevance, evidence quality, uncertainty,
  concision, and understanding value.
- Local latency, token, cost, and cancellation report.
- Short structured usability review.

Acceptance:

- All hard invariants pass.
- Calibrated behavioral metrics meet their initial thresholds across repeated
  runs.
- Human review finds the answers materially more useful than showing the
  selection to a generic model without repository evidence.
- Failure states remain honest and understandable.

## Internal contracts

### Selection request

- Repository and workspace identifiers.
- Document URI and normalized relative path.
- Language ID and document version.
- Selection range and selected text.
- Dirty-document state and content hash.
- Current trust and transmission state.

### Answer

```text
Answer
  summary
  claims[]
    text
    classification
    evidence_ids[]
  evidence[]
    repository_id
    relative_path
    symbol
    range
    content_hash
    reason_selected
    freshness
```

### Initial model policy

- One active provider selected by the user.
- No automatic model change.
- No automatic provider change.
- No write, process, Git-mutation, or arbitrary-network tool.
- Bounded turns, context, output, tool calls, and wall time.
- Clear terminal error when the configured path cannot complete.

## Definition of done

The slice is complete when a developer can use a locally built Lantern editor
on both reference fixtures and:

1. Understand the current access and transmission state.
2. Ask a useful question about selected code.
3. Inspect evidence for important claims.
4. See uncertainty and missing intelligence honestly.
5. Navigate to the next relevant code location.
6. Cancel without lingering work.
7. Continue normal editing after Lantern failure.
8. Verify through tests that Quick Ask cannot modify or execute.

Planning, learning, Guided Build, and voice work begin only after this slice
passes its usefulness gate.
