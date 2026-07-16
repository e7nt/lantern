# Reference repositories

## Purpose

Lantern studies upstream projects as architectural references. Their source is
not vendored into Lantern's authored modules, and Lantern must not depend on
private implementation details without an explicit architecture decision.

Reference clones live under the ignored `.lantern/upstream` area. Inspection
does not execute build scripts or install hooks. The separately governed Helix
and Lazygit spike builds are reproducible product inputs, not an exception that
allows reference projects to run arbitrary setup code.

## Adoption discipline

Before Lantern invents a permanent protocol or interaction, inspect the
smallest relevant surface in these projects and record:

1. the upstream behavior and exact revision;
2. the user problem it solves;
3. what Lantern adopts;
4. what Lantern deliberately rejects; and
5. the Lantern test or evaluation that proves the adopted behavior.

Reference quality is not feature quantity. Lantern does not inherit a plugin
system, session model, tool surface, framework, or fallback merely because a
successful project has one.

## Pinned inspection revisions

| Project | Repository | Revision | Role in the investigation |
| --- | --- | --- | --- |
| VSCodium | `https://github.com/VSCodium/vscodium.git` | `12b08023848522f884b04173ac0894849dd9fa05` | Distribution constraints, Code OSS customization, extension-gallery behavior, and VSCodium compatibility |
| Code OSS | `https://github.com/microsoft/vscode.git` | `ff5e57008bf59618c869f30b33f38d40bd02e921` | Stable extension API, extension-host behavior, editor integration, cancellation, workspace trust, and extension tests |
| Pi | `https://github.com/earendil-works/pi.git` | `c6d8371521fc8357958bb21fd43552c15f46c7f4` | Agent loop, providers, tools, streaming, cancellation, sessions, compaction, and process-integration boundaries; re-inspected 2026-07-16 |
| OpenCode | `https://github.com/anomalyco/opencode.git` | `c69abee0c73253aebae65e87e4e1b9bfa8c38021` | TUI/backend separation, durable prompt admission, interruption, keymaps, event synchronization, and prompt race handling; inspected 2026-07-16 |
| Helix | `https://github.com/helix-editor/helix.git` | `14d6bc0febed9c692048271a8ae2362ac969c6e0` | Editor-owned selection, navigation, LSP intelligence, picker interaction, themes, and modal keymaps |
| Lazygit | `https://github.com/jesseduffield/lazygit.git` | `080da5cacfcff63a89ea23493bb91b11b0612876` | Focused Git interaction, terminal mouse behavior, compact information hierarchy, and theme configuration |

These revisions are evidence anchors, not dependency pins. A later spike should
record the inspection date and re-check upstream behavior before an
implementation decision is finalized.

## Initial findings

The VSCodium and Code OSS findings below preserve the rejected frontend
investigation. [ADR 001](decisions/001-helix-terminal-frontend.md) supersedes
their implementation consequences; they are not an active parallel path.

### VSCodium

The VSCodium repository is a collection of scripts, patches, product settings,
and release automation that builds Microsoft's `vscode` repository into a
freely licensed distribution. It is not a maintained fork containing the Code
OSS editor implementation.

Consequences for Lantern:

- The public VS Code extension API is the intended integration boundary.
- VSCodium is the correct compatibility and distribution target.
- Code OSS extension-host behavior must be investigated in `microsoft/vscode`
  or through the pinned public API documentation, not inferred from VSCodium's
  build scripts.
- Open VSX availability and extension licensing must be included in release
  planning.
- Lantern should avoid proposed APIs unless a documented capability cannot be
  delivered through stable APIs and the compatibility cost is accepted.

### Code OSS

The pinned stable extension declarations expose the core editor capabilities
needed by the first Lantern slices: selection-change events, hover providers,
editor decorations, tree views, virtual text documents, workspace edits,
language-feature commands, cancellation tokens, and workspace trust state.

Consequences for Lantern:

- The extension can remain a normal stable-API extension for the initial
  experiences; an editor fork is not justified by the current requirements.
- Editor cancellation tokens must be bridged to daemon request cancellation.
- Virtual documents plus the built-in `vscode.diff` command are the preferred
  first implementation for staged and review diffs.
- Workspace trust is an editor signal, not Lantern's complete authorization
  model. The daemon must independently enforce read, write, execute, and network
  grants.
- Language intelligence arrives through editor commands and providers and must
  be normalized into editor-neutral protocol types.
- Markdown rendered in hovers and views must remain untrusted by default;
  command links require an explicit allowlist.

### Pi

Pi separates provider access, a stateful agent loop, and the coding-agent
application. Its agent core exposes streamed lifecycle events, typed tools,
abort propagation, context transformation, tool-call preflight and
post-processing, steering/follow-up queues, and configurable sequential or
parallel tool execution. The coding agent supports RPC and SDK integration in
addition to its terminal UI.

Consequences for Lantern:

- Pi is a useful behavioral reference for Lantern's `AgentDriver` contract.
- Runtime policy must remain outside the model prompt. Pi's `beforeToolCall`
  seam demonstrates one enforcement point, but Lantern also needs capability
  checks at tool registration and inside security-sensitive tool
  implementations.
- Pi session formats, UI concepts, and extension loading should not become
  Lantern's durable storage contract.
- Cancellation must cover provider streams and running tools, then propagate
  across Lantern's editor/daemon RPC boundary.
