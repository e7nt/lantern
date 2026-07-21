# Multi-place plan review acceptance — 2026-07-21

## User outcome

A developer can review the active Markdown plan like a small pull-request
review without leaving Helix or submitting comments one at a time:

1. Select text in `.lantern/plans/active.md` and press `Ctrl-a`.
2. Type a comment and press `Ctrl-r` to add it to the local collection.
3. Repeat at up to 32 saved locations.
4. Say `Review these comments` to request one coherent plan revision.
5. Inspect the existing full-width diff preview, then say `Apply that`.

The model is not contacted while comments are collected. Review runs through
the read-only Pi profile with no edit, write, or shell tools. The original plan
does not change until explicit application.

## Contracts proved

- Protocol v13 bounds every comment, the collection, and every exact anchor.
- Anchors must target the saved active plan and still match its current bytes.
- One model turn captures one complete plan body and exposes it only as a
  `ChangeProposal`.
- Application fails if the plan changed after review, preserving developer
  edits instead of merging or overwriting them silently.
- Failed and cancelled reviews preserve the terminal's local comment
  collection; a successful proposal consumes it.
- DeepEval deterministic contracts reject partial revisions, missing plan
  sections, Markdown wrappers, frontmatter, and implementation claims.

## Deliberate limits

Comments are session-local and summarized compactly in the agent transcript;
Lantern does not patch Helix with inline comment decorations. There is one
pending revision, no review history, reviewer identity, discussion threads, or
remote collaboration. These surfaces remain excluded until a real journey
demonstrates a concrete need.

## Reference choices

Lantern adopts Helix-owned exact selections, OpenCode-style single admission,
and the pending multi-comment submission concept familiar from pull-request
reviews. It rejects a PR sidebar/server and keeps Pi as the measured read-only
model harness rather than introducing a parallel agent loop.
