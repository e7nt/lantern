# Workbench explorer acceptance — 2026-07-21

## User outcome

Lantern now starts with a quiet folder tree beside Helix instead of Helix's
flat startup picker. The upper work region is 20% explorer and 80% editor; the
full-width agent remains below. A developer can expand folders, open a file in
the existing Helix process, return with `Space-e`, and use the same flow with a
mouse.

The tree is derived from Git's tracked and visible untracked inventory. It
does not traverse `.git`, dependency caches, or ignored private files through
a separate ignore implementation. Compact text marks expose conflicted,
staged, modified, staged-plus-modified, and added files. Collapsed folders
aggregate descendant state. Bounded submitted review comments from the current
agent-to-Git handoff appear as `·N` on the file and its ancestors.

## Deliberate boundary

Pinned Helix keeps ownership of buffers, selections, editing, undo, and LSP.
The explorer is a dependency-light Rust terminal surface and opens code only
through Lantern's existing validated `path + range` command. It cannot rename,
delete, create, drag, stage, or edit files. Focused Git remains the only Git
mutation and detailed review surface.

Every developer supplies their own model identity. Lantern delegates to Pi's
private OpenAI Codex login and contains no shared API key, credential field, or
provider fallback. External and in-agent help now state the first-use `pi` →
`/login` path explicitly.

## Evidence

- Rust tests cover folder-first ordering, nested expansion, combined Git
  state, aggregate state, review counts, and Git-ignore behavior.
- Terminal contract tests cover the persistent 20% pane, mouse support, and
  `Space-e` focus without rebuilding the layout.
- A real detached tmux journey produced a 29-column explorer beside a
  119-column Helix pane above the 149-column agent, expanded nested folders,
  opened `.github/workflows/ci.yml` in the existing Helix process, and returned
  focus between both panes without rebuilding the session.
- The canonical project check passes in full.
