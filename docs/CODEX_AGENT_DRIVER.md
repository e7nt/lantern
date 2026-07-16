# ChatGPT-subscription agent-driver spike

## Decision

Lantern will evaluate Pi RPC as the first live `AgentDriver` in the terminal
spike. Pi may use a developer's eligible ChatGPT subscription after they
authenticate interactively with Pi's `/login` command. Lantern never reads,
copies, or persists Pi's OAuth tokens.

This is not generic OpenAI API access: ChatGPT and API billing remain separate.
The experimental boundary is a pinned, locally installed Pi process using its
documented JSONL RPC protocol. Authentication remains owned by Pi.

## Pi reference

Inspected Pi revision: `c6d8371521fc8357958bb21fd43552c15f46c7f4`
on 2026-07-15.

Adopt from Pi:

- a small provider-independent driver contract;
- streamed lifecycle and text-delta events;
- abort propagation and steering while a turn is active;
- typed tool requests with policy checks outside the prompt;
- explicit provider/auth errors rather than model fallback.

The Phase 0 implementation starts Pi with no session, tools, extensions,
skills, prompt templates, or repository context. Lantern sends only the
validated current selection and question through standard input. Text deltas
stream back to the Lantern pane, and cancellation sends Pi's RPC `abort`
command. A tool event is a boundary violation and fails the turn visibly.

Do not adopt:

- Pi session files as Lantern storage;
- runtime extension loading in the trusted daemon;
- Pi-owned permission or tool policy;
- direct reuse of Pi's OAuth credentials or provider endpoints;
- subagents, compaction, or broad coding tools in Quick Ask.

## Alternative official boundary

The official Codex app-server remains the next boundary to evaluate for a
deeper product integration. It exposes these relevant primitives:

- `thread/start` and `turn/start`;
- `item/agentMessage/delta` to Lantern text deltas;
- `turn/steer` to an explicit user interruption/follow-up;
- `turn/interrupt` to Lantern cancellation;
- terminal settled, failure, and usage events.

An app-server experiment must run from an empty private directory, receive only
bounded Lantern evidence, and expose no edit, execution, MCP, or
repository-discovery tool. It is not a silent fallback when Pi fails. The user
selects an agent driver explicitly, and protocol or authentication failures
stop visibly.

## Promotion gate

The Pi driver is promoted only if deterministic protocol and cancellation tests
and versioned DeepEval cases pass for evidence grounding, uncertainty, and
understanding value. Remove it if Pi cannot preserve the bounded no-tools
context or if its subscription-authenticated behavior cannot be reproduced
with the pinned version. Evaluate Codex app-server separately; do not combine
the two drivers behind automatic fallback behavior.
