# Agent-change review handoff

## Outcome

An agent turn that successfully edits or writes repository files now ends with
one compact instruction to review those changes. The next `Space-g` from Helix
or `/git` from Lantern opens the existing focused Git rail on the first touched
path that still has a live Git change.

## Boundary

- The terminal records paths only from typed successful edit/write events for
  the active operation.
- The one-shot payload is repository-relative, unique, limited to 64 paths and
  16 KiB of path data, and published atomically in the private runtime folder.
- The Git rail consumes and validates the payload, then checks it against fresh
  Git status. It does not claim that every hunk in a touched file was authored
  by the agent, and it never hides unrelated repository changes.
- Existing hunk resume context wins when it already points into the touched
  set, preserving the Git-to-agent-to-Git review loop.
- This adds no protocol event, model prompt, durable session record, dependency,
  or second review surface.

## Deterministic evidence

- Protocol tests reject empty, duplicate, escaping, oversized-path, and
  over-count focus payloads.
- Terminal tests prove a settled edit publishes one bounded focus and exposes
  the compact review instruction.
- Git rail tests prove the one-shot focus selects the first path that remains
  changed and is removed after consumption.
- Shell interaction tests prove the private focus path reaches the existing
  10% Git popup.

No DeepEval case is required: this slice contains no model-mediated behavior.
