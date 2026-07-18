# External edit journey

This is the completion contract for Lantern's first full coding journey. It
must run against a disposable repository outside the Lantern source tree so
the product is evaluated as a workbench rather than through self-reference.

## Developer experience

1. Launch Lantern on the fixture repository.
2. Ask for the documented small behavior change from the empty prompt or the
   `Ctrl-a` composer.
3. See concise, replace-in-place activity while Pi inspects the relevant code.
4. See the changed location open in Helix after the edit.
5. Read a short explanation in the compact pane or press `F2` for full-screen.
6. See the focused verification complete.
7. Open `Space-g` or `/git` and review the exact unstaged diff.
8. Press Esc during a second run and see the operation settle without leaving
   a child process, socket, or temporary runtime directory.

## Deterministic contract

The checked-in fake Pi journey must prove, in order:

```text
read -> edit -> bash -> completed -> settled
```

It operates on a newly created external Git repository, changes the requested
file, runs that repository's focused test, and leaves the change unstaged for
review. Tests must assert the file content and Git diff rather than trusting
tool event narration.

## Live contract

The explicit subscription-backed run records model, Pi version, ordered tools,
first-tool latency, first-text latency, settlement latency, changed paths,
focused command result, and whether interruption settled within budget. Source,
prompts, command output, credentials, and absolute paths are not committed.

The live run is a promotion check, not an ordinary open-source CI requirement.
There is no simulated-success or provider fallback path.
