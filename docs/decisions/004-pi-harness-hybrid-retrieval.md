# ADR 004: Use Pi as the initial harness with hybrid retrieval

- **Status:** Accepted
- **Date:** 2026-07-17
- **Decision owner:** Lantern project

## Context

The selection-only Pi RPC spike proved subscription-authenticated streaming and
interruption, but the roadmap still proposed replacing it with a small native
loop and delaying semantic retrieval. The intended experience is broader: ask
natural questions, let the agent find and open relevant code, edit, run tests,
and work with Git without exposing retrieval mechanics to the developer.

## Decision

Pi is the initial coding-agent harness. Lantern owns the workbench, repository
context, Helix and Git integration, indexing, tool contracts, visibility, and
evaluation. Pi owns the model turn and agent loop behind a replaceable adapter.

Repository intelligence is hybrid:

1. active Helix selection and LSP symbols;
2. exact file and text search;
3. syntax, imports, manifests, and Git history;
4. semantic/vector retrieval over incrementally indexed code; and
5. commit-synchronized repository summaries checked against current source.

These signals are ranked and assembled into one bounded context package. They
are complementary evidence sources, not a stack of hidden answer fallbacks.
Generated summaries and embeddings are disposable indexes, never source of
truth.

## Consequences

- Do not build a parallel native agent loop before Pi fails a measured product
  requirement.
- The next implementation slice is a full-access Pi tool harness followed by a
  measured hybrid-index spike.
- Natural answers and direct Helix navigation are the user experience;
  evidence metadata supports trust without dominating the conversation.
- DeepEval cases must compare retrieval quality, grounding, and time to first
  useful answer with exact/LSP-only and hybrid retrieval.

## Revisit conditions

Replace Pi only if the pinned adapter cannot provide required tool control,
interruption, context isolation, reproducibility, or open-source distribution.
Replace a retrieval component when evaluation shows it adds cost without
material relevance or latency benefit.