- Pi's event vocabulary is a strong input to Lantern's streamed protocol, but
  Lantern also needs request correlation, protocol versioning, back-pressure,
  audit events, and redaction guarantees.
- The Phase 0 comparison should evaluate both embedding Pi's agent packages and
  implementing the same narrow `AgentDriver` contract natively.

The 2026-07-16 protocol inspection adds four concrete requirements:

- Keep strict LF-delimited JSONL framing and request correlation.
- Distinguish prompt acceptance from asynchronous execution events.
- Treat the last run event and the fully idle/settled boundary as different
  states; shutdown and follow-up work must wait for settlement.
- Keep steering and follow-up as different future concepts. Lantern will not
  expose either until an interruptible workflow needs them.
- Serialize protocol stdout, wait for downstream writes rather than building an
  unbounded event queue, and flush output before process exit.
- Continuously drain child stderr while retaining only a bounded diagnostic
  tail; stopping reads at the retention limit can deadlock the child.

Pi's `Escape` cancellation, queued-message restoration, and explicit hotkey
listing are interaction references. Lantern adopts immediate `Escape`
interruption and discoverable shortcuts, but rejects Pi's extensions, skills,
session tree, ambient context, and tool registry for read-only Quick Ask.

### OpenCode

OpenCode is a strong reference for isolating a terminal client from backend
implementation. Its TUI extraction requires the SDK to be the boundary and
forbids presentation code from importing server or tool implementations. It
also makes runtime paths and capabilities explicit, uses named configurable
key actions, separates durable prompt admission from execution wake-up, and has
a regression test preventing concurrent submit calls from creating empty or
phantom sessions.

Consequences for Lantern:

- The Lantern pane may depend only on the canonical protocol client, never
  daemon internals or provider types.
- Runtime paths, terminal capabilities, and session identity must be explicit
  initialization data rather than process-global discovery inside UI code.
- Prompt submission needs an admission state and a double-submit test before
  the protocol becomes permanent.
- Interruption targets active execution; interrupting an idle session is a
  harmless no-op at the daemon boundary.
- Keybindings should name actions independently of default keys so a later
  user configuration can remap behavior without changing commands.
- Daemon startup needs an explicit ready boundary, a deadline, and an
  early-exit error. Unexpected exit must remain visible without silently
  recreating lost operation state.

Lantern rejects OpenCode's HTTP/SSE server shape, large plugin surface, broad
session management, multi-agent selectors, and UI framework as defaults. Those
solve a broader product and would obscure Lantern's understanding-first path.

### Helix

Helix remains the authority for buffers, selections, language-server position
encoding, navigation, picker state, undo, and modal input. Lantern adopts typed
editor commands and native ranges instead of reproducing editor semantics in
the daemon. The two documented patches are removal-oriented seams, not a
parallel editor abstraction.

### Lazygit

Lazygit remains the authority for Git mutation and detailed staged, unstaged,
commit, branch, pull, and diff interaction. Lantern adopts its compact,
keyboard-and-mouse terminal surface and does not build a second Git client into
the agent pane. The agent may explain Git evidence later, but it does not
silently perform Git operations.

## Foundation adoption record

| Area | Reference behavior adopted | Scope deliberately rejected | Required Lantern proof |
| --- | --- | --- | --- |
| Framing | Pi strict LF JSONL, serialized writes, and downstream back-pressure | Pi's full RPC command set and an additional application event queue | v2 golden fixtures, 256 KiB event bound, malformed/oversized/Unicode recovery tests, and single-producer admission |
| Lifecycle | Pi end-versus-settled distinction and OpenCode explicit sidecar readiness/exit | implicit long-lived sessions, polling health, and automatic restart | accepted/outcome/settled ordering, joined shutdown, startup-state tests, and a live early-exit terminal probe |
| Prompt admission | OpenCode single admission before execution | durable multi-session inbox in Quick Ask | pane reservation test plus duplicate-active-ID daemon test |
| Client boundary | OpenCode TUI through SDK/protocol only | UI imports of daemon/provider internals | architecture dependency test |
| Editor context | Helix-native selection, LSP, and navigation | daemon recreation of editor semantics | exact Unicode range fixtures and live LSP trace |
| Git | Lazygit owns mutation and detailed review | agent-pane Git implementation | same-session Git interaction and no agent Git tools |
| Keybindings | named actions, immediate Escape interrupt | a large configurable command system in Phase 1 | shortcut conflict and state-transition tests |

The protocol proofs above landed in Phase 1 foundation slices on 2026-07-16.
The canonical contract is [Protocol v2](../protocol/v2/README.md). It keeps the
pane busy through settlement without rendering acceptance as UI noise, bounds a
frame at 1 MiB, drains malformed frames before continuing, prevents an active
ID from being replaced, and joins daemon workers during shutdown. The follow-up
slice admits one operation, bounds outbound events and diagnostic tails,
continuously drains Pi stderr, and keeps an actionable pane visible after
startup timeout or daemon exit. Cancelling an already-settled ID is the
intentional idempotent no-op adopted from OpenCode.

## Next inspections

1. Inspect only the permission-denial paths relevant to read-only Quick Ask;
   do not import their general tool systems.
2. Inspect structured crash-report redaction before promoting diagnostics out
   of the spike runtime.
3. Re-check upstream revisions when a finding becomes a permanent decision and
   keep the evidence fixture with Lantern's corresponding test.
