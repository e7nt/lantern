# Read-only feature investigation

## Outcome

`/investigate <feature objective>` produces a concise readiness brief in
Lantern's existing agent pane. The brief covers Goal, Observed, Affected flow,
Likely changes, Open questions, Acceptance criteria, Exclusions, Risks, and an
explicit Ready or Blocked result. Files actually read during the investigation
are emitted as navigable evidence and open through the existing Helix bridge.

## Boundary

- Protocol v10 adds one typed `investigate_agent` request.
- The daemon starts a scoped Pi RPC process with only `read`, `grep`, `find`,
  and `ls`; `edit`, `write`, and `bash` are absent from its tool schema.
- The process uses the same pinned Pi binary, model, subscription
  authentication, streaming lifecycle, cancellation path, and terminal UI.
- The scoped process ends with the operation. Normal coding turns retain their
  warm persistent Pi session and full trusted-workbench tools.
- Up to 64 KiB of streamed brief text is retained in memory and injected once
  into the next coding turn as untrusted, freshness-qualified context. A direct
  “proceed” therefore retains the investigation without creating durable chat
  history.
- The result is not persisted and does not create a planning database, report
  parser, second transcript, or new panel.

The separate scoped process is an intentional safety boundary. Pi RPC 0.80.6
does not expose runtime active-tool replacement, and prompt-only read-only
instructions would not enforce the product promise. Lantern rejects that
weaker alternative despite its lower startup cost.

## Evidence

- A daemon integration test proves the exact read-only tool allowlist, required
  brief prompt, navigable read evidence, completed lifecycle, byte-identical
  repository source, and one-time handoff to the warm coding session.
- Protocol v10 golden fixtures cover the new request and investigation evidence
  provenance.
- The versioned DeepEval contract requires all brief sections, explicit
  readiness, grounded facts, and rejection of false implementation or
  verification claims.
- Existing rendering and navigation tests cover evidence interaction through
  the shared terminal path.

## Deliberate next gate

Run a subscription-backed investigation on a real feature and record factual
grounding, useful unknowns, tool count, first activity, and total latency before
promoting the brief into a durable Markdown plan.
