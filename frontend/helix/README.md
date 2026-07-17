# Lantern Helix frontend

## Status

ADR 001 accepted the Phase 0 terminal experiment as Lantern's frontend
direction. This directory contains the maintained Helix patches, configuration,
bridges, and reproducible upstream preparation contract.

Lantern provides a coherent terminal environment without rebuilding editor and
Git behavior:

```text
+-------------------------------------------------+
|                                                 |
| Helix                                      80%  |
|                                                 |
+-------------------------------------------------+
| Lantern agent and Git summary              20%  |
+-------------------------------------------------+
```

When requested, Lazygit appears as a 10%-wide rail at the left of the upper
Helix region. It does not cover the full-width agent terminal.

Helix remains the editing authority. Lazygit owns interactive Git operations.
Lantern owns agent state, policy, evidence, and the narrow commands that connect
an agent result to an editor location.

## User outcome under test

A developer can:

1. Edit normally in Helix while Lantern occupies a persistent, full-width 20%
   pane along the bottom.
2. Click between all surfaces, position or drag-select code in Helix, scroll
   each surface with the mouse, and use Lazygit controls directly.
3. See the current branch, staged files, unstaged files, untracked files, and
   recent commits without leaving the coding surface.
4. Open Lazygit as a narrow 10% Git rail, perform focused Git operations, and
   close it without losing the Helix session.
5. Ask the local Lantern daemon to locate a literal symbol or phrase, watch
   progress and evidence stream, and have the exact result range selected in
   the existing Helix process.
6. Ask a subscription-authenticated agent about a selected symbol using one
   LSP definition and at most eight references, watch its evidence and answer
   stream, and interrupt it without granting repository tools.
7. Interrupt an active answer and see measured cancellation latency.

`/ask` remains deterministic for testing the local boundary. `/agent` is the
explicit nondeterministic Pi RPC experiment and is evaluated separately with
DeepEval.

## Run

Requirements:

- `tmux` 3.2 or newer
- a terminal at least 120 columns wide for the 10% Lazygit rail
- the pinned Helix build at
  `.lantern/upstream/helix/target/release/hx`
- the pinned Lazygit build at
  `.lantern/toolchains/lazygit/lazygit`
- the maintained release runtime at `target/release`
- Pi `0.80.6`, authenticated privately for OpenAI Codex by starting `pi`,
  running `/login`, and choosing OpenAI Codex

Launch from the repository to inspect:

```bash
./scripts/launch-lantern.sh /path/to/repository
```

The current development workspace is already prepared. To reproduce the pinned
builds after checking out the two recorded upstream repositories, run:

```bash
./frontend/helix/prepare.sh
```

The Lantern pane starts focused. Click a pane to focus it. In Helix, click to
position the cursor, drag to select code, and use the wheel to scroll. The
workspace starts locked. Enter `/trust read` for local-only questions or
`/trust model` to additionally allow selected evidence to reach the configured
model for this session. `/trust none` revokes both grants. Press `Ctrl-a` after
selecting a saved symbol to resolve its bounded LSP context and focus Lantern;
type the question directly and press Enter. `Space-g` opens the 10% Lazygit
rail. Missing trust, LSP support, or a repository definition is a visible
error; Lantern does not substitute another capability or search path.

The initial Helix explorer is mouse-aware too. Click or wheel over the left
result list to choose a file, drag across source in the right preview, and
press `Ctrl-a`. Helix opens that previewed file with the exact dragged range
selected, exports it, and focuses Lantern in one keypress.

The quiet header keeps only clickable **Ask**, **Git**, and **Cancel** actions.
Refresh and Quit remain explicit commands instead of permanent chrome. The
response area scrolls with the wheel and evidence locations are clickable. Hold
`Shift` while dragging when the terminal emulator's native text selection is
needed.

Diagnostic commands remain available:

- `/trust` shows the current session access.
- `/trust read` allows bounded local repository reads without model
  transmission.
- `/trust model` allows bounded local reads and selected-evidence transmission
  to the configured model.
- `/trust none` immediately returns the idle session to locked state.
- `/git` opens Lazygit.
- `/ask <question>` captures the current primary Helix selection through the
  session-scoped bridge and streams its grounded, deterministic acknowledgement.
