# Helix terminal architecture spike

## Decision status

- **Status:** Historical evidence; frontend promoted by ADR 001
- **Date:** 2026-07-15
- **Question:** Can Lantern combine Helix, Lazygit, and a persistent agent pane
  into a smaller coherent product than the current Code OSS foundation?
- **Promoted artifact:** [`frontend/helix`](../frontend/helix/README.md)

The spike's selection-only, no-tools agent boundary was subsequently superseded
by ADR 003 and ADR 004. See [CURRENT_STATE.md](CURRENT_STATE.md) for the active
implementation direction.

## Proposed product surface

```text
+-------------------------------------------------+
|                                                 |
| Helix editing                              80%  |
|                                                 |
+-------------------------------------------------+
| Lantern agent and Git summary              20%  |
+-------------------------------------------------+
```

The surface is one terminal environment, not three applications with shared
mutable state:

- Helix owns buffers, selections, language intelligence, and normal editing.
- Lazygit owns interactive Git operations: staged and unstaged diffs, staging,
  commits, branches, pulls, rebases, and history.
- Lantern reads Git directly for its concise summary and audit evidence. It
  does not scrape or imitate Lazygit's internal state.
- The Lantern daemon owns model interaction, policy, evidence, plans,
  cancellation, and agent tool execution.

Lazygit temporarily covers a 10%-wide rail at the left edge of the upper Helix
region. The source and full-width 20% Lantern pane remain visible, and closing
Lazygit returns to the same Helix process with its buffers and undo history
intact. The rail uses the upper 80% height because a 10%-high popup falls below
Lazygit's nine-row minimum on common terminal sizes. It requires at least 120
terminal columns so its interior also meets Lazygit's width floor; narrower
terminals receive a visible error.

## Evidence collected

The current spike uses pinned source builds:

| Component | Revision | Observed behavior |
| --- | --- | --- |
| Helix | `14d6bc0febed9c692048271a8ae2362ac969c6e0` | Built as Helix 25.07.1; editing process remained alive across navigation and Lazygit use |
| Lazygit | `080da5cacfcff63a89ea23493bb91b11b0612876` | Built from source; opened in a 10% Git rail while Helix and Lantern remained alive |
| tmux | 3.4 | Created a full-width 80/20 vertical editor/agent composition and preserved it across resize |

Observed end-to-end behavior:

1. The repository summary visibly separated staged, unstaged, and untracked
   paths and showed the active branch and recent commits.
2. A deterministic `/show Lantern is a provisional name` request found
   `README.md:3` and opened that line in the existing Helix process.
3. Lazygit ran concurrently with the same Helix and Lantern processes.
4. Closing Lazygit returned to Helix without restarting it.
5. Repository escape attempts were rejected before contacting the editor.
6. All six repository script tests passed, including the three spike boundary
   tests.

## What the spike proves

The Git experience is achievable without embedding Lazygit or rebuilding its
features. The 80/20 terminal composition is small, fast, and understandable.
It also keeps normal editing useful when Lantern is idle or unavailable.

An agent can cause Helix to show a discovered file and line through a narrow
navigation action. The model should never emit terminal keystrokes directly;
it emits a typed intent such as:

```text
Navigate {
  repository_relative_path
  line
  optional_range
  evidence_id
}
```

The client validates the intent and performs the editor-native operation.

## What remains unproven

Stock Helix does not currently expose a supported external IPC or remote-control
API. The spike establishes a known mode and sends one validated `:open` command
through tmux. That is acceptable for deciding whether the experience is useful,
but it is not a production boundary.

The following still require evidence:

- Highlighting an exact evidence range rather than opening only its first line.
- Applying an external agent edit as one normal Helix transaction with undo.
- Streaming agent events without polling or terminal-key automation.
- Preserving the 80/20 experience on small terminals; the spike is most useful
  at 120 columns or wider.
- LSP-backed symbol and reference navigation initiated by the Lantern daemon.
- Cancellation latency and daemon lifecycle.
- Whether a 20% conversational pane remains readable during longer planning
  and explanation flows.

These are the criteria for a small Helix integration patch or fork. They are
not reasons to expand the tmux automation.

## Product constitution decision test

| Question | Spike answer |
| --- | --- |
| Does it help developers understand or write code? | It keeps code primary, makes repository state visible, and lets evidence navigate directly into the editor. |
| Does it preserve authorship and interruption? | Helix remains fully usable; the agent has explicit actions rather than ownership of the terminal input stream. |
| Is it the smallest coherent solution? | It composes Helix and Lazygit and adds only Lantern-specific state and validated bridges. |
| Does it add a fallback or parallel path? | No. This is an isolated experiment; a successful pivot deletes the Code OSS release path rather than maintaining two frontends. |
| Can it remain open and local-first? | All three components and the Lantern bridge are local and open source. Model providers remain explicit adapters. |
| What evidence validates it? | Live geometry, process, navigation, Git-surface checks, deterministic boundary tests, and the next daemon integration test. |
| What causes us to remove it? | Fragile navigation after a typed Helix integration attempt, unreadable core flows, or failure to support coherent undo and interruption. |

## Second gate result

