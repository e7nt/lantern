# ADR 002: Defer SQLite until durable state is required

- **Status:** Accepted
- **Date:** 2026-07-16
- **Decision owner:** Lantern project

## Context

The roadmap schedules transactional SQLite migrations during the daemon
foundation. The current Quick Ask slice deliberately has session-only trust,
no chat history, no recovery state, no plans, and no durable agent artifacts.
Its synchronous stdio daemon also has no async runtime. Introducing the planned
SQLx evaluation now would add database, migration, build, and runtime machinery
without improving the developer's understanding flow.

## Decision

Lantern will not add SQLite during the current Quick Ask foundation. State that
is intentionally session-only remains in memory. Portable future plans remain
Markdown. SQLite evaluation and the first migration begin only when an accepted
user journey requires durable operational state.

No ad hoc JSON persistence or alternate database replaces SQLite in the
meantime. Deferral removes a path; it does not create a fallback.

## Consequences

- `P1-06` remains deferred rather than silently marked complete.
- Transitional Protocol v4 workspace grants are revoked when the Lantern
  session ends; ADR 003 replaces that UX in the next protocol revision.
- Diagnostic bundles are explicit metadata-only exports, not a persistence
  layer.
- The future database change must still provide numbered forward migrations,
  transactional repository initialization, newer-schema refusal, compatibility
  documentation, and upgrade tests.

## Revisit conditions

Revisit when a proven experience needs recovery across process restarts,
durable repository identity, learner progress, audit history, or another record
that cannot remain a reviewable text artifact. The implementation proposal must
compare a synchronous SQLite library with SQLx and justify any async runtime.
