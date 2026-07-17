# Lantern engineering standard

## Intent

Lantern aims to be exemplary open-source software: code that contributors can
understand, verify, maintain, and learn from. This standard turns that ambition
into an acceptance gate. It applies to Lantern-authored code, scripts, schemas,
documentation, tests, and upstream patches.

The standard favors evidence over ceremony. A checklist item must protect a
real property of the change; process that cannot explain its value is bloat.

## Design principles

- Build the smallest coherent solution to a named user problem.
- Give each module one clear responsibility and make ownership boundaries
  visible in its API.
- Make invalid states difficult to represent with strict types, validation, and
  explicit state transitions.
- Prefer plain control flow and descriptive names over cleverness.
- Add an abstraction only after a real boundary or repeated concept exists.
- Keep provider behavior, editor integration, tools, and durable state behind
  narrow interfaces without hiding meaningful differences.
- Preserve root causes in errors and fail visibly. Do not add silent fallbacks.
- Comment intent, constraints, and non-obvious tradeoffs—not syntax.

## Definition of Done

A change is complete only when every applicable item below is supported by
reviewable evidence.

### Purpose and scope

- The user outcome and acceptance criteria are stated.
- The change passes the product constitution's decision test.
- Unrelated refactors, compatibility paths, and speculative hooks are absent.
- New operational or conceptual cost is justified; obsolete code is removed.

### Architecture and implementation

- Responsibility belongs at the documented editor, daemon, protocol, tool,
  provider, index, or storage boundary.
- Public contracts are typed, minimal, documented where their behavior is not
  self-evident, and versioned when persisted or sent across processes.
- Inputs are validated at trust boundaries; cancellation, resource bounds, and
  concurrency behavior are explicit where applicable.
- Errors retain structured context and give users or contributors a practical
  next action.
- No credentials, source bodies, private prompts, or sensitive tool results
  enter ordinary logs.
- The implementation follows the surrounding language and upstream style.

### Verification

- Formatting, linting, static analysis, and strict type checks pass.
- Deterministic unit tests cover invariants, edge cases, and failure behavior.
- Integration or contract tests cover changed boundaries.
- User-visible model behavior has versioned DeepEval cases with thresholds and
  regression reporting; deterministic mocks cover orchestration separately.
- A focused end-to-end path proves the acceptance criteria at the appropriate
  layer.
- Bugs fixed in the change receive a regression test when reproducible.
- Performance, cancellation, and resource budgets are measured when the change
  can affect them.

### Product quality

- Normal editing remains usable when Lantern is idle or unavailable.
- Accessibility covers keyboard operation, focus, semantics, contrast, and
  reduced-motion behavior for affected UI.
- Failure, meaningful tool activity, degraded intelligence, and external model
  use are visible to the developer.
- Security and privacy implications are reviewed at every new data, tool,
  process, network, or persistence boundary.
- User-facing language is concise, specific, and localizable where required by
  the host editor.

### Open-source stewardship

- Setup and verification are reproducible from a clean checkout.
- Dependencies are necessary, pinned through the appropriate lock mechanism,
  license-compatible, and reviewed for security and maintenance risk.
- Generated files identify their source and regeneration command.
- Helix patches are minimal, listed in the patch inventory, tied to an
  upstream revision, and include a removal condition.
- Public APIs, architectural decisions, migrations, and contributor workflows
  have current documentation.
- The diff contains no unexplained generated output, secrets, local paths, or
  unrelated formatting churn.

### Review evidence

The change description records:

1. The problem and why the chosen solution is the smallest coherent one.
2. Important design decisions and rejected alternatives.
3. Commands and evaluation suites run, with meaningful results.
4. Known limitations, risks, and deliberate follow-up work.
5. Screenshots or an interaction recording when visual behavior changes.

## Exceptions

Exploratory spikes may relax production implementation requirements only when
they are visibly labeled, isolated from release paths, time-bounded, and have a
written deletion or promotion decision. Security, privacy, and licensing gates
are never waived by calling work a spike.

An exception to this standard must name the unmet item, explain why meeting it
now would be worse for the product, identify the owner and expiry condition,
and remain visible in the relevant plan. Silent exceptions are not accepted.