- `/agent <question>` sends the selection, one LSP definition, and at most
  eight LSP references to the pinned Pi RPC driver using the selected
  `openai-codex` model. Each compact evidence row explains whether it is the
  selection, definition, or a bounded reference. Every range is clickable. Pi
  receives no tools, extensions, skills, session, or ambient repository
  context.
- `/preview <one-line replacement>` shows a transient unified diff for the
  selected text; closing it leaves the repository unchanged.
- `/show <literal text>` streams bounded local evidence and selects its exact
  range in Helix.
- `/cancel` interrupts an active stream and reports local cancellation latency.
- `/diagnostics` explicitly exports bounded, metadata-only diagnostics to a
  private file in the system temporary directory. It remains available after a
  daemon crash and excludes prompts, source, paths, environment values,
  provider stderr, and all unstructured output.
- `/refresh` refreshes repository state.
- `/quit` closes the Lantern session.

Press `Ctrl-d` from an empty, idle Lantern prompt to close the session without
typing `/quit`. `Esc` remains the interruption shortcut while an agent turn is
active; quitting never silently abandons active work.

With an empty idle prompt, `Up` and `Down` cycle through prior evidence and
`Enter` opens the highlighted exact range in Helix. `Esc` returns to the input
prompt. This navigation operates only on evidence already in the pane; it does
not start a repository search or model request.

The launch command fails with an actionable error when a required binary is
missing. It does not silently substitute another editor or Git interface.

## Visual language

The terminal composition uses Helix's built-in Boo Berry palette across every
surface: a single deep-plum canvas, muted lilac borders and metadata, mint
actions, violet evidence links, and one bright text hierarchy. tmux pane titles
and heavy button brackets are deliberately absent. Two-column Helix pickers,
the 10% Lazygit rail, and the Lantern pane use the same low-contrast framing so
code and evidence remain the visual focus.

The palette is intentionally terminal-native and contains no image assets or UI
framework. Interaction remains visible through color, cursor state, selection,
and exact hit targets rather than decorative chrome.

## Pass criteria

- The initial tmux layout is 80% editor above a full-width 20% Lantern pane,
  within one terminal row of rounding.
- Git summary sections distinguish staged, unstaged, and untracked paths.
- Lazygit uses a 10%-wide rail confined to the upper 80% work region, and
  returning from it preserves Helix's open buffers and undo history.
- A validated `path + range` action selects that exact evidence in the existing
  Helix process without taking input focus from the agent pane.
- Paths outside the repository and malformed line numbers are rejected before
  an editor command is sent.
- The daemon is scoped to the tmux session and leaves no process after the
  session closes.
- Initialization either completes within two seconds or leaves a visible,
  actionable unavailable state while Helix and Lazygit remain usable.
- Unexpected daemon exit never disappears behind an automatic restart; the
  pane shows a bounded diagnostic tail and preserves explicit session exit.
- Cancellation reaches a terminal event within 500 ms locally.
- Pi authentication and protocol failures are visible and never cause an
  automatic provider fallback.
- The workspace starts locked; read and model transmission grants are visible,
  separate, session-local, revocable, and enforced before operation admission.
- Repository write and process execution requests are hard denials in Quick
  Ask and never enter an approval queue.

## Fork criteria

Stock Helix currently exposes shell commands and editor-context expansions but
does not expose a supported remote-control or IPC interface. Lantern therefore
carries two auditable Helix patches: one adds typed navigation, selection
export, and bounded LSP-context commands; the other adds generic picker mouse
interaction. tmux delivers typed commands only. Range conversion and selection
happen inside Helix.

The maintained patch set remains justified while any of these are true:

- Delivering the typed command remains fragile across editor modes or focus.
- External edits cannot be presented and accepted as coherent editor
  transactions with normal undo behavior.
- Streamed state becomes unreadable in the 20% surface.

The tmux bridge must be deleted rather than promoted if a typed Helix boundary
is introduced.

## Deliberate exclusions

- Agent-authored file changes.
- Git mutation outside Lazygit.
- Voice input and narration.
- Broad or unrelated Helix changes.
- Windows support.
