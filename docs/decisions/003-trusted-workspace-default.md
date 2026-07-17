# ADR 003: Start with a trusted-workspace agent

- **Status:** Accepted
- **Date:** 2026-07-17
- **Decision owner:** Lantern project
- **Amends:** Product constitution and the read-only scope in ADR 001

## Context

Lantern's first maintained slice introduced separate read and model grants and
hard-denied editing and process execution. That boundary was useful while the
editor/daemon path was unproven, but it makes the product speak in internal
capabilities and prevents the agent from being a useful coding collaborator.
The initial user is deliberately launching Lantern inside repositories they
already intend to work on.

## Decision

Launching Lantern for a workbench establishes a trusted local coding session.
The initial product does not ask the developer to configure read, write,
execution, Git, and model capabilities separately. The agent may inspect and
edit attached repositories, run development commands, use Git, and contact the
explicitly configured model.

Full access does not mean invisible operation. Commands, edits, and Git changes
remain visible; the developer can interrupt immediately; provider credentials
remain outside Lantern; and destructive Git history operations require an
explicit user request. Lantern does not add automatic background autonomy.

## Consequences

- The existing `policy-engine`, `/trust` commands, capability fields, and
  locked startup are transitional implementation, not the target experience.
- The next protocol revision should remove capability negotiation rather than
  add more permission states.
- Tool validation still protects type, path, cancellation, resource, and
  protocol invariants. It is not presented as a user permission system.
- Restrictions may be added later only in response to observed product or
  safety needs and must be evaluated as new product behavior.

This amendment does not change Lantern's open-source or local-first commitment.
It preserves authorship through visible actions and interruption, simplifies
the product by removing routine permission ceremony, and requires the obsolete
locked path to be deleted rather than retained as a fallback.

## Considered alternatives

- Keep mode-specific capability profiles: rejected because it makes internal
  authorization vocabulary part of everyday coding.
- Approve every consequential tool call: rejected because repeated prompts
  interrupt flow without proving better understanding.
- Remove visibility and interruption as well as permissions: rejected because
  that would conflict with developer authorship.

## Revisit conditions

Revisit when real use demonstrates accidental destructive actions, unsafe
repository instructions, shared-machine requirements, or demand for restricted
workbenches. Do not pre-build a general policy platform before that evidence.
