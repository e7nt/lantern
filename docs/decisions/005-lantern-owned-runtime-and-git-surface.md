# ADR 005: Spike Lantern-owned agent and Git control surfaces

- **Status:** Proposed; promotion requires both bounded spikes
- **Date:** 2026-07-18
- **Decision owner:** Lantern project

## Context

ADR 004 retained Pi until it failed a measured requirement. Guided Build now
requires Lantern to stop before repository mutation, present a developer-editable
plan, and resume only after confirmation. Pi's CLI RPC stream reports tool
execution but does not give the client a pre-execution gate. The Pi SDK does
provide explicit tools, in-memory sessions, subscription-backed model runtime,
and blocking `tool_call` hooks.

Lazygit proved the terminal Git rail, but its complete interactive surface is
larger than Lantern's review journey. Keeping it would make advanced Git states
and shortcuts part of Lantern's permanent UX even though most are unused.

## Proposed direction

Spike two replaceable Lantern-owned surfaces while borrowing proven concepts
and code boundaries rather than recreating their entire products.

1. A private adapter process uses pinned Pi SDK packages for authentication,
   provider calls, streaming, conversation, and compaction. Lantern owns tool
   definitions, pre-execution control, plans, cancellation, evidence, and typed
   lifecycle events.
2. A focused Git rail uses the Git CLI as its only mutation boundary. It owns
   status, review, staging, committing, branches, and synchronization needed by
   the accepted external-edit journey.

The current Pi RPC and Lazygit paths remain the maintained product until their
respective spikes pass. Promotion replaces each old path; Lantern will not keep
parallel permanent implementations.

## Agent-runtime promotion gate

- Existing Pi/OpenAI Codex authentication works without Lantern credentials.
- A mutation is blocked before execution and leaves source byte-identical.
- Confirmation allows exactly the requested mutation in the same session.
- Streaming, interruption, context isolation, and the existing DeepEval cases
  pass.
- Lantern-owned overhead adds no more than 150 ms to first useful activity.
- The promoted adapter removes the RPC workaround instead of wrapping it.

## Focused Git rail

The first retained operations are:

- active branch and concise staged, unstaged, untracked, and conflicted paths;
- file and hunk diff review;
- stage and unstage one file or hunk;
- open the selected range in the existing Helix process;
- create and switch a local branch;
- write a commit with a developer-supplied message;
- fetch and fast-forward pull with divergence shown explicitly; and
- inspect a bounded recent commit list and one commit diff.

The initial surface excludes push, force operations, discard/reset, stash
management, rebase, cherry-pick, bisect, remote administration, submodule
management, commit amendment, and an embedded conflict editor. Conflicts are
visible and open in Helix. Destructive operations remain possible through an
explicit developer-requested agent or terminal command, not the focused rail.

## Git-rail promotion gate

- The external edit journey can inspect, stage, unstage, commit, branch, and
  fast-forward pull using keyboard and mouse.
- Every mutation is a typed Git command with captured exit status and an
  actionable error; no porcelain output is parsed as prose.
- Staged and unstaged diffs remain exact, large output is bounded, conflicts
  are never hidden, and the repository scope cannot escape the workbench.
- The rail starts faster and uses less memory than pinned Lazygit on the same
  fixture.
- Promotion removes the Lazygit binary, configuration, preparation, launcher,
  and tests in one checkpoint.

## Consequences

This proposal does not authorize a general agent framework or a broad Git
client. Failure of either promotion gate retains its current adapter and deletes
that spike. Successful promotion requires an accepted follow-up ADR with the
measured results and migration plan.