The follow-up spike adds a session-scoped Rust daemon with a versioned JSONL
protocol. It now demonstrates:

- streamed operation, progress, evidence, text, completion, error, and
  cancellation events;
- bounded local literal search across at most 2,000 files and 512 KiB per file;
- symlink, binary, generated-output, and dependency-directory exclusions;
- exact one-based evidence ranges;
- an auditable Helix-native `:lantern-navigate` command that turns validated
  ranges into native selections;
- interruption while the Lantern pane retains focus; and
- deterministic cancellation and evidence contract tests.

The initial `/ask` path intentionally remains deterministic. The follow-up
`/agent` path calls one pinned Pi RPC driver using an eligible ChatGPT
subscription login owned by Pi. Pi starts without tools, sessions, extensions,
skills, templates, or ambient repository context; Lantern supplies the bounded
selection, one LSP definition, at most eight references, and the question. It
has no provider or evidence fallback.

Live acceptance results:

- `/show Lantern is a provisional name` streamed two evidence records and
  selected `README.md:3:1-3:30` in the existing Helix process.
- `/show understand`, followed by `/cancel`, produced a terminal cancellation
  event in **5 ms** while leaving the exact evidence selection visible in
  Helix.
- Lazygit opened in the Git rail while Helix, the Lantern pane, and the
  daemon remained alive; closing it returned to the same session.
- `/quit` removed the tmux session and all four scoped processes.
- Four Rust unit/contract tests and six repository script tests passed.
- A live `/bin/false` daemon probe left the Lantern pane visible with an
  actionable failure message, working Git action, and `Ctrl-d` exit instead of
  closing the pane or restarting silently.

Selection-context acceptance result:

- The native `:lantern-export-selection` command captured the primary Helix
  selection, including unsaved buffer text, into a private session-scoped file.
- `/ask What does this selected sentence establish?` delivered the exact
  `README.md:3:1-3:30` range and 29 selected characters to the daemon without
  placing source text in shell arguments or terminal output.
- The pane consumed and deleted the transient file; selection input is bounded
  to 64 KiB and repository-relative saved buffers.
- `/preview Lantern is the working product name.` displayed a unified diff in
  the upper work region and left `README.md` unchanged. Preview inputs were
  removed when the view closed.

Agent-driver contract result:

- Pi RPC text deltas map to the existing Lantern stream without putting source
  or questions in process arguments.
- `/cancel` sends Pi's RPC `abort`, including when cancellation races with
  driver startup; the deterministic boundary test completes under 500 ms.
- Source files remain unchanged and any Pi tool request fails the turn.
- The isolated DeepEval `quick_ask` v2 dataset adds a symbol-grounded case to
  the supported evidence, missing evidence, and embedded-instruction cases.
- A live Pi `0.80.6` / `openai-codex` / `gpt-5.4` run passed all three
  deterministic DeepEval contracts after calibrating one false-negative phrase
  match. Reports remain local and ignored.

Mouse and direct-interaction result:

- tmux mouse routing makes a click focus Helix, Lantern, or the Lazygit popup;
- Helix's native mouse path provides cursor placement, drag selection, and
  wheel scrolling;
- `Ctrl-a` exports the current Helix selection and focuses Lantern, where a
  plain question starts the bounded agent flow without `/agent` syntax;
- the Lantern pane uses one direct Crossterm dependency—not a widget
  framework—to provide clickable actions, scrolling, input editing, and
  clickable evidence while stripping terminal control sequences from model
  text; and
- Lazygit's pinned config explicitly enables its existing mouse events.

Explorer-preview result:

- the stock picker had consumed every mouse event and exposed no preview
  selection state;
- a second narrow Helix patch adds result-list click/wheel handling and a
  temporary source range in document previews using Helix's own document
  coordinate and highlight machinery;
- changing the picker result or query clears that temporary state; and
- `Ctrl-a` promotes the range through the existing picker callback into the
  real editor document, then bubbles to the configured export/focus binding.
  A live drag exported `AGENTS.md:5:5-5:26` with the exact selected text.

## Final gate result

- Real Helix plus rust-analyzer resolved the saved `resolved_port` call to its
  exact definition and three references. The bridge reported
  `Lantern captured one definition and 3 references`.
- The first live answer exposed a useful evidence defect: the exact definition
  range provided the signature but not its body, so the model correctly stated
  that it could not confirm `8080`.
- A four-line, 1,024-character maximum definition window fixed that grounding
  gap while reference excerpts stayed single-line and clickable ranges stayed
  exact. The repeated answer identified `8080`, the `u16` type, all three uses,
  and the limits of its surrounding context.
- DeepEval `quick_ask` v2 passed all four live cases through Pi `0.80.6`,
  `openai-codex`, and `gpt-5.4`.
- Six protocol/library invariant tests, two daemon unit tests, eight pane tests,
  twenty daemon integration tests, all 15 Helix library tests, strict Clippy,
  release builds, fixture tests, and byte-for-byte clean patch replay pass.

ADR 001 therefore accepts the Helix-centered frontend for v0.1. Code OSS is a
rejected alternative, not a maintained fallback. The next phase promotes the
spike's protocol, lifecycle, policy, and reproducible patch checks into the
maintained foundation.
